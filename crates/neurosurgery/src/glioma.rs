//! Typed glioma molecular evidence with explicit assay missingness.
//!
//! The ordinary [`crate::Observation`] type remains useful for caller summaries, but molecular
//! work needs more structure than free text: an absent call is not the same thing as an unrun
//! assay, and a call without specimen/assay provenance cannot be treated as measured evidence.
//! This module is a research inventory only. It never classifies a tumour, assigns grade, or
//! recommends a treatment.

use crate::NeurosurgeryError;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Version of the typed glioma molecular-panel contract.
pub const GLIOMA_MOLECULAR_SCHEMA_VERSION: &str = "bioprism-neurosurgery-glioma-molecular/0.1";
const MAX_PANEL_OBSERVATIONS: usize = 64;
const MAX_PANEL_TEXT_BYTES: usize = 512;

/// Research markers commonly needed to keep adult diffuse-glioma evidence dimensions distinct.
/// Marker names are labels, not a diagnostic criteria table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaMarker {
    Idh1Mutation,
    Idh2Mutation,
    Codeletion1p19q,
    H3K27Alteration,
    H3G34Mutation,
    MgmtPromoterMethylation,
    TertPromoterMutation,
    EgfrAmplification,
    Chromosome7Gain10Loss,
    Cdkna2bHomozygousDeletion,
    AtrxLoss,
    Tp53Mutation,
    PtenLoss,
    BrafV600e,
    NtrkFusion,
    MismatchRepairDeficiency,
    MethylationClassifier,
    TumourMutationalBurden,
}

impl GliomaMarker {
    pub const ALL: [Self; 18] = [
        Self::Idh1Mutation,
        Self::Idh2Mutation,
        Self::Codeletion1p19q,
        Self::H3K27Alteration,
        Self::H3G34Mutation,
        Self::MgmtPromoterMethylation,
        Self::TertPromoterMutation,
        Self::EgfrAmplification,
        Self::Chromosome7Gain10Loss,
        Self::Cdkna2bHomozygousDeletion,
        Self::AtrxLoss,
        Self::Tp53Mutation,
        Self::PtenLoss,
        Self::BrafV600e,
        Self::NtrkFusion,
        Self::MismatchRepairDeficiency,
        Self::MethylationClassifier,
        Self::TumourMutationalBurden,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Idh1Mutation => "IDH1 mutation",
            Self::Idh2Mutation => "IDH2 mutation",
            Self::Codeletion1p19q => "1p/19q whole-arm codeletion",
            Self::H3K27Alteration => "H3 K27 alteration",
            Self::H3G34Mutation => "H3 G34 mutation",
            Self::MgmtPromoterMethylation => "MGMT promoter methylation",
            Self::TertPromoterMutation => "TERT promoter mutation",
            Self::EgfrAmplification => "EGFR amplification",
            Self::Chromosome7Gain10Loss => "combined chromosome 7 gain and 10 loss",
            Self::Cdkna2bHomozygousDeletion => "CDKN2A/B homozygous deletion",
            Self::AtrxLoss => "ATRX loss or inactivation",
            Self::Tp53Mutation => "TP53 mutation",
            Self::PtenLoss => "PTEN loss or inactivation",
            Self::BrafV600e => "BRAF V600E",
            Self::NtrkFusion => "NTRK fusion",
            Self::MismatchRepairDeficiency => "mismatch-repair deficiency",
            Self::MethylationClassifier => "DNA-methylation classifier result",
            Self::TumourMutationalBurden => "tumour mutational burden",
        }
    }
}

/// The state of one caller-supplied assay dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaEvidenceState {
    Present,
    Absent,
    NotCollected,
    Uninterpretable,
    Conflicting,
}

impl GliomaEvidenceState {
    pub const fn is_measured(self) -> bool {
        matches!(self, Self::Present | Self::Absent)
    }
}

