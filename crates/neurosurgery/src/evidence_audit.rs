//! Granular specialty intake coverage for provider-free research workflows.
//!
//! This module deliberately reports observation coverage rather than interpreting observations.
//! A `measured` row means only that a caller supplied an observation with `Observed` status; it
//! does not assert validity, applicability, diagnostic identity, or clinical sufficiency.

use crate::{
    CaseRequest, EvidenceState, NeurosurgeryError, ObservationKind, ObservationStatus, Specialty,
    ToolCapability,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const EVIDENCE_AUDIT_SCHEMA_VERSION: &str = "bioprism-neurosurgery-evidence-audit/0.1";

/// One required research observation class and its caller-supplied coverage state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAuditItem {
    pub observation_kind: ObservationKind,
    pub required_for_review: bool,
    pub observed_count: usize,
    pub not_collected_count: usize,
    pub uninterpretable_count: usize,
    pub conflicting_count: usize,
    pub provenance_complete_count: usize,
    pub state: EvidenceState,
    pub reviewer_note: String,
}

/// Digest-bound, metadata-only intake coverage report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAuditReport {
    pub schema_version: String,
    /// Digest over the complete audit with this field cleared.
    #[serde(default)]
    pub audit_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub required_observation_kinds: Vec<ObservationKind>,
    pub items: Vec<EvidenceAuditItem>,
    pub missing_required_kinds: Vec<ObservationKind>,
    pub provenance_gap_count: usize,
    pub evidence_record_count: usize,
    pub verified_evidence_count: usize,
    pub unverified_evidence_count: usize,
    pub evidence_supporting_synthesis_count: usize,
    pub coverage_complete: bool,
    pub human_review_required: bool,
    pub reviewer_roles: Vec<String>,
    pub next_research_questions: Vec<String>,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    /// Nested temporal coverage for the same request. It reports date/label availability only;
    /// no interval is interpreted as progression, response, or clinical change.
    pub temporal_alignment: crate::TemporalAlignmentReport,
}

impl EvidenceAuditReport {
    /// Validate a persisted intake audit without reopening caller observation values.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != EVIDENCE_AUDIT_SCHEMA_VERSION
            || !is_sha256_hex(&self.audit_digest)
            || !is_sha256_hex(&self.request_digest)
            || self.required_observation_kinds.as_slice()
                != required_observation_kinds(self.specialty)
            || self.items.len() != self.required_observation_kinds.len()
            || self
                .missing_required_kinds
                .iter()
                .any(|kind| !self.required_observation_kinds.contains(kind))
            || self.coverage_complete
                != (self.missing_required_kinds.is_empty() && self.provenance_gap_count == 0)
            || !self.human_review_required
            || self.reviewer_roles.is_empty()
            || self
                .next_research_questions
                .iter()
                .any(|question| question.trim().is_empty())
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(audit_rejected("evidence audit envelope is invalid"));
        }
        let mut item_kinds = BTreeSet::new();
        let mut missing = Vec::new();
        let mut provenance_gaps = 0usize;
        for (expected_kind, item) in self.required_observation_kinds.iter().zip(&self.items) {
            if item.observation_kind != *expected_kind
                || !item.required_for_review
                || !item_kinds.insert(item.observation_kind)
                || item.provenance_complete_count > item.observed_count
                || item.reviewer_note.trim().is_empty()
            {
                return Err(audit_rejected(
                    "evidence audit item ordering or bounds are invalid",
                ));
            }
            let total_state_count = item
                .observed_count
                .saturating_add(item.not_collected_count)
                .saturating_add(item.uninterpretable_count)
                .saturating_add(item.conflicting_count);
            if total_state_count == 0 {
                if item.state != EvidenceState::Unmeasured {
                    return Err(audit_rejected("evidence audit empty state is invalid"));
                }
            } else if item.state
                != if item.conflicting_count > 0 {
                    EvidenceState::Conflicting
                } else if item.uninterpretable_count > 0 {
                    EvidenceState::Uninterpretable
                } else if item.observed_count > 0 {
                    EvidenceState::Measured
                } else {
                    EvidenceState::Unmeasured
                }
            {
                return Err(audit_rejected("evidence audit state projection is invalid"));
            }
            provenance_gaps = provenance_gaps.saturating_add(
                item.observed_count
                    .saturating_sub(item.provenance_complete_count),
            );
            if item.state != EvidenceState::Measured {
                missing.push(item.observation_kind);
            }
        }
        if self.missing_required_kinds != missing
            || self.provenance_gap_count != provenance_gaps
            || self
                .verified_evidence_count
                .saturating_add(self.unverified_evidence_count)
                != self.evidence_record_count
            || self.evidence_supporting_synthesis_count > self.evidence_record_count
            || self.temporal_alignment.schema_version
                != crate::temporal::TEMPORAL_ALIGNMENT_SCHEMA_VERSION
            || self.temporal_alignment.request_digest != self.request_digest
            || self.temporal_alignment.specialty != self.specialty
            || self.temporal_alignment.observation_count
                != self.temporal_alignment.observations.len()
            || self
                .temporal_alignment
                .timestamped_observation_count
                .saturating_add(self.temporal_alignment.untimestamped_observation_count)
                != self.temporal_alignment.observation_count
            || !self.temporal_alignment.human_review_required
            || self.temporal_alignment.provider != "none"
            || self.temporal_alignment.network
            || self.temporal_alignment.effect != "read_only"
            || self.audit_digest != digest_report(self)?
        {
            return Err(audit_rejected(
                "evidence audit counters or digest are invalid",
            ));
        }
        Ok(())
    }

    /// Rebuild the audit from the exact request and compare every projected state and count.
    pub fn validate_for_request(&self, request: &CaseRequest) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        if self.request_digest != digest_request(request)? || self.specialty != request.specialty {
            return Err(audit_rejected("evidence audit request binding is invalid"));
        }
        let expected = audit(request)?;
        if &expected != self {
            return Err(audit_rejected(
                "evidence audit does not replay to the exact request",
            ));
        }
        Ok(())
    }
}

