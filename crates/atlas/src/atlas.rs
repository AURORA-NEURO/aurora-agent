//! The atlas itself: capability × evidence coverage.
//!
//! Implements the object blueprint section 33 names in its principle statement — "The primary
//! public object is a scorecard plus failure atlas, not a rank."
//!
//! [`AtlasBuilder`] folds evidence records into cells. Three things happen in that fold that a
//! naive count would get wrong:
//!
//! - **Trial resolution.** Several oracles may judge one trial. The highest-authority record
//!   wins, and the losers that disagreed are counted as `superseded` rather than dropped, so the
//!   03.09 invariant — "nondeterministic judgments never silently override deterministic or
//!   execution-grounded evidence" — is both enforced and visible. Two oracles of *equal*
//!   authority that disagree are not arbitrated at all; the build fails.
//! - **Empty denominators become holes.** A capability whose every trial was non-evaluable or
//!   abstained is [`crate::CapabilityCell::Unmeasured`] with the reason that says which, not a
//!   score of zero. Non-evaluable and abstained counts survive on the measurement when there is
//!   one, because 33.01 gates on "missing and abstained cases cannot disappear silently".
//! - **Generated scale is separated from independent parents.** 33.19 requires it; the
//!   measurement carries both numbers and [`crate::Measurement::effective_size`] uses only the
//!   parents.
//!
//! NOT implemented: the full stratification key of 33.01 (`system × architecture × model × pack ×
//! parent world × decision family × biological scale × modality × disease entity × site ×
//! population/time stratum × oracle tier × budget × mutation family`). This crate keys on
//! capability and carries the coordinates that change the *epistemic* status of a cell — parent
//! world, site, domain, tier, oracle. The remaining coordinates are a store schema, not an atlas
//! invariant, and inventing them here would fix a shape the store has not yet chosen.

use crate::error::AtlasError;
use crate::evidence::{
    CapabilityCell, EvidenceRecord, EvidenceTier, Measurement, MeasurementFields, OracleTier,
    TrialOutcome, UnmeasuredReason,
};
use crate::failure::FailureRecord;
use crate::ontology::{CapabilityId, CapabilityOntology};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A structural disagreement inside a built atlas.
///
/// These are not build errors. An atlas containing them is still a truthful record of what was
/// observed; what it is not is a coherent one, and the coverage report says so out loud rather
/// than letting a reader assume coherence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Inconsistency {
    /// A failure was diagnosed against a capability the atlas reports as never measured. One of
    /// the two records is wrong, and neither can be trusted until that is resolved.
    FailureAgainstUnmeasuredCapability {
        capability: String,
        failure_id: String,
    },
    /// More distinct failures were recorded than there were failing trials. Failure records and
    /// evidence records disagree about how often the system failed.
    MoreFailuresThanFailedTrials {
        capability: String,
        failures_recorded: usize,
        failed_trials: usize,
    },
    /// The taxonomy did not cover an observation. Debt against the closed mechanism set of 03.10.
    UnclassifiedFailure {
        capability: String,
        failure_id: String,
    },
    /// Reviewers did not converge and the record honestly says so. Reported so a reader does not
    /// mistake a contested diagnosis for a settled one.
    ContestedDiagnosis {
        capability: String,
        failure_id: String,
    },
}

/// Capability × evidence coverage, plus the failures diagnosed against it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Atlas {
    ontology: CapabilityOntology,
    cells: BTreeMap<CapabilityId, CapabilityCell>,
    failures: Vec<FailureRecord>,
}

impl Atlas {
    pub fn builder(ontology: CapabilityOntology) -> AtlasBuilder {
        AtlasBuilder {
            ontology,
            records: Vec::new(),
            failures: Vec::new(),
            declared: BTreeMap::new(),
        }
    }

    pub fn ontology(&self) -> &CapabilityOntology {
        &self.ontology
    }

    /// Every capability in the ontology has a cell. A capability with no cell would be a third
    /// state — neither measured nor a declared hole — and there is no such state.
    pub fn cell(&self, capability: &CapabilityId) -> Option<&CapabilityCell> {
        self.cells.get(capability)
    }

    pub fn cells(&self) -> impl Iterator<Item = (&CapabilityId, &CapabilityCell)> {
        self.cells.iter()
    }