/// One molecular result or explicit missingness declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMarkerObservation {
    pub marker: GliomaMarker,
    pub state: GliomaEvidenceState,
    /// Assay name/version, such as a caller's panel or sequencing workflow.
    #[serde(default)]
    pub assay: Option<String>,
    /// Specimen class/timepoint label; this must remain de-identified.
    #[serde(default)]
    pub specimen: Option<String>,
    /// Caller-owned provenance identifier, never a patient identifier.
    #[serde(default)]
    pub source_id: Option<String>,
    /// Optional UTC acquisition/release timestamp for temporal alignment.
    #[serde(default)]
    pub observed_at: Option<String>,
}

/// A bounded, typed molecular panel supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularPanel {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    #[serde(default)]
    pub observations: Vec<GliomaMarkerObservation>,
}

fn default_schema_version() -> String {
    GLIOMA_MOLECULAR_SCHEMA_VERSION.to_string()
}

/// Aggregate state used by the route before a digest is attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaMolecularCoverage {
    pub marker_count: usize,
    pub measured_count: usize,
    pub not_collected_count: usize,
    pub uninterpretable_count: usize,
    pub conflicting_count: usize,
    pub provenance_complete_count: usize,
    pub missing_provenance_count: usize,
    pub missing_assay_count: usize,
    pub missing_specimen_count: usize,
    pub assay_count: usize,
    pub specimen_count: usize,
    pub source_ids: Vec<String>,
    pub gaps: Vec<GliomaMolecularGap>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GliomaMolecularGap {
    pub marker: GliomaMarker,
    pub state: GliomaEvidenceState,
    pub reason: String,
}

/// Serializable molecular inventory attached to a neurosurgical response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMolecularSummary {
    pub schema_version: String,
    pub panel_digest: String,
    pub marker_count: usize,
    pub measured_count: usize,
    pub not_collected_count: usize,
    pub uninterpretable_count: usize,
    pub conflicting_count: usize,
    pub provenance_complete_count: usize,
    pub missing_provenance_count: usize,
    pub missing_assay_count: usize,
    pub missing_specimen_count: usize,
    pub assay_count: usize,
    pub specimen_count: usize,
    pub source_ids: Vec<String>,
    pub markers: Vec<GliomaMarkerStatus>,
    pub research_gaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaMarkerStatus {
    pub marker: GliomaMarker,
    pub state: GliomaEvidenceState,
    pub assay_present: bool,
    pub specimen_present: bool,
    pub provenance_present: bool,
    pub provenance_complete: bool,
    pub observed_at_present: bool,
}

impl Default for GliomaMolecularPanel {
    fn default() -> Self {
        Self {
            schema_version: default_schema_version(),
            observations: Vec::new(),
        }
    }
}

