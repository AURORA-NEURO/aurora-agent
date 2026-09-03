//! Bounded retention for deterministic developer-workbench reports.
//!
//! [`crate::workbench`] produces a complete authoring/notebook projection, but a report is only
//! useful operationally if a caller can retain it, find it later, and prove that a restart did
//! not silently change its contents. This module supplies that narrow storage boundary. It stores
//! only structurally valid workbench reports, keys them by the canonical report digest, exposes
//! digest-ordered index queries, and verifies both report and snapshot digests during restore.
//!
//! The registry is deliberately not an execution queue, notebook database, CI runner, GitHub
//! client, release gate, or provenance authority. A retained `release_ready` field is an evidence
//! posture emitted by the workbench and remains caller-reviewable rather than becoming approval.

use crate::workbench::{WorkbenchReport, WORKBENCH_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const WORKBENCH_REGISTRY_SCHEMA_VERSION: &str = "bioprism-devplat-workbench-registry/0.1";
pub const WORKBENCH_REGISTRY_IMPORT_SCHEMA_VERSION: &str = "bioprism-devplat-workbench-import/0.1";
pub const WORKBENCH_REGISTRY_QUERY_SCHEMA_VERSION: &str = "bioprism-devplat-workbench-query/0.1";
pub const WORKBENCH_REGISTRY_GET_SCHEMA_VERSION: &str = "bioprism-devplat-workbench-get/0.1";
pub const MAX_WORKBENCH_REPORTS: usize = 512;
pub const MAX_WORKBENCH_REGISTRY_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_WORKBENCH_QUERY_ITEMS: usize = 256;
const MAX_REPORT_NOTES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkbenchRegistryError {
    #[error("workbench registry input is not an object")]
    NotObject,
    #[error("workbench report is invalid: {0}")]
    InvalidReport(String),
    #[error("workbench registry has reached its {maximum}-report limit")]
    Full { maximum: usize },
    #[error("workbench report digest conflict for {digest}")]
    Conflict { digest: String },
    #[error("workbench registry snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("workbench registry snapshot is {actual} bytes, above the {maximum}-byte bound")]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("workbench registry could not be canonicalised: {0}")]
    Canonicalisation(String),
}

#[derive(Debug, Clone, Default)]
pub struct WorkbenchReportRegistry {
    generation: u64,
    reports: BTreeMap<String, Value>,
}

impl WorkbenchReportRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.reports.len()
    }

    pub fn is_empty(&self) -> bool {
        self.reports.is_empty()
    }

    /// Return deterministic report identities without exposing retained report bodies.
    pub fn digests_for_audit(&self) -> Vec<String> {
        self.reports.keys().cloned().collect()
    }

    /// Import one direct or transport-wrapped workbench report.
    ///
    /// Transport metadata (`request_id`, MCP error markers, and the response envelope) is removed
    /// before identity is calculated. This makes a report returned through MCP or REST safely
    /// round-trippable while keeping the canonical stored value equal to `WorkbenchReport`.
    pub fn import(&mut self, report: &Value) -> Result<Value, WorkbenchRegistryError> {
        let normalized = normalized_report(report)?;
        let digest = verify_report(&normalized)?;
        let already_present = match self.reports.get(&digest) {
            None => false,
            Some(existing) if existing == &normalized => true,
            Some(_) => {
                return Err(WorkbenchRegistryError::Conflict { digest });
            }
        };
        if !already_present && self.reports.len() >= MAX_WORKBENCH_REPORTS {
            return Err(WorkbenchRegistryError::Full {
                maximum: MAX_WORKBENCH_REPORTS,
            });
        }
        if !already_present {
            let mut candidate = self.clone();
            candidate.reports.insert(digest.clone(), normalized);
            candidate.generation = candidate.generation.saturating_add(1);
            candidate.ensure_snapshot_bound()?;
            self.reports = candidate.reports;
            self.generation = candidate.generation;
        }
        Ok(json!({
            "ok": true,
            "schema": WORKBENCH_REGISTRY_IMPORT_SCHEMA_VERSION,
            "workflow": "developer_workbench_import",
            "workbench_report_digest": digest,
            "created": !already_present,
            "already_present": already_present,
            "registry_generation": self.generation,
            "registry_size": self.reports.len(),
            "execution": "not_started",
            "guarantees": [
                "only a structurally valid developer_workbench report is retained",
                "re-importing the same canonical report is idempotent",
                "import does not execute notebook cells, CI, GitHub, or any external effect"
            ],
            "limitations": [
                "the registry is a bounded local report index rather than a distributed object store",
                "structural release_ready posture is not scientific, clinical, security, or production approval"
            ]
        }))
    }

    pub fn get(&self, digest: &str) -> Option<Value> {
        self.reports.get(digest).cloned()
    }

    /// Return one digest-bound report envelope, keeping the raw `get` method useful internally.
    pub fn get_response(&self, digest: &str) -> Result<Value, WorkbenchRegistryError> {
        let report = self.get(digest).ok_or_else(|| {
            WorkbenchRegistryError::InvalidReport(format!("no report exists for digest {digest}"))
        })?;
        Ok(json!({
            "ok": true,
            "schema": WORKBENCH_REGISTRY_GET_SCHEMA_VERSION,
            "workflow": "developer_workbench_get",
            "workbench_report_digest": digest,
            "report": report,
            "registry_generation": self.generation,
            "registry_size": self.reports.len(),
            "execution": "not_started",
            "guarantees": [
                "the returned report was revalidated when it entered the registry",
                "the lookup does not execute or re-evaluate the workbench"
            ],
            "limitations": [
                "absence means only that this bounded registry has no matching report"
            ]
        }))
    }

    /// Query digest-ordered report index rows without returning full bodies by default.
    #[allow(clippy::too_many_arguments)]
    pub fn query(
        &self,
        session_digest: Option<&str>,
        domain: Option<&str>,
        capability: Option<&str>,
        state: Option<&str>,
        release_ready: Option<bool>,
        after: Option<&str>,
        max_items: usize,
        include_reports: bool,
    ) -> Result<Value, WorkbenchRegistryError> {
        if !(1..=MAX_WORKBENCH_QUERY_ITEMS).contains(&max_items) {
            return Err(WorkbenchRegistryError::InvalidSnapshot(format!(
                "max_items must be between 1 and {MAX_WORKBENCH_QUERY_ITEMS}"
            )));
        }
        for (field, value) in [
            ("domain", domain),
            ("capability", capability),
            ("state", state),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty()
                    || value != value.trim()
                    || value.chars().any(char::is_control)
                {
                    return Err(WorkbenchRegistryError::InvalidSnapshot(format!(
                        "{field} must be bounded text without surrounding whitespace or controls"
                    )));
                }
            }
        }
        for (field, value) in [("session_digest", session_digest), ("after", after)] {
            if let Some(value) = value {
                ContentHash::parse(value.to_string()).map_err(|_| {
                    WorkbenchRegistryError::InvalidSnapshot(format!(
                        "{field} must be a lowercase SHA-256 content hash"
                    ))
                })?;
            }
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, report) in self
            .reports
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            let index = index_row(digest, report)?;
            let matches = session_digest.is_none_or(|value| {
                index.get("session_digest").and_then(Value::as_str) == Some(value)
            }) && domain.is_none_or(|value| {
                index
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(|item| item.as_str() == Some(value)))
            }) && capability.is_none_or(|value| {
                index
                    .get("capabilities")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(|item| item.as_str() == Some(value)))
            }) && state.is_none_or(|value| {
                index
                    .get("states")
                    .and_then(Value::as_array)
                    .is_some_and(|values| values.iter().any(|item| item.as_str() == Some(value)))
            }) && release_ready.is_none_or(|value| {
                index.get("release_ready").and_then(Value::as_bool) == Some(value)
            });
            if !matches {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = index;
            if include_reports {
                row["report"] = report.clone();
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("workbench_report_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": WORKBENCH_REGISTRY_QUERY_SCHEMA_VERSION,
            "workflow": "developer_workbench_query",
            "filters": {
                "session_digest": session_digest,
                "domain": domain,
                "capability": capability,
                "state": state,
                "release_ready": release_ready,
                "after": after,
                "max_items": max_items,
                "include_reports": include_reports
            },
            "registry_generation": self.generation,
            "registry_size": self.reports.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by canonical workbench report digest",
                "filters are applied to retained structural metadata",
                "query does not execute notebook cells, CI, GitHub, or any external effect"
            ],
            "limitations": [
                "results cover only the bounded local registry",
                "absence from this registry is not evidence that a report never existed"
            ]
        }))
    }

    /// Return a digest-protected checkpoint suitable for atomic persistence.
    pub fn snapshot(&self) -> Result<Value, WorkbenchRegistryError> {
        let mut document = json!({
            "schema": WORKBENCH_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "report_count": self.reports.len(),
            "reports": self.reports.iter().map(|(digest, report)| json!({
                "workbench_report_digest": digest,
                "report": report
            })).collect::<Vec<_>>(),
            "retention": {
                "max_reports": MAX_WORKBENCH_REPORTS,
                "max_bytes": MAX_WORKBENCH_REGISTRY_BYTES
            },
            "execution": "not_started"
        });
        let state_digest = snapshot_digest(&document)?;
        document["state_digest"] = Value::String(state_digest);
        self.ensure_encoded_bound(&document)?;
        Ok(document)
    }

    /// Restore a registry while revalidating every report and the registry state digest.
    pub fn from_snapshot(document: &Value) -> Result<Self, WorkbenchRegistryError> {
        let object = document.as_object().ok_or_else(|| {
            WorkbenchRegistryError::InvalidSnapshot("snapshot must be an object".into())
        })?;
        let encoded = serde_json::to_vec(document)
            .map_err(|error| WorkbenchRegistryError::Canonicalisation(error.to_string()))?;
        if encoded.len() > MAX_WORKBENCH_REGISTRY_BYTES {
            return Err(WorkbenchRegistryError::SnapshotTooLarge {
                actual: encoded.len(),
                maximum: MAX_WORKBENCH_REGISTRY_BYTES,
            });
        }
        if object.get("schema").and_then(Value::as_str) != Some(WORKBENCH_REGISTRY_SCHEMA_VERSION) {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "schema is invalid".into(),
            ));
        }
        let claimed_digest = object
            .get("state_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("state_digest is missing".into())
            })?;
        let mut unsigned = document.clone();
        let Some(unsigned_object) = unsigned.as_object_mut() else {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "snapshot is not an object after cloning".into(),
            ));
        };
        unsigned_object.remove("state_digest");
        if claimed_digest != snapshot_digest(&unsigned)? {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("generation is invalid".into())
            })?;
        if object.get("execution").and_then(Value::as_str) != Some("not_started") {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "execution must remain not_started".into(),
            ));
        }
        let retention = object
            .get("retention")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("retention must be an object".into())
            })?;
        if retention.get("max_reports").and_then(Value::as_u64)
            != Some(MAX_WORKBENCH_REPORTS as u64)
            || retention.get("max_bytes").and_then(Value::as_u64)
                != Some(MAX_WORKBENCH_REGISTRY_BYTES as u64)
        {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "retention does not match the registry bounds".into(),
            ));
        }
        let rows = object
            .get("reports")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("reports must be an array".into())
            })?;
        if rows.len() > MAX_WORKBENCH_REPORTS {
            return Err(WorkbenchRegistryError::Full {
                maximum: MAX_WORKBENCH_REPORTS,
            });
        }
        if generation < rows.len() as u64 {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "generation cannot be below the retained report count".into(),
            ));
        }
        let mut registry = Self {
            generation,
            reports: BTreeMap::new(),
        };
        for row in rows {
            let row_object = row.as_object().ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("report index row must be an object".into())
            })?;
            let claimed = row_object
                .get("workbench_report_digest")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    WorkbenchRegistryError::InvalidSnapshot(
                        "workbench_report_digest is missing".into(),
                    )
                })?;
            let report = row_object.get("report").ok_or_else(|| {
                WorkbenchRegistryError::InvalidSnapshot("workbench report body is missing".into())
            })?;
            let recomputed = verify_report(report).map_err(|error| {
                WorkbenchRegistryError::InvalidSnapshot(format!(
                    "workbench report {claimed} is invalid: {error}"
                ))
            })?;
            if recomputed != claimed {
                return Err(WorkbenchRegistryError::InvalidSnapshot(format!(
                    "workbench report {claimed} failed digest verification"
                )));
            }
            if registry
                .reports
                .insert(claimed.to_string(), report.clone())
                .is_some()
            {
                return Err(WorkbenchRegistryError::InvalidSnapshot(
                    "snapshot contains duplicate workbench report digests".into(),
                ));
            }
        }
        if object.get("report_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(WorkbenchRegistryError::InvalidSnapshot(
                "report_count does not match reports".into(),
            ));
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), WorkbenchRegistryError> {
        let document = self.snapshot()?;
        self.ensure_encoded_bound(&document)
    }

    fn ensure_encoded_bound(&self, document: &Value) -> Result<(), WorkbenchRegistryError> {
        let bytes = serde_json::to_vec(document)
            .map_err(|error| WorkbenchRegistryError::Canonicalisation(error.to_string()))?;
        if bytes.len() > MAX_WORKBENCH_REGISTRY_BYTES {
            return Err(WorkbenchRegistryError::SnapshotTooLarge {
                actual: bytes.len(),
                maximum: MAX_WORKBENCH_REGISTRY_BYTES,
            });
        }
        Ok(())
    }
}

