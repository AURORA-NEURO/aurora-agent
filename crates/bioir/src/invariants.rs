//! The non-compressible classes, as a closure check over a context projection.
//!
//! Implements blueprint 39.05. That module lists ten classes of biological semantics that a
//! context compiler "may replace ... with stable codes and graph references, but may not omit".
//! It is a closure constraint: a capsule is valid only when every class that applies is still
//! representable in what was selected.
//!
//! # What this check actually verifies
//!
//! It is **structural, not semantic**. It asks whether the projection contains a carrier for
//! each class — a lineage graph for identity, a lens catalog for units and protocol versions, a
//! cohort for split structure — not whether the content in that carrier is faithful. A
//! projection can pass this and still contain a wrong subject id. The check is therefore a
//! floor: failing it proves a class was dropped, passing it proves only that dropping it was
//! not the failure mode.
//!
//! That distinction is the reason [`RetentionReport`] separates *omitted* from
//! *unrepresentable*. Section 25 has no type for a counterfactual support boundary — that is
//! 25.08's decision cell and 25.10's translation spine — so this crate cannot close the 39.05
//! constraint by itself, and a report that quietly counted the class as satisfied would be
//! claiming otherwise.

use crate::cohort::CohortDefinition;
use crate::evidence::{EvidenceLedger, Locator};
use crate::lens::LensCatalog;
use crate::lineage::LineageGraph;
use crate::uncertainty::UncertaintyBudget;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The ten classes of 39.05, in the blueprint's order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedClass {
    /// Subject, lesion, region, specimen, aliquot, assay, artifact and model-system identity.
    Identity,
    /// Valid, collection, treatment, record, release and decision time.
    Time,
    /// Units, coordinate system, genome build, orientation and scale.
    UnitsAndReference,
    /// Platform, protocol, reagent, preprocessing, model, ontology and classifier version.
    VersionAndProtocol,
    /// Cohort inclusion/exclusion, duplicate structure, split unit and censoring rule.
    CohortStructure,
    /// Uncertainty, quality, detection limit, missingness class and failure state.
    UncertaintyAndQuality,
    /// Contradictory and negative evidence.
    ContradictoryEvidence,
    /// Privacy, consent/use restriction, data residency and role visibility.
    AccessAndConsent,
    /// The distinction between observation, interpretation, hypothesis, causal claim and
    /// recommendation.
    ClaimType,
    /// The counterfactual support boundary.
    CounterfactualSupport,
}

impl ProtectedClass {
    pub const ALL: [ProtectedClass; 10] = [
        ProtectedClass::Identity,
        ProtectedClass::Time,
        ProtectedClass::UnitsAndReference,
        ProtectedClass::VersionAndProtocol,
        ProtectedClass::CohortStructure,
        ProtectedClass::UncertaintyAndQuality,
        ProtectedClass::ContradictoryEvidence,
        ProtectedClass::AccessAndConsent,
        ProtectedClass::ClaimType,
        ProtectedClass::CounterfactualSupport,
    ];
}

impl fmt::Display for ProtectedClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            ProtectedClass::Identity => "identity",
            ProtectedClass::Time => "time",
            ProtectedClass::UnitsAndReference => "units and reference",
            ProtectedClass::VersionAndProtocol => "version and protocol",
            ProtectedClass::CohortStructure => "cohort structure",
            ProtectedClass::UncertaintyAndQuality => "uncertainty and quality",
            ProtectedClass::ContradictoryEvidence => "contradictory evidence",
            ProtectedClass::AccessAndConsent => "access and consent",
            ProtectedClass::ClaimType => "claim type",
            ProtectedClass::CounterfactualSupport => "counterfactual support",
        };
        f.write_str(name)
    }
}

/// What a caller selected to hand to a model or a reviewer.
///
/// Every field is optional because a projection is a *selection*: the whole point of the
/// closure constraint is to notice when the selection left something out.
#[derive(Debug, Default, Clone, Copy)]
pub struct ContextProjection<'a> {
    pub evidence: Option<&'a EvidenceLedger>,
    pub lineage: Option<&'a LineageGraph>,
    pub cohort: Option<&'a CohortDefinition>,
    pub lenses: Option<&'a LensCatalog>,
    pub uncertainty: Option<&'a UncertaintyBudget>,
}