/// Return the bounded research-intake observation classes for a specialty. These are review
/// expectations, not diagnostic criteria or a claim that every class is clinically necessary.
pub fn required_observation_kinds(specialty: Specialty) -> &'static [ObservationKind] {
    match specialty {
        Specialty::Glioma => &[
            ObservationKind::Histology,
            ObservationKind::Molecular,
            ObservationKind::Imaging,
            ObservationKind::Neuroanatomy,
            ObservationKind::LongitudinalOutcome,
            ObservationKind::SurgicalHistory,
        ],
        Specialty::CranialBase => &[
            ObservationKind::Imaging,
            ObservationKind::Neuroanatomy,
            ObservationKind::NeurologicFunction,
            ObservationKind::SurgicalHistory,
        ],
        Specialty::Craniosynostosis => &[
            ObservationKind::DevelopmentalTrajectory,
            ObservationKind::Imaging,
            ObservationKind::Neuroanatomy,
            ObservationKind::NeurologicFunction,
            ObservationKind::SurgicalHistory,
        ],
        Specialty::Encephalocele => &[
            ObservationKind::Imaging,
            ObservationKind::Neuroanatomy,
            ObservationKind::DevelopmentalTrajectory,
            ObservationKind::NeurologicFunction,
            ObservationKind::SurgicalHistory,
        ],
        Specialty::SpinaBifida => &[
            ObservationKind::SpinalDysraphism,
            ObservationKind::Imaging,
            ObservationKind::NeurologicFunction,
            ObservationKind::DevelopmentalTrajectory,
            ObservationKind::SurgicalHistory,
        ],
        Specialty::ChiariMalformation => &[
            ObservationKind::CraniocervicalJunction,
            ObservationKind::Imaging,
            ObservationKind::Neuroanatomy,
            ObservationKind::NeurologicFunction,
            ObservationKind::LongitudinalOutcome,
            ObservationKind::SurgicalHistory,
        ],
    }
}

/// Project caller-supplied observation coverage for any specialty lane without exposing values.
/// This is shared by the standalone audit and multi-lane evidence programs so cross-specialty
/// portfolio views use the same typed coverage semantics as the request's primary lane.
pub(crate) fn observation_audit_items(
    request: &CaseRequest,
    required: &[ObservationKind],
) -> Vec<EvidenceAuditItem> {
    required
        .iter()
        .map(|kind| {
            // The typed glioma molecular panel is the structured realization of the
            // molecular observation. Prefer its explicit marker-level coverage over
            // a duplicated free-text row so a complete panel is visible to this
            // standalone audit exactly as it is to the route's MolecularContext
            // capability. The panel has already passed request validation when this
            // function is reached through `NeurosurgicalAgent`.
            let panel_coverage = (*kind == ObservationKind::Molecular)
                .then(|| {
                    request
                        .glioma_molecular
                        .as_ref()
                        .map(|panel| panel.coverage())
                })
                .flatten();
            let observations = request
                .observations
                .iter()
                .filter(|observation| observation.kind == *kind)
                .collect::<Vec<_>>();
            let (
                observed_count,
                not_collected_count,
                uninterpretable_count,
                conflicting_count,
                provenance_complete_count,
            ) = if let Some(coverage) = panel_coverage.as_ref() {
                (
                    coverage.measured_count,
                    coverage.not_collected_count,
                    coverage.uninterpretable_count,
                    coverage.conflicting_count,
                    coverage.provenance_complete_count,
                )
            } else {
                (
                    observations
                        .iter()
                        .filter(|observation| observation.status == ObservationStatus::Observed)
                        .count(),
                    observations
                        .iter()
                        .filter(|observation| observation.status == ObservationStatus::NotCollected)
                        .count(),
                    observations
                        .iter()
                        .filter(|observation| {
                            observation.status == ObservationStatus::Uninterpretable
                        })
                        .count(),
                    observations
                        .iter()
                        .filter(|observation| observation.status == ObservationStatus::Conflicting)
                        .count(),
                    observations
                        .iter()
                        .filter(|observation| {
                            observation.status == ObservationStatus::Observed
                                && observation
                                    .source_id
                                    .as_deref()
                                    .is_some_and(|source| !source.trim().is_empty())
                        })
                        .count(),
                )
            };
            let state = if conflicting_count > 0 {
                EvidenceState::Conflicting
            } else if uninterpretable_count > 0 {
                EvidenceState::Uninterpretable
            } else if observed_count > 0 {
                EvidenceState::Measured
            } else {
                EvidenceState::Unmeasured
            };
            EvidenceAuditItem {
                observation_kind: *kind,
                required_for_review: true,
                observed_count,
                not_collected_count,
                uninterpretable_count,
                conflicting_count,
                provenance_complete_count,
                state,
                reviewer_note: format!(
                "{}{} is an intake coverage class for research review, not a diagnostic criterion",
                observation_kind_label(*kind),
                if panel_coverage.is_some() {
                    " (typed glioma molecular panel)"
                } else {
                    ""
                }
            ),
            }
        })
        .collect()
}

