//! Digest and boundary integrity checks for the composed neurosurgical mission.
//!
//! A mission is intentionally assembled from several independent, source-bound reports. This
//! module is the final fuse: it checks that those reports still point at the same request and
//! caller-supplied real snapshots before a worker hands the envelope to a reviewer. It does not
//! score evidence, infer a diagnosis, or decide clinical readiness.

use crate::{
    CaseRequest, DicomCaseImport, FhirCaseImport, NeurosurgeryError, NeurosurgicalAgent,
    NeurosurgicalMissionResult, PublicLiteratureBundle, RealGliomaBundle,
    NEUROSURGERY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const MISSION_AUDIT_SCHEMA_VERSION: &str = "bioprism-neurosurgery-mission-audit/0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionAuditCheckStatus {
    Pass,
    Review,
    Fail,
}

/// One deterministic integrity assertion. `Review` denotes an absent optional plane; `Fail`
/// denotes an invariant mismatch that should stop automated handoff until reconciled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionAuditCheck {
    pub code: String,
    pub status: MissionAuditCheckStatus,
    pub detail: String,
}

/// Digest-bound mission integrity receipt. This is a contract audit, not a clinical quality
/// score or a substitute for qualified human review.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionAuditReport {
    pub schema_version: String,
    pub audit_digest: String,
    pub mission_id: String,
    pub request_digest: String,
    pub checks: Vec<MissionAuditCheck>,
    pub pass_count: usize,
    pub review_count: usize,
    pub fail_count: usize,
    pub integrity_ok: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl MissionAuditReport {
    /// Validate a persisted audit receipt without rebuilding the mission. This checks receipt
    /// shape and digest closure; it does not certify clinical sufficiency.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != MISSION_AUDIT_SCHEMA_VERSION
            || !is_sha256_hex(&self.audit_digest)
            || self.mission_id.trim().is_empty()
            || !is_sha256_hex(&self.request_digest)
            || self
                .pass_count
                .saturating_add(self.review_count)
                .saturating_add(self.fail_count)
                != self.checks.len()
            || self.integrity_ok != (self.fail_count == 0)
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
            || self.audit_digest != digest_report(self)?
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "mission audit receipt is invalid".to_string(),
            });
        }
        Ok(())
    }
}