/// Which protected classes survived a projection.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionReport {
    pub retained: BTreeSet<ProtectedClass>,
    /// Classes this crate could have carried and the projection did not.
    pub omitted: BTreeSet<ProtectedClass>,
    /// Classes no type in section 25's implemented modules can carry.
    ///
    /// These are not the caller's fault and cannot be fixed by selecting more evidence.
    pub unrepresentable: BTreeSet<ProtectedClass>,
}

impl RetentionReport {
    /// True when nothing representable was dropped.
    ///
    /// Deliberately ignores `unrepresentable`: a caller cannot act on a class this crate has no
    /// type for, and folding it in would make every report fail for a reason that is about the
    /// implementation rather than about the projection.
    pub fn is_closed(&self) -> bool {
        self.omitted.is_empty()
    }
}

/// Runs the 39.05 closure check over a projection.
pub fn audit(projection: &ContextProjection<'_>) -> RetentionReport {
    let mut report = RetentionReport::default();
    for class in ProtectedClass::ALL {
        match carrier_state(projection, class) {
            CarrierState::Present => report.retained.insert(class),
            CarrierState::Missing => report.omitted.insert(class),
            CarrierState::Unrepresentable => report.unrepresentable.insert(class),
        };
    }
    report
}

enum CarrierState {
    Present,
    Missing,
    Unrepresentable,
}

fn carrier_state(projection: &ContextProjection<'_>, class: ProtectedClass) -> CarrierState {
    let present = match class {
        ProtectedClass::Identity => {
            projection.lineage.is_some_and(|graph| !graph.is_empty())
                || projection.evidence.is_some_and(|ledger| {
                    ledger.iter().any(|object| {
                        object.context.subject.is_some() || object.context.specimen.is_some()
                    })
                })
        }
        ProtectedClass::Time => {
            projection.evidence.is_some_and(|ledger| {
                ledger.iter().any(|object| {
                    object.context.observed_at.is_some() || object.validity.start.is_some()
                })
            }) || projection.lineage.is_some_and(|graph| !graph.is_empty())
        }
        // A unit lives on a lens target or on a genome build in a sequence locator. Nothing
        // else in these five modules states one, so those are the only two carriers.
        ProtectedClass::UnitsAndReference => {
            projection.lenses.is_some_and(|catalog| !catalog.is_empty())
                || projection.evidence.is_some_and(|ledger| {
                    ledger.iter().any(|object| {
                        matches!(object.locator, Locator::SequenceRange { .. })
                    })
                })
        }
        ProtectedClass::VersionAndProtocol => {
            projection.lenses.is_some_and(|catalog| !catalog.is_empty())
                || projection.evidence.is_some_and(|ledger| {
                    ledger
                        .iter()
                        .any(|object| !object.provenance.parser_version.is_empty())
                })
        }
        ProtectedClass::CohortStructure => projection.cohort.is_some(),
        ProtectedClass::UncertaintyAndQuality => {
            projection
                .uncertainty
                .is_some_and(|budget| !budget.is_empty())
                || projection.evidence.is_some_and(|ledger| {
                    ledger.iter().any(|object| !object.quality.grade.is_empty())
                })
        }
        // The carrier is the relation set, not the presence of a contradiction: a ledger with
        // no contradictions is a claim that there are none, and a projection with no ledger
        // cannot make that claim either way.
        ProtectedClass::ContradictoryEvidence => projection.evidence.is_some(),
        ProtectedClass::AccessAndConsent => {
            projection.evidence.is_some_and(|ledger| {
                ledger.iter().any(|object| !object.access.labels.is_empty())
            }) || projection.lineage.is_some_and(|graph| {
                graph.iter().any(|specimen| !specimen.consent_labels.is_empty())
            })
        }
        // Modality is mandatory on every evidence object and separates expert interpretation
        // from a measured observation, which is the part of this distinction 25.11 carries.
        ProtectedClass::ClaimType => projection.evidence.is_some(),
        ProtectedClass::CounterfactualSupport => return CarrierState::Unrepresentable,
    };
    if present {
        CarrierState::Present
    } else {
        CarrierState::Missing
    }
}