impl GliomaMolecularPanel {
    /// Validate bounded text, uniqueness, and timestamp/provenance shape.
    pub fn validate(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != GLIOMA_MOLECULAR_SCHEMA_VERSION {
            return Err(NeurosurgeryError::GliomaPanelRejected {
                reason: format!(
                    "unsupported schema {:?}; expected {:?}",
                    self.schema_version, GLIOMA_MOLECULAR_SCHEMA_VERSION
                ),
            });
        }
        if self.observations.len() > MAX_PANEL_OBSERVATIONS {
            return Err(NeurosurgeryError::GliomaPanelRejected {
                reason: format!(
                    "panel contains {} observations; maximum is {}",
                    self.observations.len(),
                    MAX_PANEL_OBSERVATIONS
                ),
            });
        }
        let mut markers = BTreeSet::new();
        for observation in &self.observations {
            if !markers.insert(observation.marker) {
                return Err(NeurosurgeryError::GliomaPanelRejected {
                    reason: format!("marker {:?} appears more than once", observation.marker),
                });
            }
            for (field, value) in [
                ("assay", observation.assay.as_deref()),
                ("specimen", observation.specimen.as_deref()),
                ("source_id", observation.source_id.as_deref()),
            ] {
                if let Some(value) = value {
                    validate_panel_text(value, field)?;
                }
            }
            if let Some(observed_at) = &observation.observed_at {
                validate_panel_text(observed_at, "observed_at")?;
                if !is_utc_timestamp(observed_at) {
                    return Err(NeurosurgeryError::GliomaPanelRejected {
                        reason: "observed_at must be a UTC RFC3339 timestamp".to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Compute explicit coverage/missingness without assigning a clinical meaning.
    pub fn coverage(&self) -> GliomaMolecularCoverage {
        let observations = self
            .observations
            .iter()
            .map(|observation| (observation.marker, observation))
            .collect::<BTreeMap<_, _>>();
        let mut measured_count = 0;
        let mut not_collected_count = 0;
        let mut uninterpretable_count = 0;
        let mut conflicting_count = 0;
        let mut provenance_complete_count = 0;
        let mut missing_provenance_count = 0;
        let mut missing_assay_count = 0;
        let mut missing_specimen_count = 0;
        let mut assays = BTreeSet::new();
        let mut specimens = BTreeSet::new();
        let mut source_ids = BTreeSet::new();
        let mut gaps = Vec::new();

        for marker in GliomaMarker::ALL {
            let Some(observation) = observations.get(&marker).copied() else {
                not_collected_count += 1;
                gaps.push(GliomaMolecularGap {
                    marker,
                    state: GliomaEvidenceState::NotCollected,
                    reason: format!("{} was not collected", marker.label()),
                });
                continue;
            };
            match observation.state {
                GliomaEvidenceState::Present | GliomaEvidenceState::Absent => {
                    measured_count += 1;
                    if observation.source_id.is_none() {
                        missing_provenance_count += 1;
                    } else if let Some(source_id) = &observation.source_id {
                        source_ids.insert(source_id.clone());
                    }
                    if observation.assay.is_none() {
                        missing_assay_count += 1;
                    } else if let Some(assay) = &observation.assay {
                        assays.insert(assay.clone());
                    }
                    if observation.specimen.is_none() {
                        missing_specimen_count += 1;
                    } else if let Some(specimen) = &observation.specimen {
                        specimens.insert(specimen.clone());
                    }
                    if observation.source_id.is_some()
                        && observation.assay.is_some()
                        && observation.specimen.is_some()
                    {
                        provenance_complete_count += 1;
                    }
                    if observation.source_id.is_none()
                        || observation.assay.is_none()
                        || observation.specimen.is_none()
                    {
                        gaps.push(GliomaMolecularGap {
                            marker,
                            state: observation.state,
                            reason: format!(
                                "{} has a call but is missing {} provenance field(s)",
                                marker.label(),
                                [
                                    observation.source_id.is_none().then_some("source_id"),
                                    observation.assay.is_none().then_some("assay"),
                                    observation.specimen.is_none().then_some("specimen"),
                                ]
                                .into_iter()
                                .flatten()
                                .collect::<Vec<_>>()
                                .join(", ")
                            ),
                        });
                    }
                }
                GliomaEvidenceState::NotCollected => {
                    not_collected_count += 1;
                    gaps.push(GliomaMolecularGap {
                        marker,
                        state: observation.state,
                        reason: format!("{} was explicitly not collected", marker.label()),
                    });
                }
                GliomaEvidenceState::Uninterpretable => {
                    uninterpretable_count += 1;
                    gaps.push(GliomaMolecularGap {
                        marker,
                        state: observation.state,
                        reason: format!("{} is uninterpretable", marker.label()),
                    });
                }
                GliomaEvidenceState::Conflicting => {
                    conflicting_count += 1;
                    gaps.push(GliomaMolecularGap {
                        marker,
                        state: observation.state,
                        reason: format!("{} has conflicting calls", marker.label()),
                    });
                }
            }
        }
        GliomaMolecularCoverage {
            marker_count: GliomaMarker::ALL.len(),
            measured_count,
            not_collected_count,
            uninterpretable_count,
            conflicting_count,
            provenance_complete_count,
            missing_provenance_count,
            missing_assay_count,
            missing_specimen_count,
            assay_count: assays.len(),
            specimen_count: specimens.len(),
            source_ids: source_ids.into_iter().collect(),
            gaps,
        }
    }

    /// Return a digest-bound, serializable inventory for a response.
    pub fn summary(&self) -> Result<GliomaMolecularSummary, NeurosurgeryError> {
        self.validate()?;
        let coverage = self.coverage();
        let observations = self
            .observations
            .iter()
            .map(|observation| (observation.marker, observation))
            .collect::<BTreeMap<_, _>>();
        let markers = GliomaMarker::ALL
            .into_iter()
            .map(|marker| {
                let observation = observations.get(&marker).copied();
                GliomaMarkerStatus {
                    marker,
                    state: observation
                        .map_or(GliomaEvidenceState::NotCollected, |value| value.state),
                    assay_present: observation.is_some_and(|value| value.assay.is_some()),
                    specimen_present: observation.is_some_and(|value| value.specimen.is_some()),
                    provenance_present: observation.is_some_and(|value| value.source_id.is_some()),
                    provenance_complete: observation.is_some_and(|value| {
                        value.assay.is_some()
                            && value.specimen.is_some()
                            && value.source_id.is_some()
                    }),
                    observed_at_present: observation
                        .is_some_and(|value| value.observed_at.is_some()),
                }
            })
            .collect();
        let bytes = serde_json::to_vec(self)
            .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
        Ok(GliomaMolecularSummary {
            schema_version: GLIOMA_MOLECULAR_SCHEMA_VERSION.to_string(),
            panel_digest: sha256_hex(&bytes),
            marker_count: coverage.marker_count,
            measured_count: coverage.measured_count,
            not_collected_count: coverage.not_collected_count,
            uninterpretable_count: coverage.uninterpretable_count,
            conflicting_count: coverage.conflicting_count,
            provenance_complete_count: coverage.provenance_complete_count,
            missing_provenance_count: coverage.missing_provenance_count,
            missing_assay_count: coverage.missing_assay_count,
            missing_specimen_count: coverage.missing_specimen_count,
            assay_count: coverage.assay_count,
            specimen_count: coverage.specimen_count,
            source_ids: coverage.source_ids,
            markers,
            research_gaps: coverage.gaps.iter().map(|gap| gap.reason.clone()).collect(),
        })
    }

    /// Used by the real-data gate to reject synthetic labels in caller metadata.
    pub fn contains_synthetic_marker(&self) -> bool {
        self.observations.iter().any(|observation| {
            [
                observation.assay.as_deref(),
                observation.specimen.as_deref(),
                observation.source_id.as_deref(),
                observation.observed_at.as_deref(),
            ]
            .into_iter()
            .flatten()
            .any(contains_synthetic_marker)
        })
    }
}

fn validate_panel_text(value: &str, field: &str) -> Result<(), NeurosurgeryError> {
    if value.trim().is_empty() {
        return Err(NeurosurgeryError::GliomaPanelRejected {
            reason: format!("{field} is empty"),
        });
    }
    if value.len() > MAX_PANEL_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(NeurosurgeryError::GliomaPanelRejected {
            reason: format!("{field} exceeds its safety bound"),
        });
    }
    Ok(())
}

fn contains_synthetic_marker(value: &str) -> bool {
    value.to_ascii_lowercase().contains("synthetic")
}

fn is_utc_timestamp(value: &str) -> bool {
    let bytes = value.as_bytes();
    if value.len() != 20
        || ![0usize, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let hour = (bytes[11] - b'0') * 10 + (bytes[12] - b'0');
    let minute = (bytes[14] - b'0') * 10 + (bytes[15] - b'0');
    let second = (bytes[17] - b'0') * 10 + (bytes[18] - b'0');
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    month >= 1 && day >= 1 && day <= days_in_month && hour < 24 && minute < 60 && second < 60
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
