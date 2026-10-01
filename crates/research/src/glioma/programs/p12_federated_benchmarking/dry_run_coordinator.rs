//! Synthetic/site-local federated benchmark dry-run coordination.
//!
//! This feature exercises the contract surface before live federation: binding, schema, model,
//! fixture, approval, locality, failure-mode, cost, and quorum checks are evaluated for every
//! supplied site. The resulting report is explicitly simulation-only and cannot query a site or
//! turn a synthetic pass into biological evidence.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F13";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBenchmarkDryRun1@1";
pub const MAX_FIXTURES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDryRunSiteFixture {
    pub site_id: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub model_system: GliomaModelSystem,
    pub schema_version: String,
    pub synthetic_artifact_count: usize,
    pub projected_cost_units: u64,
    pub approval_required: bool,
    pub approval_granted: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub declared_failure_modes: Vec<String>,
    pub fixture_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkDryRunRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub model_system: GliomaModelSystem,
    pub required_schema_version: String,
    pub minimum_sites: usize,
    pub maximum_budget_units: u64,
    pub require_approval: bool,
    pub fixtures: Vec<FederatedDryRunSiteFixture>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunCheckStatus {
    Pass,
    Fail,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DryRunCheck {
    pub site_id: String,
    pub check_id: String,
    pub status: DryRunCheckStatus,
    pub reason: String,
    pub projected_cost_units: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBenchmarkDryRunReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub site_order: Vec<String>,
    pub passed_site_order: Vec<String>,
    pub failed_site_order: Vec<String>,
    pub unexecuted_site_order: Vec<String>,
    pub checks: Vec<DryRunCheck>,
    pub projected_cost_units: u64,
    pub quorum_possible: bool,
    pub budget_satisfied: bool,
    pub approval_complete: bool,
    pub simulation_only: bool,
    pub evidence_status: String,
    pub disposition: DryRunDisposition,
    pub negative_evidence: Vec<String>,
    pub omissions: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedDryRunError {
    #[error("federated dry-run request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated dry-run report is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated dry-run digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value)) && values.windows(2).all(|pair| pair[0] != pair[1])
}

fn output_body(report: &FederatedBenchmarkDryRunReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "site_order": report.site_order,
        "passed_site_order": report.passed_site_order,
        "failed_site_order": report.failed_site_order,
        "unexecuted_site_order": report.unexecuted_site_order,
        "checks": report.checks,
        "projected_cost_units": report.projected_cost_units,
        "quorum_possible": report.quorum_possible,
        "budget_satisfied": report.budget_satisfied,
        "approval_complete": report.approval_complete,
        "simulation_only": report.simulation_only,
        "evidence_status": report.evidence_status,
        "disposition": report.disposition,
        "negative_evidence": report.negative_evidence,
        "omissions": report.omissions,
    })
}

impl FederatedBenchmarkDryRunReport {
    pub fn validate(&self) -> Result<(), FederatedDryRunError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !canonical(&self.site_order)
            || !canonical(&self.passed_site_order)
            || !canonical(&self.failed_site_order)
            || !canonical(&self.unexecuted_site_order)
            || !self.simulation_only
            || self.evidence_status != "simulation_only_non_evidence"
            || self.checks.iter().any(|check| {
                !safe_text(&check.site_id)
                    || !safe_text(&check.check_id)
                    || !safe_text(&check.reason)
            })
        {
            return Err(FederatedDryRunError::InvalidOutput(
                "dry-run identity, non-evidence boundary, site partitions, or check invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederatedDryRunError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedDryRunError::InvalidOutput(
                "dry-run report digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &FederatedBenchmarkDryRunRequest) -> Result<(), FederatedDryRunError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.capability_id)
        || !safe_text(&request.benchmark_world)
        || !safe_text(&request.required_schema_version)
        || request.minimum_sites == 0
        || request.maximum_budget_units == 0
        || request.fixtures.is_empty()
        || request.fixtures.len() > MAX_FIXTURES
    {
        return Err(FederatedDryRunError::InvalidRequest(
            "objective, benchmark binding, schema, quorum, budget, and bounded fixtures are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for fixture in &request.fixtures {
        if !safe_text(&fixture.site_id)
            || !ids.insert(fixture.site_id.clone())
            || !safe_text(&fixture.capability_id)
            || !safe_text(&fixture.benchmark_world)
            || !safe_text(&fixture.schema_version)
            || fixture.synthetic_artifact_count == 0
            || fixture.projected_cost_units == 0
            || fixture.contains_human_data
            || fixture.contains_direct_identifiers
            || fixture
                .declared_failure_modes
                .iter()
                .any(|mode| !safe_text(mode))
        {
            return Err(FederatedDryRunError::InvalidRequest(format!(
                "fixture {} is invalid, empty, protected, or duplicated",
                fixture.site_id
            )));
        }
    }
    Ok(())
}

pub fn execute_federated_benchmark_dry_run(
    request: &FederatedBenchmarkDryRunRequest,
) -> Result<FederatedBenchmarkDryRunReport, FederatedDryRunError> {
    validate_request(request)?;
    let mut fixtures = request.fixtures.iter().collect::<Vec<_>>();
    fixtures.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let mut checks = Vec::new();
    let mut passed = Vec::new();
    let mut failed = Vec::new();
    let mut unexecuted = Vec::new();
    let mut omissions = Vec::new();
    let mut negative = Vec::new();
    let mut projected_cost = 0_u64;
    for fixture in &fixtures {
        let mut site_failed = false;
        let checks_for_site = [
            (
                "binding",
                fixture.capability_id == request.capability_id
                    && fixture.benchmark_world == request.benchmark_world
                    && fixture.model_system == request.model_system,
                "capability, benchmark-world, and model-system binding",
            ),
            (
                "schema",
                fixture.schema_version == request.required_schema_version,
                "schema-version compatibility",
            ),
            (
                "fixture",
                fixture.synthetic_artifact_count > 0,
                "synthetic fixture availability",
            ),
            (
                "approval",
                !request.require_approval || !fixture.approval_required || fixture.approval_granted,
                "approval requirement",
            ),
            (
                "locality",
                fixture.local_only,
                "local-only dry-run boundary",
            ),
            (
                "declared_failure_modes",
                fixture.declared_failure_modes.is_empty(),
                "declared synthetic failure modes",
            ),
        ];
        for (check_id, ok, description) in checks_for_site {
            let status = if ok {
                DryRunCheckStatus::Pass
            } else {
                site_failed = true;
                DryRunCheckStatus::Fail
            };
            let reason = if ok {
                format!("{description} passed in simulation")
            } else {
                format!("{description} failed in simulation")
            };
            checks.push(DryRunCheck {
                site_id: fixture.site_id.clone(),
                check_id: check_id.into(),
                status,
                reason,
                projected_cost_units: fixture.projected_cost_units,
            });
        }
        if site_failed {
            failed.push(fixture.site_id.clone());
            if fixture.approval_required && !fixture.approval_granted {
                unexecuted.push(fixture.site_id.clone());
                omissions.push(format!(
                    "{} approval was not granted; no live query was attempted",
                    fixture.site_id
                ));
            }
            negative.push(format!(
                "{} has one or more simulated dry-run contract failures",
                fixture.site_id
            ));
        } else {
            passed.push(fixture.site_id.clone());
            projected_cost = projected_cost.saturating_add(fixture.projected_cost_units);
        }
    }
    passed.sort();
    failed.sort();
    unexecuted.sort();
    omissions.sort();
    negative.sort();
    negative.dedup();
    let budget_satisfied = projected_cost <= request.maximum_budget_units;
    if !budget_satisfied {
        negative.push("passed-fixture projected cost exceeds the dry-run budget".into());
    }
    let quorum_possible = passed.len() >= request.minimum_sites;
    if !quorum_possible {
        negative.push("dry-run pass count is below the required quorum".into());
    }
    if !budget_satisfied || !quorum_possible {
        negative.sort();
        negative.dedup();
    }
    let disposition = if budget_satisfied && quorum_possible && failed.is_empty() {
        DryRunDisposition::Ready
    } else if !passed.is_empty() && quorum_possible && budget_satisfied {
        DryRunDisposition::Partial
    } else {
        DryRunDisposition::Blocked
    };
    let mut report = FederatedBenchmarkDryRunReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        site_order: fixtures
            .iter()
            .map(|fixture| fixture.site_id.clone())
            .collect(),
        passed_site_order: passed,
        failed_site_order: failed,
        unexecuted_site_order: unexecuted,
        checks,
        projected_cost_units: projected_cost,
        quorum_possible,
        budget_satisfied,
        approval_complete: omissions.is_empty(),
        simulation_only: true,
        evidence_status: "simulation_only_non_evidence".into(),
        disposition,
        negative_evidence: negative,
        omissions,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-dry-run"),
    };
    report.digest = ContentHash::of_value(&output_body(&report))
        .map_err(|error| FederatedDryRunError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(site_id: &str) -> FederatedDryRunSiteFixture {
        FederatedDryRunSiteFixture {
            site_id: site_id.into(),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            model_system: GliomaModelSystem::Organoid,
            schema_version: "schema-v1".into(),
            synthetic_artifact_count: 2,
            projected_cost_units: 10,
            approval_required: false,
            approval_granted: true,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            declared_failure_modes: Vec::new(),
            fixture_digest: ContentHash::of_bytes(site_id.as_bytes()),
        }
    }

    fn request(fixtures: Vec<FederatedDryRunSiteFixture>) -> FederatedBenchmarkDryRunRequest {
        FederatedBenchmarkDryRunRequest {
            objective: "dry-run a glioma benchmark".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            model_system: GliomaModelSystem::Organoid,
            required_schema_version: "schema-v1".into(),
            minimum_sites: 2,
            maximum_budget_units: 100,
            require_approval: true,
            fixtures,
        }
    }

    #[test]
    fn passing_fixtures_are_ready_but_not_evidence() {
        let report = execute_federated_benchmark_dry_run(&request(vec![
            fixture("site-a"),
            fixture("site-b"),
        ]))
        .unwrap();
        assert_eq!(report.disposition, DryRunDisposition::Ready);
        assert!(report.simulation_only);
        assert_eq!(report.evidence_status, "simulation_only_non_evidence");
        assert!(report.validate().is_ok());
    }

    #[test]
    fn schema_and_binding_failures_are_accounted_for() {
        let mut bad = fixture("site-b");
        bad.schema_version = "schema-old".into();
        bad.capability_id = "other".into();
        let report =
            execute_federated_benchmark_dry_run(&request(vec![fixture("site-a"), bad])).unwrap();
        assert_eq!(report.disposition, DryRunDisposition::Blocked);
        assert!(report.failed_site_order.contains(&"site-b".into()));
        assert!(report
            .checks
            .iter()
            .any(|check| check.site_id == "site-b" && check.status == DryRunCheckStatus::Fail));
    }

    #[test]
    fn denied_approval_is_unexecuted_not_passed() {
        let mut denied = fixture("site-b");
        denied.approval_required = true;
        denied.approval_granted = false;
        let report =
            execute_federated_benchmark_dry_run(&request(vec![fixture("site-a"), denied])).unwrap();
        assert!(report.unexecuted_site_order.contains(&"site-b".into()));
        assert!(!report.approval_complete);
        assert!(report
            .omissions
            .iter()
            .any(|item| item.contains("approval")));
    }

    #[test]
    fn declared_failure_and_budget_are_negative() {
        let mut failed = fixture("site-a");
        failed.declared_failure_modes = vec!["schema-corruption".into()];
        let mut expensive = fixture("site-b");
        expensive.projected_cost_units = 200;
        let report =
            execute_federated_benchmark_dry_run(&request(vec![failed, expensive])).unwrap();
        assert!(!report.budget_satisfied);
        assert!(report
            .negative_evidence
            .iter()
            .any(|item| item.contains("cost")));
    }

    #[test]
    fn protected_fixture_is_rejected_before_simulation() {
        let mut protected = fixture("site-a");
        protected.contains_human_data = true;
        assert!(execute_federated_benchmark_dry_run(&request(vec![protected])).is_err());
    }
}