/// Build the audit after the caller's request has passed the agent's safety and bound checks.
pub fn audit(request: &CaseRequest) -> Result<EvidenceAuditReport, NeurosurgeryError> {
    let required = required_observation_kinds(request.specialty);
    let items = observation_audit_items(request, required);
    let mut missing_required_kinds = Vec::new();
    let mut provenance_gap_count = 0usize;
    let mut next_research_questions = Vec::new();
    for item in &items {
        provenance_gap_count = provenance_gap_count.saturating_add(
            item.observed_count
                .saturating_sub(item.provenance_complete_count),
        );
        if item.state != EvidenceState::Measured {
            missing_required_kinds.push(item.observation_kind);
            next_research_questions.push(format!(
                "Review caller-supplied {} observations and resolve the {} state before synthesis",
                observation_kind_label(item.observation_kind),
                evidence_state_label(item.state),
            ));
        } else if item.provenance_complete_count < item.observed_count {
            next_research_questions.push(format!(
                "Attach source identifiers to every observed {} record before relying on it",
                observation_kind_label(item.observation_kind)
            ));
        }
    }

    let verified_evidence_count = request
        .evidence
        .iter()
        .filter(|record| record.tier.is_verified())
        .count();
    let unverified_evidence_count = request
        .evidence
        .iter()
        .filter(|record| !record.tier.is_verified())
        .count();
    let evidence_supporting_synthesis_count = request
        .evidence
        .iter()
        .filter(|record| record.supports.contains(&ToolCapability::EvidenceSynthesis))
        .count();
    let coverage_complete = missing_required_kinds.is_empty() && provenance_gap_count == 0;
    let request_digest = digest_request(request)?;
    let mut report = EvidenceAuditReport {
        schema_version: EVIDENCE_AUDIT_SCHEMA_VERSION.to_string(),
        audit_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        required_observation_kinds: required.to_vec(),
        items,
        missing_required_kinds,
        provenance_gap_count,
        evidence_record_count: request.evidence.len(),
        verified_evidence_count,
        unverified_evidence_count,
        evidence_supporting_synthesis_count,
        coverage_complete,
        human_review_required: true,
        reviewer_roles: request.specialty.profile().human_review_roles,
        next_research_questions,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        temporal_alignment: crate::temporal::audit_temporal(request)?,
    };
    report.audit_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn digest_request(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn observation_kind_label(kind: ObservationKind) -> &'static str {
    match kind {
        ObservationKind::Imaging => "imaging",
        ObservationKind::Histology => "histology",
        ObservationKind::Molecular => "molecular",
        ObservationKind::Neuroanatomy => "neuroanatomy",
        ObservationKind::NeurologicFunction => "neurologic function",
        ObservationKind::DevelopmentalTrajectory => "developmental trajectory",
        ObservationKind::SpinalDysraphism => "spinal dysraphism",
        ObservationKind::CraniocervicalJunction => "craniocervical junction",
        ObservationKind::SurgicalHistory => "surgical history",
        ObservationKind::LongitudinalOutcome => "longitudinal outcome",
    }
}

fn evidence_state_label(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Measured => "measured",
        EvidenceState::Unmeasured => "unmeasured",
        EvidenceState::Uninterpretable => "uninterpretable",
        EvidenceState::Conflicting => "conflicting",
    }
}

fn digest_report(report: &EvidenceAuditReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.audit_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn audit_rejected(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