    pub fn measured(&self) -> impl Iterator<Item = (&CapabilityId, &Measurement)> {
        self.cells
            .iter()
            .filter_map(|(id, cell)| cell.measurement().map(|m| (id, m)))
    }

    /// The holes. This iterator is the reason the atlas is not a marketing artifact.
    pub fn holes(&self) -> impl Iterator<Item = (&CapabilityId, UnmeasuredReason)> {
        self.cells
            .iter()
            .filter_map(|(id, cell)| cell.unmeasured_reason().map(|r| (id, r)))
    }

    pub fn failures(&self) -> &[FailureRecord] {
        &self.failures
    }

    pub fn failures_for<'a>(
        &'a self,
        capability: &'a CapabilityId,
    ) -> impl Iterator<Item = &'a FailureRecord> {
        self.failures
            .iter()
            .filter(move |f| &f.implicates == capability)
    }

    /// Everything the atlas knows to be internally contradictory about itself.
    pub fn inconsistencies(&self) -> Vec<Inconsistency> {
        let mut out = Vec::new();
        let mut failures_by_capability: BTreeMap<&CapabilityId, usize> = BTreeMap::new();

        for failure in &self.failures {
            *failures_by_capability.entry(&failure.implicates).or_insert(0) += 1;
            let measured = self
                .cells
                .get(&failure.implicates)
                .is_some_and(CapabilityCell::is_measured);
            if !measured {
                out.push(Inconsistency::FailureAgainstUnmeasuredCapability {
                    capability: failure.implicates.to_string(),
                    failure_id: failure.failure_id.clone(),
                });
            }
            if failure.labels.has_unclassified() {
                out.push(Inconsistency::UnclassifiedFailure {
                    capability: failure.implicates.to_string(),
                    failure_id: failure.failure_id.clone(),
                });
            }
            if failure.labels.modal().is_none() {
                out.push(Inconsistency::ContestedDiagnosis {
                    capability: failure.implicates.to_string(),
                    failure_id: failure.failure_id.clone(),
                });
            }
        }

        for (capability, recorded) in failures_by_capability {
            let failed_trials = self
                .cells
                .get(capability)
                .and_then(CapabilityCell::measurement)
                .map_or(0, Measurement::failures);
            if recorded > failed_trials {
                out.push(Inconsistency::MoreFailuresThanFailedTrials {
                    capability: capability.to_string(),
                    failures_recorded: recorded,
                    failed_trials,
                });
            }
        }
        out
    }
}

/// Accumulates evidence and failures, then validates everything at once.
///
/// Validation is deferred to [`AtlasBuilder::build`] on purpose: a record that names a capability
/// declared later in the same construction is fine, and rejecting it at insertion time would make
/// declaration order load-bearing.
#[derive(Debug, Clone)]
pub struct AtlasBuilder {
    ontology: CapabilityOntology,
    records: Vec<EvidenceRecord>,
    failures: Vec<FailureRecord>,
    declared: BTreeMap<CapabilityId, UnmeasuredReason>,
}

impl AtlasBuilder {
    pub fn evidence(mut self, record: EvidenceRecord) -> Self {
        self.records.push(record);
        self
    }

    pub fn evidence_all(mut self, records: impl IntoIterator<Item = EvidenceRecord>) -> Self {
        self.records.extend(records);
        self
    }

    pub fn failure(mut self, record: FailureRecord) -> Self {
        self.failures.push(record);
        self
    }

    /// Declares why a capability is expected to have no measurement.
    ///
    /// This is how a hole stops being an accident and becomes a stated position. Only
    /// [`UnmeasuredReason::OutOfScopeByDeclaredUse`] closes a hole for claim purposes; the others
    /// merely say something truer than "not attempted".
    pub fn declare_unmeasured(
        mut self,
        capability: CapabilityId,
        reason: UnmeasuredReason,
    ) -> Self {
        self.declared.insert(capability, reason);
        self
    }