impl NeurosurgicalMissionResult {
    /// Validate a persisted mission envelope before a worker hands it to another process.
    ///
    /// This is the no-input gate: it checks the mission/run/session envelope, every available
    /// nested digest receipt, and the provider-free boundary. It cannot prove that the original
    /// request or source snapshot is still the one on disk; use [`Self::validate_for_inputs`] for
    /// that exact replay check.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema != "bioprism-neurosurgical-research-mission/0.1"
            || self.mission_id.trim().is_empty()
            || self.provider != "none"
            || self.network
            || !self.human_review_required
            || self.effects.is_empty()
            || self
                .effects
                .iter()
                .any(|effect| *effect != crate::ToolEffect::ReadOnly)
            || self.catalogue.specialty_count != crate::Specialty::ALL.len()
            || self.catalogue.tool_count != crate::tool_catalogue().len()
            || self.run.schema_version != NEUROSURGERY_SCHEMA_VERSION
            || self.run.steps_executed != self.run.session.events.len()
            || self.run.response.specialty != self.specialty
            || self.run.response.status != self.status
            || self.run.response.request_digest != self.run.session.request_digest
            || self.run.response.tool_runs.len() != self.run.session.events.len()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "neurosurgical mission envelope invariants are invalid".to_string(),
            });
        }

        // Reuse the authoritative route/event-chain validator instead of maintaining a second
        // implementation here. The default agent is provider-neutral and has no side effects.
        NeurosurgicalAgent::default().validate_session_integrity(&self.run.session)?;
        self.run.response.validate_integrity()?;

        let audit =
            self.mission_audit
                .as_ref()
                .ok_or_else(|| NeurosurgeryError::RealDataRejected {
                    reason: "neurosurgical mission is missing its integrity audit".to_string(),
                })?;
        audit.validate_integrity()?;
        if !audit.integrity_ok || audit.mission_id != self.mission_id {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "neurosurgical mission integrity audit is not a passing receipt"
                    .to_string(),
            });
        }

        if let Some(report) = self.case_asset_manifest.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.case_dicom_import.as_ref() {
            report.validate_integrity()?;
            let manifest = self.case_asset_manifest.as_ref().ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: "DICOM import receipt is missing its mission asset manifest"
                        .to_string(),
                }
            })?;
            let bound = if self.case_fhir_import.is_some() {
                manifest.contains_projection(&report.manifest_report)
            } else {
                report.manifest_report == *manifest
            };
            if !bound {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "DICOM import receipt is not bound to the mission asset manifest"
                        .to_string(),
                });
            }
        }
        if let Some(report) = self.case_fhir_import.as_ref() {
            report.validate_integrity()?;
            let manifest = self.case_asset_manifest.as_ref().ok_or_else(|| {
                NeurosurgeryError::RealDataRejected {
                    reason: "FHIR import receipt is missing its mission asset manifest".to_string(),
                }
            })?;
            let bound = if self.case_dicom_import.is_some() {
                manifest.contains_projection(&report.manifest_report)
            } else {
                report.manifest_report == *manifest
            };
            if !bound {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "FHIR import receipt is not bound to the mission asset manifest"
                        .to_string(),
                });
            }
        }
        if let Some(report) = self.case_asset_review_disposition.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_query.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_query.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_coverage.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_trial_landscape.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_molecular_coverage.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_cohort_landscape.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_evidence_packet.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_autonomous_workflow.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_freshness.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_evidence_graph.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.real_data_reasoning_context.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_reasoning_context.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_evidence_packet.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_freshness.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_integrity_audit.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_review_queue.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_workbench.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.public_literature_portfolio.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.evidence_synthesis.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.research_plan.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.research_brief.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.evidence_program.as_ref() {
            report.validate_integrity()?;
        }
        if let Some(report) = self.specialty_evidence_map.as_ref() {
            report.validate_integrity()?;
        }
        Ok(())
    }

    /// Validate a persisted mission against the exact request and caller-owned snapshots.
    ///
    /// The mission audit is rebuilt deterministically and must byte-for-byte equal the stored
    /// receipt. A changed request, real-data snapshot, or PubMed snapshot therefore fails closed
    /// before a local worker can consume stale research context.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_for_inputs_with_case_imports(
            request,
            real_data,
            public_literature,
            None,
            None,
        )
    }

    /// Validate a persisted mission against the exact population snapshots and optional
    /// sanitized case metadata exports that produced it. Case import receipts intentionally do
    /// not retain source payloads, so callers must provide the original DICOM/FHIR envelope when
    /// they need end-to-end replay rather than receipt-shape validation alone.
    pub fn validate_for_inputs_with_case_imports(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_dicom_import: Option<&DicomCaseImport>,
        case_fhir_import: Option<&FhirCaseImport>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        match (self.case_dicom_import.as_ref(), case_dicom_import) {
            (Some(receipt), Some(import)) => receipt.validate_for_inputs(request, import)?,
            (Some(_), None) => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "mission contains a DICOM receipt; the original sanitized DICOM metadata is required for exact replay".to_string(),
                })
            }
            (None, Some(_)) => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "DICOM metadata was supplied for a mission without a DICOM receipt"
                            .to_string(),
                })
            }
            (None, None) => {}
        }
        match (self.case_fhir_import.as_ref(), case_fhir_import) {
            (Some(receipt), Some(import)) => receipt.validate_for_inputs(request, import)?,
            (Some(_), None) => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "mission contains a FHIR receipt; the original sanitized FHIR Bundle is required for exact replay".to_string(),
                })
            }
            (None, Some(_)) => {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason:
                        "FHIR metadata was supplied for a mission without a FHIR receipt"
                            .to_string(),
                })
            }
            (None, None) => {}
        }
        let expected = audit_mission(self, request, real_data, public_literature)?;
        if !expected.integrity_ok || self.mission_audit.as_ref() != Some(&expected) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "neurosurgical mission failed exact request/snapshot replay".to_string(),
            });
        }
        Ok(())
    }
}