fn normalized_report(report: &Value) -> Result<Value, WorkbenchRegistryError> {
    if !report.is_object() {
        return Err(WorkbenchRegistryError::NotObject);
    }
    let mut normalized = report.clone();
    let Some(object) = normalized.as_object_mut() else {
        return Err(WorkbenchRegistryError::InvalidReport(
            "report is not an object after cloning".into(),
        ));
    };
    if let Some(ok) = object.get("ok") {
        if ok != &Value::Bool(true) {
            return Err(WorkbenchRegistryError::InvalidReport(
                "ok must be the boolean true when supplied".into(),
            ));
        }
    }
    if let Some(workflow) = object.get("workflow") {
        if workflow.as_str() != Some("developer_workbench") {
            return Err(WorkbenchRegistryError::InvalidReport(
                "workflow must be the developer_workbench string when supplied".into(),
            ));
        }
    }
    if let Some(schema) = object.get("workbench_schema_version") {
        if schema.as_str() != Some(WORKBENCH_SCHEMA_VERSION) {
            return Err(WorkbenchRegistryError::InvalidReport(
                "workbench_schema_version must be the canonical string when supplied".into(),
            ));
        }
    }
    if let Some(is_error) = object.get("__isError") {
        if is_error != &Value::Bool(false) {
            return Err(WorkbenchRegistryError::InvalidReport(
                "__isError must be the boolean false when supplied".into(),
            ));
        }
    }
    for field in [
        "request_id",
        "__isError",
        "artifact_registry",
        "ok",
        "workflow",
        "workbench_schema_version",
        "workbench_report_digest",
    ] {
        object.remove(field);
    }
    Ok(normalized)
}