    pub fn build(self) -> Result<Atlas, AtlasError> {
        self.ontology.validate()?;
        let version = self.ontology.version().to_string();

        for record in &self.records {
            check_version(&record.trial, &version, &record.ontology_version)?;
            self.ontology.node(&record.capability)?;
        }
        for failure in &self.failures {
            check_version(&failure.failure_id, &version, &failure.ontology_version)?;
            self.ontology.node(&failure.implicates)?;
        }
        for capability in self.declared.keys() {
            self.ontology.node(capability)?;
        }

        let mut by_capability: BTreeMap<&CapabilityId, Vec<&EvidenceRecord>> = BTreeMap::new();
        for record in &self.records {
            by_capability
                .entry(&record.capability)
                .or_default()
                .push(record);
        }

        let mut cells = BTreeMap::new();
        for capability in self.ontology.ids() {
            let cell = match by_capability.get(capability) {
                None => CapabilityCell::unmeasured(
                    self.declared
                        .get(capability)
                        .copied()
                        .unwrap_or(UnmeasuredReason::NotAttempted),
                ),
                Some(records) => fold(capability, records)?,
            };
            cells.insert(capability.clone(), cell);
        }

        Ok(Atlas {
            ontology: self.ontology,
            cells,
            failures: self.failures,
        })
    }
}

fn check_version(subject: &str, expected: &str, found: &str) -> Result<(), AtlasError> {
    if expected == found {
        return Ok(());
    }
    Err(AtlasError::OntologyVersionMismatch {
        subject: subject.to_string(),
        expected: expected.to_string(),
        found: found.to_string(),
    })
}

/// Folds one capability's records into a cell.
fn fold(
    capability: &CapabilityId,
    records: &[&EvidenceRecord],
) -> Result<CapabilityCell, AtlasError> {
    let mut winners: BTreeMap<&str, &EvidenceRecord> = BTreeMap::new();
    let mut superseded = 0usize;

    for record in records {
        match winners.get(record.trial.as_str()) {
            None => {
                winners.insert(record.trial.as_str(), record);
            }
            Some(current) => {
                if record.oracle == current.oracle {
                    if record.outcome != current.outcome {
                        return Err(AtlasError::ConflictingEvidence {
                            capability: capability.to_string(),
                            trial: record.trial.clone(),
                            oracle: record.oracle.as_str(),
                        });
                    }
                    continue;
                }
                let (kept, dropped) = if record.oracle > current.oracle {
                    (*record, *current)
                } else {
                    (*current, *record)
                };
                if kept.outcome != dropped.outcome {
                    superseded += 1;
                }
                winners.insert(record.trial.as_str(), kept);
            }
        }
    }

    let mut passes = 0usize;
    let mut failures = 0usize;
    let mut non_evaluable = 0usize;
    let mut abstained = 0usize;
    let mut deterministic_failures = 0usize;
    let mut generated_instances = 0usize;
    let mut parents: BTreeSet<&str> = BTreeSet::new();
    let mut sites: BTreeSet<&str> = BTreeSet::new();
    let mut domains: BTreeSet<&str> = BTreeSet::new();
    let mut highest_tier = EvidenceTier::UnitConformance;
    let mut strongest_oracle = OracleTier::ModelJudge;

    for record in winners.values() {
        match record.outcome {
            TrialOutcome::Pass => passes += 1,
            TrialOutcome::Fail => {
                failures += 1;
                if record.oracle.is_deterministic() {
                    deterministic_failures += 1;
                }
            }
            TrialOutcome::NonEvaluable => non_evaluable += 1,
            TrialOutcome::Abstained => abstained += 1,
        }
        if record.generated {
            generated_instances += 1;
        }
        if let Some(world) = &record.parent_world {
            parents.insert(world.as_str());
        }
        if let Some(site) = &record.site {
            sites.insert(site.as_str());
        }
        if let Some(domain) = &record.domain {
            domains.insert(domain.as_str());
        }
        highest_tier = highest_tier.max(record.tier);
        strongest_oracle = strongest_oracle.max(record.oracle);
    }

    if passes + failures == 0 {
        let reason = match (non_evaluable > 0, abstained > 0) {
            (true, false) => UnmeasuredReason::AllTrialsNonEvaluable,
            (false, true) => UnmeasuredReason::AllTrialsAbstained,
            _ => UnmeasuredReason::NoEligibleEvidence,
        };
        return Ok(CapabilityCell::unmeasured(reason));
    }

    let measurement = Measurement::try_from(MeasurementFields {
        capability: capability.clone(),
        passes,
        failures,
        non_evaluable,
        abstained,
        superseded,
        independent_parents: parents.len(),
        generated_instances,
        independent_sites: sites.len(),
        domains: domains.len(),
        highest_tier,
        strongest_oracle,
        deterministic_failures,
    })?;
    Ok(CapabilityCell::measured(measurement))
}
