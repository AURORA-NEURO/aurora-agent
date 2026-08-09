//! Factory quality control and independent audit.
//!
//! Blueprint 35.18: "audit generators, oracles, lineage, statistics, data rights, and claims before
//! public release." The eight quality gates are identical in every 35 module — they are the
//! section's shared release checklist — so they live here once as [`QualityGate`] rather than being
//! restated per module.
//!
//! ## Two things this refuses
//!
//! **Self-audit.** 35.18's last required component is "independent reproduction", and gate 8 is "a
//! second machine or site reproduces the release candidate". An auditor whose id matches the
//! producer's is [`AuditError::SelfAudit`]. It is a crude check — an id string is not an identity —
//! but it makes the intent unambiguous and catches the common case, which is a pipeline auditing
//! its own output because that was the convenient wiring.
//!
//! **An unevaluated gate.** A gate with no recorded outcome is not a passed gate; it is
//! [`AuditError::GateUnevaluated`]. This is the same distinction `AGENTS.md` insists on between
//! "provably cannot matter" and "nobody checked", applied to release readiness.
//!
//! ## Not implemented
//!
//! No conformance suite execution, no statistical audit, no privacy review. All three are 35.18
//! components and all three are other crates' work; what is here is the ledger that says which of
//! them were done, by whom, and what they found.

use crate::error::AuditError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The eight gates every section 35 module repeats verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityGate {
    /// The world replays from a clean environment.
    CleanReplay,
    /// Decision boundaries are meaningful to a domain reviewer.
    MeaningfulBoundaries,
    /// At least one non-LLM oracle covers the primary defect where feasible.
    NonLlmOracle,
    /// Mutations have executable semantic relations.
    ExecutableRelations,
    /// Duplicate and contamination scans pass.
    DuplicateAndContaminationScans,
    /// Parent-clustered statistical analysis is available.
    ParentClusteredStatistics,
    /// Data license and access policy permit the selected release mode.
    RightsPermitReleaseMode,
    /// A second machine or site reproduces the release candidate.
    IndependentReproduction,
}

impl QualityGate {
    pub const ALL: [QualityGate; 8] = [
        QualityGate::CleanReplay,
        QualityGate::MeaningfulBoundaries,
        QualityGate::NonLlmOracle,
        QualityGate::ExecutableRelations,
        QualityGate::DuplicateAndContaminationScans,
        QualityGate::ParentClusteredStatistics,
        QualityGate::RightsPermitReleaseMode,
        QualityGate::IndependentReproduction,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            QualityGate::CleanReplay => "clean_replay",
            QualityGate::MeaningfulBoundaries => "meaningful_boundaries",
            QualityGate::NonLlmOracle => "non_llm_oracle",
            QualityGate::ExecutableRelations => "executable_relations",
            QualityGate::DuplicateAndContaminationScans => "duplicate_and_contamination_scans",
            QualityGate::ParentClusteredStatistics => "parent_clustered_statistics",
            QualityGate::RightsPermitReleaseMode => "rights_permit_release_mode",
            QualityGate::IndependentReproduction => "independent_reproduction",
        }
    }
}

/// Who is auditing. An id string, not an identity — see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Auditor {
    pub id: String,
}

impl Auditor {
    pub fn new(id: impl Into<String>) -> Self {
        Auditor { id: id.into() }
    }
}

/// One gate's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutcome {
    pub gate: QualityGate,
    pub passed: bool,
    pub finding: String,
}

/// An audit in progress, bound to the artefact and its producer.
#[derive(Debug, Clone)]
pub struct ReleaseAudit {
    artefact: String,
    produced_by: String,
    auditor: Auditor,
    outcomes: BTreeMap<QualityGate, GateOutcome>,
}

impl ReleaseAudit {
    /// Opens an audit, refusing an auditor who produced the artefact.
    pub fn open(
        artefact: impl Into<String>,
        produced_by: impl Into<String>,
        auditor: Auditor,
    ) -> Result<Self, AuditError> {
        let produced_by = produced_by.into();
        if produced_by == auditor.id {
            return Err(AuditError::SelfAudit { auditor: auditor.id });
        }
        Ok(ReleaseAudit {
            artefact: artefact.into(),
            produced_by,
            auditor,
            outcomes: BTreeMap::new(),
        })
    }

    pub fn record(&mut self, gate: QualityGate, passed: bool, finding: impl Into<String>) -> &mut Self {
        self.outcomes.insert(
            gate,
            GateOutcome {
                gate,
                passed,
                finding: finding.into(),
            },
        );
        self
    }

    /// Finalizes the audit, requiring every gate to have been evaluated.
    pub fn finish(self) -> Result<AuditReport, AuditError> {
        for gate in QualityGate::ALL {
            if !self.outcomes.contains_key(&gate) {
                return Err(AuditError::GateUnevaluated {
                    gate: gate.as_str().to_string(),
                });
            }
        }
        let outcomes: Vec<GateOutcome> = self.outcomes.into_values().collect();
        let failed: Vec<&GateOutcome> = outcomes.iter().filter(|outcome| !outcome.passed).collect();
        let blocking = if failed.is_empty() {
            None
        } else {
            Some(AuditError::ReleaseBlocked {
                failed: failed.len(),
                names: failed
                    .iter()
                    .map(|outcome| outcome.gate.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            })
        };
        Ok(AuditReport {
            artefact: self.artefact,
            produced_by: self.produced_by,
            auditor: self.auditor.id,
            release_blocking_defects: failed.len(),
            blocking_reason: blocking.map(|error| error.to_string()),
            outcomes,
        })
    }
}

/// The finished audit. `release_blocking_defects == 0` is the only publishable state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    pub artefact: String,
    pub produced_by: String,
    pub auditor: String,
    pub outcomes: Vec<GateOutcome>,
    pub release_blocking_defects: usize,
    pub blocking_reason: Option<String>,
}

impl AuditReport {
    pub fn may_release(&self) -> bool {
        self.release_blocking_defects == 0
    }
}