/// Audit the fully assembled mission against its original request and the exact snapshots used
/// to construct it. The mission's own audit field is ignored, so adding this receipt cannot alter
/// the values it verifies.
pub fn audit_mission(
    mission: &NeurosurgicalMissionResult,
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
) -> Result<MissionAuditReport, NeurosurgeryError> {
    let request_digest = digest_json(request)?;
    let real_digest = real_data
        .map(|data| data.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let public_digest = public_literature
        .map(|literature| literature.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let mut checks = Vec::new();
    check(
        &mut checks,
        "mission_schema",
        mission.schema == "bioprism-neurosurgical-research-mission/0.1",
        "mission envelope schema is the supported neurosurgical mission contract",
    );
    check(
        &mut checks,
        "specialty_binding",
        mission.specialty == request.specialty
            && mission.run.response.specialty == request.specialty,
        "mission, request, and terminal run share one specialty lane",
    );
    check(
        &mut checks,
        "status_binding",
        mission.status == mission.run.response.status,
        "mission status equals the terminal route status",
    );
    check(
        &mut checks,
        "run_response_integrity",
        mission.run.response.validate_integrity().is_ok(),
        "terminal response carries a valid digest-bound route and tool trace",
    );
    check(
        &mut checks,
        "request_digest_binding",
        report_request_digests_match(mission, &request_digest),
        "request-bound reports use the original caller request digest",
    );
    check(
        &mut checks,
        "specialty_evidence_map_binding",
        mission.specialty_evidence_map.as_ref().is_some_and(|map| {
            map.request_digest == request_digest
                && map.specialty == request.specialty
                && map.validate_for_request(request).is_ok()
        }),
        "specialty evidence coverage is present, request-bound, and structurally valid",
    );
    check(
        &mut checks,
        "provider_boundary",
        mission.provider == "none"
            && !mission.network
            && mission.human_review_required
            && mission
                .effects
                .iter()
                .all(|effect| *effect == crate::ToolEffect::ReadOnly),
        "mission remains provider-free, network-free, read-only, and human-review gated",
    );
    check_plane_binding(
        &mut checks,
        "real_data_presence",
        real_data.is_some(),
        mission.real_data_coverage.is_some()
            && mission.real_data_trial_landscape.is_some()
            && mission.real_data_molecular_coverage.is_some()
            && mission.real_data_evidence_packet.is_some()
            && mission.real_data_autonomous_workflow.is_some()
            && mission.real_data_reasoning_context.is_some()
            && mission.evidence_program.is_some()
            && mission.evidence_acquisition.is_some(),
        "real-data missions carry coverage, packet, reasoning context, evidence program, and acquisition planes",
    );
    check(
        &mut checks,
        "case_fhir_request_binding",
        mission
            .case_fhir_import
            .as_ref()
            .is_none_or(|report| report.request_digest == request_digest),
        "FHIR import receipt remains bound to the original request",
    );
    check(
        &mut checks,
        "case_fhir_manifest_binding",
        mission
            .case_fhir_import
            .as_ref()
            .is_none_or(|report| case_manifest_binding_ok(mission, &report.manifest_report)),
        "FHIR import receipt remains bound to the mission asset manifest",
    );
    check_plane_binding(
        &mut checks,
        "public_literature_presence",
        public_literature.is_some(),
        mission.public_literature_integrity_audit.is_some()
            && mission.public_literature_evidence_packet.is_some()
            && mission.public_literature_reasoning_context.is_some()
            && mission.public_literature_workbench.is_some()
            && mission.evidence_program.is_some()
            && mission.evidence_acquisition.is_some(),
        "public-literature missions carry integrity, packet, context, workbench, evidence program, and acquisition planes",
    );
    if let Some(expected) = real_digest.as_deref() {
        if let Some(query) = mission.real_data_query.as_ref() {
            check(
                &mut checks,
                "real_data_query_integrity",
                query.bundle_digest == expected
                    && real_data.is_some_and(|data| query.validate_for_inputs(data).is_ok()),
                "real-data query remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_data_digest_binding",
            expected,
            mission
                .real_data_coverage
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data coverage remains bound to the supplied snapshot",
        );
        if let Some(coverage) = mission.real_data_coverage.as_ref() {
            check(
                &mut checks,
                "real_data_coverage_integrity",
                coverage.bundle_digest == expected
                    && real_data.is_some_and(|data| coverage.validate_for_inputs(data).is_ok()),
                "real-data coverage remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_data_trial_landscape_digest_binding",
            expected,
            mission
                .real_data_trial_landscape
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data trial landscape remains bound to the supplied snapshot",
        );
        if let Some(landscape) = mission.real_data_trial_landscape.as_ref() {
            check(
                &mut checks,
                "real_data_trial_landscape_integrity",
                landscape.bundle_digest == expected
                    && real_data.is_some_and(|data| landscape.validate_for_inputs(data).is_ok()),
                "real-data trial landscape remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_data_molecular_coverage_digest_binding",
            expected,
            mission
                .real_data_molecular_coverage
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data molecular coverage remains bound to the supplied snapshot",
        );
        if let Some(molecular) = mission.real_data_molecular_coverage.as_ref() {
            check(
                &mut checks,
                "real_data_molecular_coverage_integrity",
                molecular.bundle_digest == expected
                    && real_data
                        .is_some_and(|data| molecular.validate_for_inputs(data).is_ok()),
                "real-data molecular coverage remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_data_cohort_landscape_digest_binding",
            expected,
            mission
                .real_data_cohort_landscape
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data cohort landscape remains bound to the supplied snapshot",
        );
        if let Some(landscape) = mission.real_data_cohort_landscape.as_ref() {
            check(
                &mut checks,
                "real_data_cohort_landscape_integrity",
                landscape.bundle_digest == expected
                    && real_data.is_some_and(|data| landscape.validate_for_inputs(data).is_ok()),
                "real-data cohort landscape remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_packet_digest_binding",
            expected,
            mission
                .real_data_evidence_packet
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data packet remains bound to the supplied snapshot",
        );
        if let Some(packet) = mission.real_data_evidence_packet.as_ref() {
            check(
                &mut checks,
                "real_data_evidence_packet_integrity",
                packet.bundle_digest == expected
                    && real_data
                        .is_some_and(|data| packet.validate_for_inputs(data).is_ok()),
                "real-data evidence packet remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_reasoning_context_digest_binding",
            expected,
            mission
                .real_data_reasoning_context
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data reasoning context remains bound to the supplied snapshot",
        );
        if let Some(context) = mission.real_data_reasoning_context.as_ref() {
            check(
                &mut checks,
                "real_reasoning_context_integrity",
                context.bundle_digest == expected
                    && real_data.is_some_and(|data| context.validate_for_inputs(data).is_ok()),
                "real-data reasoning context remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_autonomous_workflow_digest_binding",
            expected,
            mission
                .real_data_autonomous_workflow
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "real-data autonomous workflow remains bound to the supplied snapshot",
        );
        if let Some(workflow) = mission.real_data_autonomous_workflow.as_ref() {
            check(
                &mut checks,
                "real_autonomous_workflow_integrity",
                workflow.bundle_digest == expected
                    && real_data
                        .is_some_and(|data| workflow.validate_for_inputs(data).is_ok()),
                "real-data autonomous workflow remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "real_program_digest_binding",
            expected,
            mission
                .evidence_program
                .as_ref()
                .and_then(|report| report.real_data_digest.as_deref()),
            "evidence program retains the supplied real-data digest",
        );
        check_digest(
            &mut checks,
            "real_acquisition_digest_binding",
            expected,
            mission
                .evidence_acquisition
                .as_ref()
                .and_then(|report| report.real_data_digest.as_deref()),
            "acquisition plan retains the supplied real-data digest",
        );
        if let Some(graph) = mission.real_data_evidence_graph.as_ref() {
            check(
                &mut checks,
                "real_data_evidence_graph_integrity",
                graph.bundle_digest == expected
                    && real_data.is_some_and(|data| graph.validate_for_inputs(data).is_ok()),
                "real-data evidence graph remains canonical and replayable against the supplied snapshot",
            );
        }
    }
    if let Some(expected) = public_digest.as_deref() {
        if let Some(query) = mission.public_literature_query.as_ref() {
            check(
                &mut checks,
                "public_literature_query_integrity",
                query.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| query.validate_for_inputs(data).is_ok()),
                "public-literature query remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "public_integrity_digest_binding",
            expected,
            mission
                .public_literature_integrity_audit
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "public-literature integrity remains bound to the supplied snapshot",
        );
        if let Some(integrity) = mission.public_literature_integrity_audit.as_ref() {
            check(
                &mut checks,
                "public_integrity_audit_integrity",
                integrity.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| integrity.validate_for_inputs(data).is_ok()),
                "public-literature integrity audit remains canonical and replayable against the supplied snapshot",
            );
        }
        if let Some(queue) = mission.public_literature_review_queue.as_ref() {
            check(
                &mut checks,
                "public_review_queue_integrity",
                queue.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| queue.validate_for_inputs(data).is_ok()),
                "public-literature review queue remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "public_packet_digest_binding",
            expected,
            mission
                .public_literature_evidence_packet
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "public-literature packet remains bound to the supplied snapshot",
        );
        if let Some(packet) = mission.public_literature_evidence_packet.as_ref() {
            check(
                &mut checks,
                "public_packet_integrity",
                packet.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| packet.validate_for_inputs(data).is_ok()),
                "public-literature packet remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "public_reasoning_context_digest_binding",
            expected,
            mission
                .public_literature_reasoning_context
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "public-literature reasoning context remains bound to the supplied snapshot",
        );
        if let Some(context) = mission.public_literature_reasoning_context.as_ref() {
            check(
                &mut checks,
                "public_reasoning_context_integrity",
                context.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| context.validate_for_inputs(data).is_ok()),
                "public-literature reasoning context remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "public_workbench_digest_binding",
            expected,
            mission
                .public_literature_workbench
                .as_ref()
                .map(|report| report.bundle_digest.as_str()),
            "public-literature workbench remains bound to the supplied snapshot",
        );
        if let Some(workbench) = mission.public_literature_workbench.as_ref() {
            check(
                &mut checks,
                "public_workbench_integrity",
                workbench.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| workbench.validate_for_inputs(data).is_ok()),
                "public-literature workbench remains canonical and replayable against the supplied snapshot",
            );
        }
        if let Some(portfolio) = mission.public_literature_portfolio.as_ref() {
            check(
                &mut checks,
                "public_portfolio_integrity",
                portfolio.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| portfolio.validate_for_inputs(data).is_ok()),
                "public-literature portfolio remains canonical and replayable against the supplied snapshot",
            );
        }
        check_digest(
            &mut checks,
            "public_program_digest_binding",
            expected,
            mission
                .evidence_program
                .as_ref()
                .and_then(|report| report.public_literature_digest.as_deref()),
            "evidence program retains the supplied public-literature digest",
        );
        check_digest(
            &mut checks,
            "public_acquisition_digest_binding",
            expected,
            mission
                .evidence_acquisition
                .as_ref()
                .and_then(|report| report.public_literature_digest.as_deref()),
            "acquisition plan retains the supplied public-literature digest",
        );
    }
    if let Some(program) = mission.evidence_program.as_ref() {
        check(
            &mut checks,
            "evidence_program_integrity",
            program
                .validate_for_inputs(
                    request,
                    real_data,
                    public_literature,
                    mission.case_asset_manifest.as_ref(),
                    mission.case_asset_review_disposition.as_ref(),
                )
                .is_ok(),
            "evidence program remains canonical and bound to the exact request, snapshots, and asset review state",
        );
    }
    if let Some(plan) = mission.research_plan.as_ref() {
        // A dual-bundle glioma mission keeps the research plan on the canonical real-data route;
        // the independent PubMed plane is carried by the evidence program/portfolio. The plan
        // compiler intentionally accepts one source bundle, so replay it against the source
        // whose digest the persisted plan actually records instead of passing both bundles.
        let plan_real_data = plan.real_data_digest.as_ref().and(real_data);
        let plan_public_literature = plan
            .public_literature_digest
            .as_ref()
            .and(public_literature);
        check(
            &mut checks,
            "research_plan_integrity",
            plan
                .validate_for_inputs(
                    request,
                    plan_real_data,
                    plan_public_literature,
                    plan.max_tasks,
                    plan.max_references_per_task,
                )
                .is_ok(),
            "research plan remains canonical and bound to the exact request, snapshots, and persisted bounds",
        );
    }
    if let Some(brief) = mission.research_brief.as_ref() {
        check(
            &mut checks,
            "research_brief_integrity",
            brief
                .validate_for_inputs(request, real_data, public_literature)
                .is_ok(),
            "research brief remains canonical and bound to the exact request and source snapshot",
        );
    }
    if let Some(synthesis) = mission.evidence_synthesis.as_ref() {
        check(
            &mut checks,
            "synthesis_human_review",
            synthesis.human_review_required && synthesis.provider == "none" && !synthesis.network,
            "evidence synthesis retains the provider-free human-review boundary",
        );
        check(
            &mut checks,
            "evidence_synthesis_integrity",
            synthesis
                .validate_for_inputs(
                    request,
                    real_data,
                    public_literature,
                    mission.case_asset_manifest.as_ref(),
                    mission.case_asset_review_disposition.as_ref(),
                )
                .is_ok(),
            "evidence synthesis remains canonical and bound to the exact request, snapshots, and asset review state",
        );
        if let Some(expected) = real_digest.as_deref() {
            if let Some(freshness) = mission.real_data_freshness.as_ref() {
                check(
                    &mut checks,
                    "real_freshness_integrity",
                    freshness.bundle_digest == expected
                        && real_data
                            .is_some_and(|data| freshness.validate_for_real_inputs(data).is_ok()),
                    "real-data freshness posture remains canonical and replayable against the supplied snapshot",
                );
            }
            check_digest(
                &mut checks,
                "synthesis_real_digest_binding",
                expected,
                synthesis
                    .real_data_summary
                    .as_ref()
                    .map(|summary| summary.bundle_digest.as_str()),
                "synthesis real-data summary remains bound to the supplied snapshot",
            );
        }
        if let Some(expected) = public_digest.as_deref() {
            if let Some(freshness) = mission.public_literature_freshness.as_ref() {
                check(
                &mut checks,
                "public_freshness_integrity",
                freshness.bundle_digest == expected
                    && public_literature
                        .is_some_and(|data| freshness.validate_for_public_inputs(data).is_ok()),
                "public-literature freshness posture remains canonical and replayable against the supplied snapshot",
            );
            }
            check_digest(
                &mut checks,
                "synthesis_public_digest_binding",
                expected,
                synthesis
                    .public_literature_summary
                    .as_ref()
                    .map(|summary| summary.bundle_digest.as_str()),
                "synthesis public-literature summary remains bound to the supplied snapshot",
            );
        }
        let molecular_expected =
            request.specialty == crate::Specialty::Glioma && request.glioma_molecular.is_some();
        check(
            &mut checks,
            "glioma_molecular_map_presence",
            synthesis.glioma_molecular_map.is_some() == molecular_expected,
            "typed glioma molecular requests carry exactly one marker-grounding map",
        );
        if let Some(map) = synthesis.glioma_molecular_map.as_ref() {
            check(
                &mut checks,
                "glioma_molecular_map_integrity",
                map.validate_for_inputs(request, real_data, public_literature)
                    .is_ok(),
                "glioma molecular marker grounding remains bound to the exact request and snapshots",
            );
        }
    } else {
        checks.push(MissionAuditCheck {
            code: "synthesis_presence".to_string(),
            status: MissionAuditCheckStatus::Fail,
            detail: "composed mission is missing its evidence synthesis ledger".to_string(),
        });
    }
    match (
        mission.case_dicom_import.as_ref(),
        mission.case_asset_manifest.as_ref(),
    ) {
        (Some(dicom), Some(_manifest)) => {
            check(
                &mut checks,
                "case_dicom_import_integrity",
                dicom.validate_integrity().is_ok(),
                "DICOM metadata import receipt has a valid digest-bound envelope",
            );
            check(
                &mut checks,
                "case_dicom_request_binding",
                dicom.request_digest == request_digest
                    && dicom.specialty == request.specialty
                    && dicom.manifest_report.request_digest == request_digest
                    && dicom.manifest_report.specialty == request.specialty,
                "DICOM metadata import remains bound to the original request and specialty lane",
            );
            check(
                &mut checks,
                "case_dicom_manifest_binding",
                case_manifest_binding_ok(mission, &dicom.manifest_report),
                "DICOM metadata import remains bound to the mission asset manifest projection",
            );
        }
        (Some(_), None) => checks.push(MissionAuditCheck {
            code: "case_dicom_without_manifest".to_string(),
            status: MissionAuditCheckStatus::Fail,
            detail: "a DICOM import receipt cannot be carried without its asset manifest"
                .to_string(),
        }),
        (None, _) => {}
    }
    if let (Some(asset_manifest), Some(synthesis)) = (
        mission.case_asset_manifest.as_ref(),
        mission.evidence_synthesis.as_ref(),
    ) {
        check(
            &mut checks,
            "case_asset_digest_binding",
            synthesis.case_asset_report_digest.as_deref()
                == Some(asset_manifest.report_digest.as_str()),
            "evidence synthesis asset summary remains bound to the manifest projection",
        );
    } else if mission.case_asset_manifest.is_some() {
        checks.push(MissionAuditCheck {
            code: "case_asset_synthesis_presence".to_string(),
            status: MissionAuditCheckStatus::Fail,
            detail: "case-asset manifest is present without a synthesis ledger".to_string(),
        });
    }
    match (
        mission.case_asset_manifest.as_ref(),
        mission.case_asset_review_disposition.as_ref(),
    ) {
        (Some(asset_manifest), Some(disposition)) => {
            check(
                &mut checks,
                "case_asset_disposition_integrity",
                disposition.validate_integrity().is_ok(),
                "persisted case-asset reviewer state has a valid envelope and digest",
            );
            check(
                &mut checks,
                "case_asset_disposition_report_binding",
                disposition.report_digest == asset_manifest.report_digest
                    && disposition.returned_item_count == asset_manifest.review_items.len()
                    && disposition.omitted_item_count == asset_manifest.omitted_review_item_count,
                "case-asset reviewer state remains bound to the emitted manifest projection",
            );
            check(
                &mut checks,
                "case_asset_disposition_synthesis_binding",
                mission
                    .evidence_synthesis
                    .as_ref()
                    .is_some_and(|synthesis| {
                        synthesis.case_asset_review_disposition_digest.as_deref()
                            == Some(disposition.disposition_digest.as_str())
                            && synthesis.case_asset_review_pending_item_count
                                == Some(disposition.pending_item_count)
                    }),
                "evidence synthesis carries the same disposition digest and pending count",
            );
            check(
                &mut checks,
                "case_asset_disposition_program_binding",
                mission.evidence_program.as_ref().is_some_and(|program| {
                    program.case_asset_review_disposition_digest.as_deref()
                        == Some(disposition.disposition_digest.as_str())
                        && program.case_asset_review_pending_item_count
                            == Some(disposition.pending_item_count)
                }),
                "evidence program carries the same disposition digest and pending count",
            );
            check(
                &mut checks,
                "case_asset_disposition_acquisition_binding",
                mission
                    .evidence_acquisition
                    .as_ref()
                    .is_some_and(|acquisition| {
                        acquisition.case_asset_review_disposition_digest.as_deref()
                            == Some(disposition.disposition_digest.as_str())
                            && acquisition.case_asset_review_pending_item_count
                                == Some(disposition.pending_item_count)
                    }),
                "acquisition plan carries the same disposition digest and pending count",
            );
            check(
                &mut checks,
                "case_asset_disposition_session_binding",
                mission
                    .evidence_acquisition_session
                    .as_ref()
                    .is_some_and(|session| {
                        session.case_asset_review_disposition_digest.as_deref()
                            == Some(disposition.disposition_digest.as_str())
                    }),
                "acquisition session carries the same disposition digest",
            );
        }
        (None, Some(_)) => checks.push(MissionAuditCheck {
            code: "case_asset_disposition_without_manifest".to_string(),
            status: MissionAuditCheckStatus::Fail,
            detail:
                "a case-asset disposition ledger cannot be used without its manifest projection"
                    .to_string(),
        }),
        (Some(_), None) => {}
        (None, None) => {}
    }
    if let Some(asset_manifest) = mission.case_asset_manifest.as_ref() {
        check(
            &mut checks,
            "case_asset_program_binding",
            program_asset_binding_ok(mission, asset_manifest),
            "evidence-program asset coverage remains bound to the manifest projection",
        );
        check(
            &mut checks,
            "case_asset_acquisition_binding",
            mission
                .evidence_acquisition
                .as_ref()
                .and_then(|report| report.case_asset_report_digest.as_deref())
                == Some(asset_manifest.report_digest.as_str()),
            "acquisition work remains bound to the manifest review projection",
        );
    } else {
        check(
            &mut checks,
            "case_asset_program_absence",
            program_has_no_asset_projection(mission),
            "evidence program does not claim asset coverage when no manifest was supplied",
        );
        check(
            &mut checks,
            "case_asset_acquisition_absence",
            mission
                .evidence_acquisition
                .as_ref()
                .is_some_and(|report| report.case_asset_report_digest.is_none()),
            "acquisition work does not claim asset coverage when no manifest was supplied",
        );
    }

    let pass_count = checks
        .iter()
        .filter(|check| check.status == MissionAuditCheckStatus::Pass)
        .count();
    let review_count = checks
        .iter()
        .filter(|check| check.status == MissionAuditCheckStatus::Review)
        .count();
    let fail_count = checks
        .iter()
        .filter(|check| check.status == MissionAuditCheckStatus::Fail)
        .count();
    let mut report = MissionAuditReport {
        schema_version: MISSION_AUDIT_SCHEMA_VERSION.to_string(),
        audit_digest: String::new(),
        mission_id: mission.mission_id.clone(),
        request_digest,
        checks,
        pass_count,
        review_count,
        fail_count,
        integrity_ok: fail_count == 0,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "checks validate contract identity and provenance binding only; they do not grade evidence or establish clinical sufficiency".to_string(),
            "review statuses identify absent optional planes; they never represent negative evidence or urgency".to_string(),
            "the audit never fetches sources, invokes a model, opens credentials, reads patient files, or emits clinical action".to_string(),
        ],
    };
    report.audit_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn check(checks: &mut Vec<MissionAuditCheck>, code: &str, passed: bool, detail: &str) {
    checks.push(MissionAuditCheck {
        code: code.to_string(),
        status: if passed {
            MissionAuditCheckStatus::Pass
        } else {
            MissionAuditCheckStatus::Fail
        },
        detail: detail.to_string(),
    });
}

fn check_plane_binding(
    checks: &mut Vec<MissionAuditCheck>,
    code: &str,
    supplied: bool,
    present: bool,
    detail: &str,
) {
    checks.push(MissionAuditCheck {
        code: code.to_string(),
        status: if supplied == present {
            MissionAuditCheckStatus::Pass
        } else {
            MissionAuditCheckStatus::Fail
        },
        detail: if !supplied {
            format!("{detail}; plane not supplied for this mission")
        } else {
            detail.to_string()
        },
    });
}

fn check_digest(
    checks: &mut Vec<MissionAuditCheck>,
    code: &str,
    expected: &str,
    actual: Option<&str>,
    detail: &str,
) {
    checks.push(MissionAuditCheck {
        code: code.to_string(),
        status: if actual == Some(expected) {
            MissionAuditCheckStatus::Pass
        } else {
            MissionAuditCheckStatus::Fail
        },
        detail: detail.to_string(),
    });
}

fn report_request_digests_match(mission: &NeurosurgicalMissionResult, expected: &str) -> bool {
    mission
        .evidence_synthesis
        .as_ref()
        .is_some_and(|report| report.request_digest == expected)
        && mission
            .research_plan
            .as_ref()
            .is_some_and(|report| report.request_digest == expected)
        && mission
            .evidence_program
            .as_ref()
            .is_some_and(|report| report.request_digest == expected)
        && mission
            .evidence_acquisition
            .as_ref()
            .is_some_and(|report| report.request_digest == expected)
        && mission
            .specialty_evidence_map
            .as_ref()
            .is_some_and(|report| report.request_digest == expected)
        && mission.case_dicom_import.as_ref().is_none_or(|report| {
            report.request_digest == expected && report.manifest_report.request_digest == expected
        })
}

fn case_manifest_binding_ok(
    mission: &NeurosurgicalMissionResult,
    child: &crate::CaseAssetManifestReport,
) -> bool {
    let Some(manifest) = mission.case_asset_manifest.as_ref() else {
        return false;
    };
    if mission.case_dicom_import.is_some() && mission.case_fhir_import.is_some() {
        manifest.contains_projection(child)
    } else {
        manifest == child
    }
}

fn program_asset_binding_ok(
    mission: &NeurosurgicalMissionResult,
    manifest: &crate::CaseAssetManifestReport,
) -> bool {
    let Some(program) = mission.evidence_program.as_ref() else {
        return false;
    };
    let expected = manifest
        .coverage
        .iter()
        .map(|coverage| {
            (
                coverage.kind,
                (
                    coverage.total_count,
                    coverage.observed_count,
                    coverage.provenance_complete_count,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    program
        .lanes
        .iter()
        .flat_map(|lane| lane.tracks.iter())
        .all(|track| {
            let Some(rows) = track.asset_coverage.as_ref() else {
                return false;
            };
            let derived_missing = rows
                .iter()
                .filter(|row| row.state == crate::EvidenceProgramAssetCoverageState::Missing)
                .map(|row| row.asset_kind)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if derived_missing != track.missing_asset_kinds
                || track.asset_coverage_complete != Some(derived_missing.is_empty())
            {
                return false;
            }
            rows.iter().all(|row| {
                expected.get(&row.asset_kind).is_some_and(|counts| {
                    counts
                        == &(
                            row.total_count,
                            row.observed_count,
                            row.provenance_complete_count,
                        )
                })
            })
        })
}

fn program_has_no_asset_projection(mission: &NeurosurgicalMissionResult) -> bool {
    mission.evidence_program.as_ref().is_some_and(|program| {
        program
            .lanes
            .iter()
            .flat_map(|lane| lane.tracks.iter())
            .all(|track| {
                track.asset_coverage.is_none()
                    && track.missing_asset_kinds.is_empty()
                    && track.asset_coverage_complete.is_none()
            })
    })
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_report(report: &MissionAuditReport) -> Result<String, NeurosurgeryError> {
    let mut copy = report.clone();
    copy.audit_digest.clear();
    digest_json(&copy)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