fn verify_report(report: &Value) -> Result<String, WorkbenchRegistryError> {
    let object = report
        .as_object()
        .ok_or(WorkbenchRegistryError::NotObject)?;
    let typed: WorkbenchReport = serde_json::from_value(report.clone()).map_err(|error| {
        WorkbenchRegistryError::InvalidReport(format!("schema validation failed: {error}"))
    })?;
    if typed.schema_version != WORKBENCH_SCHEMA_VERSION {
        return Err(WorkbenchRegistryError::InvalidReport(
            "schema_version is invalid".into(),
        ));
    }
    ContentHash::parse(typed.audit.session_digest.clone()).map_err(|_| {
        WorkbenchRegistryError::InvalidReport(
            "audit.session_digest must be a lowercase SHA-256 content hash".into(),
        )
    })?;
    if let Some(ci) = &typed.ci {
        ContentHash::parse(ci.digest.clone()).map_err(|_| {
            WorkbenchRegistryError::InvalidReport(
                "ci.digest must be a lowercase SHA-256 content hash".into(),
            )
        })?;
        if ci.workflow_yaml.trim().is_empty() {
            return Err(WorkbenchRegistryError::InvalidReport(
                "ci.workflow_yaml must be non-empty".into(),
            ));
        }
        let computed_ci_digest = ContentHash::of_bytes(ci.workflow_yaml.as_bytes()).to_string();
        if ci.digest != computed_ci_digest {
            return Err(WorkbenchRegistryError::InvalidReport(
                "ci.digest does not match ci.workflow_yaml".into(),
            ));
        }
        if ci.execution != "not_executed" {
            return Err(WorkbenchRegistryError::InvalidReport(
                "ci.execution must remain not_executed".into(),
            ));
        }
        if ci.required_check_count > ci.check_count || ci.check_count == 0 {
            return Err(WorkbenchRegistryError::InvalidReport(
                "ci check counts are inconsistent".into(),
            ));
        }
    }
    for field in ["schema_version", "audit", "guarantees", "limitations"] {
        if !object.contains_key(field) {
            return Err(WorkbenchRegistryError::InvalidReport(format!(
                "{field} is missing"
            )));
        }
    }
    for (field, values) in [
        ("guarantees", &typed.guarantees),
        ("limitations", &typed.limitations),
    ] {
        if values.is_empty()
            || values.len() > MAX_REPORT_NOTES
            || values.iter().any(|value| value.trim().is_empty())
        {
            return Err(WorkbenchRegistryError::InvalidReport(format!(
                "{field} must contain between 1 and {MAX_REPORT_NOTES} non-empty entries"
            )));
        }
    }
    if typed.audit.ordered_cells.len() != typed.audit.cell_count
        || typed.audit.executed_cell_count > typed.audit.cell_count
        || (typed.audit.release_ready && !typed.audit.release_blockers.is_empty())
    {
        return Err(WorkbenchRegistryError::InvalidReport(
            "audit counts or release posture are inconsistent".into(),
        ));
    }
    ContentHash::of_value(report)
        .map(|digest| digest.to_string())
        .map_err(|error| WorkbenchRegistryError::Canonicalisation(error.to_string()))
}

