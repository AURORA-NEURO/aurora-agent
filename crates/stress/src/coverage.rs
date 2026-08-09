//! Which generator answers which of the reference world's mutation families.
//!
//! Blueprint 38.01 lists six mutation families for the radiogenomic reference world and makes
//! "six mutations pass semantic validation" a vertical-slice acceptance criterion. An audit of the
//! platform found four: `bioprism-worldgen`'s `LeakageMechanism` has identity, site, temporal and
//! preprocessing members, and prevalence shift, segmentation perturbation and assay uncertainty
//! had no generator knob at all. That gap is what this crate closes, and this module is the
//! machine-checkable record of it — the claim "all six families have a generator" is a test, not a
//! sentence in a README.
//!
//! One family is shared rather than transferred. 38.01's *site-style swap preserving molecular
//! state* has two distinct readings, and both are real: the site label can predict the split, which
//! is a leakage defect and belongs to `bioprism-mutation`; or the site can shift the measurement,
//! which is a distribution shift and belongs here. Claiming sole ownership either way would leave
//! one of the two unimplemented behind a green checkmark.

use crate::family::StressFamily;
use serde::{Deserialize, Serialize};

/// A mutation family named by blueprint 38.01 for the radiogenomic reference world.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReferenceFamily {
    SiteStyleSwap,
    WrongTimeSpecimenPairing,
    DuplicateParticipantAcrossSplits,
    PrevalenceShift,
    SegmentationPerturbation,
    AssayUncertainty,
}

/// Which crate supplies the generator for a family.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum FamilyOwner {
    /// A split defect with an oracle witness. `bioprism-mutation`'s leakage mechanisms.
    LeakageMutation { mechanism: String },
    /// A change to the data-generating process. This crate.
    BiologicalStress { family: StressFamily },
    /// Two mechanisms with the same name and different physics; both are implemented.
    Shared {
        mechanism: String,
        family: StressFamily,
    },
}

impl ReferenceFamily {
    pub const ALL: [ReferenceFamily; 6] = [
        ReferenceFamily::SiteStyleSwap,
        ReferenceFamily::WrongTimeSpecimenPairing,
        ReferenceFamily::DuplicateParticipantAcrossSplits,
        ReferenceFamily::PrevalenceShift,
        ReferenceFamily::SegmentationPerturbation,
        ReferenceFamily::AssayUncertainty,
    ];

    /// The wording 38.01 uses, so the mapping can be checked against the source.
    pub fn blueprint_wording(self) -> &'static str {
        match self {
            ReferenceFamily::SiteStyleSwap => "site-style swap preserving molecular state",
            ReferenceFamily::WrongTimeSpecimenPairing => "wrong-time specimen pairing",
            ReferenceFamily::DuplicateParticipantAcrossSplits => {
                "duplicate participant across splits"
            }
            ReferenceFamily::PrevalenceShift => "prevalence shift",
            ReferenceFamily::SegmentationPerturbation => "segmentation perturbation",
            ReferenceFamily::AssayUncertainty => "assay uncertainty",
        }
    }

    pub fn owner(self) -> FamilyOwner {
        match self {
            ReferenceFamily::SiteStyleSwap => FamilyOwner::Shared {
                mechanism: "site".into(),
                family: StressFamily::BatchEffect,
            },
            ReferenceFamily::WrongTimeSpecimenPairing => {
                FamilyOwner::LeakageMutation {
                    mechanism: "temporal".into(),
                }
            }
            ReferenceFamily::DuplicateParticipantAcrossSplits => {
                FamilyOwner::LeakageMutation {
                    mechanism: "identity".into(),
                }
            }
            ReferenceFamily::PrevalenceShift => FamilyOwner::BiologicalStress {
                family: StressFamily::PrevalenceShift,
            },
            ReferenceFamily::SegmentationPerturbation => FamilyOwner::BiologicalStress {
                family: StressFamily::SegmentationJitter,
            },
            ReferenceFamily::AssayUncertainty => FamilyOwner::BiologicalStress {
                family: StressFamily::AssayDegradation,
            },
        }
    }

    /// The stress family that implements this reference family here, if any.
    pub fn stress_family(self) -> Option<StressFamily> {
        match self.owner() {
            FamilyOwner::BiologicalStress { family } | FamilyOwner::Shared { family, .. } => {
                Some(family)
            }
            FamilyOwner::LeakageMutation { .. } => None,
        }
    }
}

/// One row of the coverage table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Coverage {
    pub family: ReferenceFamily,
    pub blueprint_wording: String,
    pub owner: FamilyOwner,
}

/// The full mapping from 38.01's six families to their generators.
pub fn coverage() -> Vec<Coverage> {
    ReferenceFamily::ALL
        .into_iter()
        .map(|family| Coverage {
            family,
            blueprint_wording: family.blueprint_wording().to_string(),
            owner: family.owner(),
        })
        .collect()
}