fn snapshot_digest(document: &Value) -> Result<String, WorkbenchRegistryError> {
    ContentHash::of_value(document)
        .map(|digest| digest.to_string())
        .map_err(|error| WorkbenchRegistryError::Canonicalisation(error.to_string()))
}

fn index_row(digest: &str, report: &Value) -> Result<Value, WorkbenchRegistryError> {
    let typed: WorkbenchReport = serde_json::from_value(report.clone()).map_err(|error| {
        WorkbenchRegistryError::InvalidReport(format!("schema validation failed: {error}"))
    })?;
    let mut domains = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut states = BTreeSet::new();
    let mut dashboard_matched = 0usize;
    let mut dashboard_holes = 0usize;
    let mut dashboard_stale = 0usize;
    if let Some(dashboard) = &typed.dashboard {
        dashboard_matched = dashboard.matched;
        dashboard_holes = dashboard.holes;
        dashboard_stale = dashboard.stale;
        for row in &dashboard.rows {
            domains.insert(row.domain.clone());
            capabilities.insert(row.capability.clone());
            states.insert(
                serde_json::to_string(&row.state)
                    .map_err(|error| WorkbenchRegistryError::Canonicalisation(error.to_string()))?
                    .trim_matches('"')
                    .to_string(),
            );
        }
    }
    Ok(json!({
        "workbench_report_digest": digest,
        "schema_version": typed.schema_version,
        "session_digest": typed.audit.session_digest,
        "audit_valid": typed.audit.valid,
        "release_ready": typed.audit.release_ready,
        "artifact_count": typed.audit.artifact_count,
        "cell_count": typed.audit.cell_count,
        "change_count": typed.audit.change_count,
        "executed_cell_count": typed.audit.executed_cell_count,
        "dashboard_present": typed.dashboard.is_some(),
        "dashboard_matched": dashboard_matched,
        "dashboard_holes": dashboard_holes,
        "dashboard_stale": dashboard_stale,
        "ci_present": typed.ci.is_some(),
        "ci_digest": typed.ci.as_ref().map(|ci| ci.digest.clone()),
        "domains": domains.into_iter().collect::<Vec<_>>(),
        "capabilities": capabilities.into_iter().collect::<Vec<_>>(),
        "states": states.into_iter().collect::<Vec<_>>(),
        "execution": "not_started"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workbench::{
        run_workbench, ArtifactCard, ArtifactState, DashboardQuery, EvidencePosture, StudioCell,
        StudioSession, WorkbenchRequest,
    };

    fn report() -> Value {
        let session = StudioSession {
            session_id: "registry-session".into(),
            owner: "tester".into(),
            goal: "retain a report".into(),
            environment_digest: None,
            artifacts: vec![ArtifactCard {
                id: "artifact".into(),
                title: "Artifact".into(),
                path: "artifacts/result.json".into(),
                domain: "oncology".into(),
                capability: "evidence".into(),
                state: ArtifactState::Validated,
                evidence: EvidencePosture::Observed,
                digest: Some("a".repeat(64)),
                score: Some(0.9),
                tags: vec![],
            }],
            cells: vec![StudioCell {
                id: "review".into(),
                kind: crate::workbench::CellKind::Review,
                source: "inspect".into(),
                inputs: vec![],
                depends_on: vec![],
                executed: false,
                output_digest: None,
            }],
            changes: vec![],
            policy: Default::default(),
        };
        serde_json::to_value(
            run_workbench(&WorkbenchRequest {
                session,
                dashboard: Some(DashboardQuery::default()),
                ci: None,
            })
            .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn imports_transport_envelope_idempotently_and_indexes_domain() {
        let canonical = report();
        let mut transport = canonical.clone();
        transport["ok"] = json!(true);
        transport["workflow"] = json!("developer_workbench");
        transport["workbench_schema_version"] = json!(WORKBENCH_SCHEMA_VERSION);
        transport["request_id"] = json!("req-1");
        transport["__isError"] = json!(false);
        let mut registry = WorkbenchReportRegistry::new();
        let first = registry.import(&transport).unwrap();
        let second = registry.import(&canonical).unwrap();
        assert_eq!(first["created"], json!(true));
        assert_eq!(second["already_present"], json!(true));
        assert_eq!(registry.len(), 1);
        let digest = first["workbench_report_digest"].as_str().unwrap();
        let query = registry
            .query(
                None,
                Some("oncology"),
                Some("evidence"),
                None,
                None,
                None,
                10,
                false,
            )
            .unwrap();
        assert_eq!(query["rows"][0]["workbench_report_digest"], digest);
        assert_eq!(registry.get(digest), Some(canonical));
    }

    #[test]
    fn snapshot_round_trip_and_tamper_are_digest_bound() {
        let mut registry = WorkbenchReportRegistry::new();
        registry.import(&report()).unwrap();
        let snapshot = registry.snapshot().unwrap();
        let restored = WorkbenchReportRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.digests_for_audit(), registry.digests_for_audit());
        let mut tampered = snapshot;
        tampered["reports"][0]["report"]["limitations"] = json!(["tampered"]);
        assert!(matches!(
            WorkbenchReportRegistry::from_snapshot(&tampered),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn rejects_invalid_schema_and_query_cursor() {
        let mut registry = WorkbenchReportRegistry::new();
        assert!(matches!(
            registry.import(&json!({"schema_version": WORKBENCH_SCHEMA_VERSION})),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));
        assert!(matches!(
            registry.query(None, None, None, None, None, Some("not-a-digest"), 1, false),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));

        let mut malformed_wrapper = report();
        malformed_wrapper["ok"] = json!("true");
        assert!(matches!(
            registry.import(&malformed_wrapper),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));

        let mut malformed_error_marker = report();
        malformed_error_marker["__isError"] = json!(true);
        assert!(matches!(
            registry.import(&malformed_error_marker),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));

        assert!(matches!(
            registry.query(None, Some(" oncology"), None, None, None, None, 1, false),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn import_rejects_a_digest_conflict_instead_of_replacing_retained_report() {
        let canonical = report();
        let digest = ContentHash::of_value(&canonical).unwrap().to_string();
        let mut registry = WorkbenchReportRegistry::new();
        registry
            .reports
            .insert(digest.clone(), json!({"different": "report"}));

        assert!(matches!(
            registry.import(&canonical),
            Err(WorkbenchRegistryError::Conflict { digest: received })
                if received == digest
        ));
        assert_eq!(registry.reports[&digest], json!({"different": "report"}));
        assert_eq!(registry.generation(), 0);
    }

    fn reseal_snapshot(mut document: Value) -> Value {
        document
            .as_object_mut()
            .expect("snapshot fixture must be an object")
            .remove("state_digest");
        let digest = snapshot_digest(&document).unwrap();
        document["state_digest"] = json!(digest);
        document
    }

    #[test]
    fn digest_valid_snapshot_metadata_still_has_to_match_registry_contract() {
        let mut registry = WorkbenchReportRegistry::new();
        registry.import(&report()).unwrap();

        let mut execution = registry.snapshot().unwrap();
        execution["execution"] = json!("executed");
        assert!(matches!(
            WorkbenchReportRegistry::from_snapshot(&reseal_snapshot(execution)),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));

        let mut retention = registry.snapshot().unwrap();
        retention["retention"]["max_reports"] = json!(1);
        assert!(matches!(
            WorkbenchReportRegistry::from_snapshot(&reseal_snapshot(retention)),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));

        let mut generation = registry.snapshot().unwrap();
        generation["generation"] = json!(0);
        assert!(matches!(
            WorkbenchReportRegistry::from_snapshot(&reseal_snapshot(generation)),
            Err(WorkbenchRegistryError::InvalidSnapshot(_))
        ));
    }

    #[test]
    fn report_metadata_and_audit_posture_are_not_accepted_as_empty_or_inconsistent() {
        let mut registry = WorkbenchReportRegistry::new();

        let mut empty_guarantees = report();
        empty_guarantees["guarantees"] = json!([]);
        assert!(matches!(
            registry.import(&empty_guarantees),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));

        let mut inconsistent_audit = report();
        inconsistent_audit["audit"]["ordered_cells"] = json!([]);
        assert!(matches!(
            registry.import(&inconsistent_audit),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));
    }

    #[test]
    fn retained_ci_plan_is_digest_bound_and_explicitly_not_executed() {
        let mut value = report();
        let workflow_yaml = "name: 'ci'\n";
        let digest = ContentHash::of_bytes(workflow_yaml.as_bytes()).to_string();
        value["ci"] = json!({
            "workflow": "ci",
            "workflow_yaml": workflow_yaml,
            "digest": digest,
            "check_count": 1,
            "required_check_count": 1,
            "execution": "not_executed",
            "network_access": "denied_by_plan",
            "limitations": ["review only"]
        });
        let mut registry = WorkbenchReportRegistry::new();
        registry.import(&value).unwrap();

        value["ci"]["execution"] = json!("executed");
        assert!(matches!(
            registry.import(&value),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));
        value["ci"]["execution"] = json!("not_executed");
        value["ci"]["digest"] = json!("a".repeat(64));
        assert!(matches!(
            registry.import(&value),
            Err(WorkbenchRegistryError::InvalidReport(_))
        ));
    }
}
