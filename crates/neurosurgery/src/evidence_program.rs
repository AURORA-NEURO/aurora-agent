//! Deterministic, source-grounded review programs for the six neurosurgical lanes.
//!
//! A program turns the closed specialty protocol into a small set of review tracks and projects
//! each track onto exact records in caller-supplied snapshots. It is deliberately lexical and
//! transparent: a match is a retrieval observation, never a relevance score, diagnosis, outcome,
//! treatment rule, or operative instruction. The core remains offline and provider-free.

use crate::{
    CaseAssetKind, CaseAssetManifestReport, CaseRequest, NeurosurgeryError, PublicLiteratureBundle,
    PublicLiteratureQuery, RealDataFreshnessQuery, RealDataFreshnessReport, RealDataQuery,
    RealDataQueryHit, RealGliomaBundle, Specialty, NEUROSURGERY_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const EVIDENCE_PROGRAM_SCHEMA_VERSION: &str = "bioprism-neurosurgery-evidence-program/0.1";
pub const MAX_EVIDENCE_PROGRAM_SPECIALTIES: usize = 6;
pub const MAX_EVIDENCE_PROGRAM_TRACKS_PER_LANE: usize = 8;
pub const MAX_EVIDENCE_PROGRAM_REFERENCES_PER_TRACK: usize = 16;
const DEFAULT_TRACKS_PER_LANE: usize = 6;
const DEFAULT_REFERENCES_PER_TRACK: usize = 8;

fn default_tracks_per_lane() -> usize {
    DEFAULT_TRACKS_PER_LANE
}
fn default_references_per_track() -> usize {
    DEFAULT_REFERENCES_PER_TRACK
}

/// Which population/citation plane supplied a matched record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProgramSource {
    RealGliomaPopulation,
    PublicLiterature,
}

/// Bounded controls for a local, source-grounded evidence program. `specialties: None` uses the
/// request lane; an explicit list is useful for a portfolio worker and is still capped at six.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramQuery {
    #[serde(default)]
    pub specialties: Option<Vec<Specialty>>,
    #[serde(default = "default_tracks_per_lane")]
    pub max_tracks_per_lane: usize,
    #[serde(default = "default_references_per_track")]
    pub max_references_per_track: usize,
    #[serde(default)]
    pub include_abstracts: bool,
    #[serde(default)]
    pub freshness: Option<RealDataFreshnessQuery>,
}

impl Default for EvidenceProgramQuery {
    fn default() -> Self {
        Self {
            specialties: None,
            max_tracks_per_lane: default_tracks_per_lane(),
            max_references_per_track: default_references_per_track(),
            include_abstracts: false,
            freshness: None,
        }
    }
}

/// One exact source record attached to a track. `record_id` is an NCT/PMID/other stable ID from
/// the snapshot; the URI is copied from source metadata and never fetched by this crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramReference {
    pub source: EvidenceProgramSource,
    pub source_id: String,
    /// Stable source record kind when emitted by the current builder. The default preserves
    /// deserialization of older persisted programs that predate this metadata field.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub record_kind: String,
    pub record_id: String,
    pub title: String,
    pub uri: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstract_excerpt: Option<String>,
    /// Optional source metadata preserved when a matched record is a registry trial, portal
    /// study, or dated PubMed article. These fields are descriptive and never interpreted as
    /// eligibility, efficacy, safety, or patient findings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub study_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrollment_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub intervention_names: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sample_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_date: Option<String>,
}

/// Metadata-only coverage for one observation class required by a protocol track. Counts and
/// state are copied from the typed intake audit; observation values and patient identifiers are
/// never promoted into the program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramObservationCoverage {
    pub observation_kind: crate::ObservationKind,
    pub state: crate::EvidenceState,
    pub observed_count: usize,
    pub provenance_complete_count: usize,
    pub provenance_gap_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceProgramAssetCoverageState {
    Observed,
    PresentNotObserved,
    Missing,
}

/// Metadata-only mapping from a protocol observation class to the corresponding caller asset
/// class. It helps a local worker find the next real export to review without opening its bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramAssetCoverage {
    pub observation_kind: crate::ObservationKind,
    pub asset_kind: CaseAssetKind,
    pub state: EvidenceProgramAssetCoverageState,
    pub total_count: usize,
    pub observed_count: usize,
    pub provenance_complete_count: usize,
}

/// A deterministic, metadata-only next-review obligation emitted for one track. Work items
/// describe missing caller metadata or provenance, never a clinical action or interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramWorkItem {
    pub code: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_kind: Option<crate::ObservationKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_kind: Option<CaseAssetKind>,
    pub detail: String,
}

/// One protocol-defined research track projected onto the real snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramTrack {
    pub track_id: String,
    pub label: String,
    pub review_objective: String,
    pub search_terms: Vec<String>,
    pub required_observation_kinds: Vec<crate::ObservationKind>,
    pub observation_coverage: Vec<EvidenceProgramObservationCoverage>,
    pub missing_observation_kinds: Vec<crate::ObservationKind>,
    /// True only when every required track observation class is measured. This is intake
    /// coverage metadata, not a statement of diagnostic or clinical sufficiency.
    pub observation_coverage_complete: bool,
    /// True only when every measured track observation has a caller-supplied source identifier.
    pub observation_provenance_complete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_coverage: Option<Vec<EvidenceProgramAssetCoverage>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_asset_kinds: Vec<CaseAssetKind>,
    /// `None` means no real asset manifest was supplied; false is reserved for an explicitly
    /// supplied manifest whose mapped classes are not all observed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_coverage_complete: Option<bool>,
    #[serde(default)]
    pub review_worklist: Vec<EvidenceProgramWorkItem>,
    pub reviewer_roles: Vec<String>,
    /// Sum of local query matches across the controlled terms; a record matching two terms may
    /// contribute twice. References are separately de-duplicated by source and stable ID.
    pub real_match_count: usize,
    pub real_returned_count: usize,
    pub real_truncated: bool,
    /// Sum of local query matches across the controlled terms; a record matching two terms may
    /// contribute twice. References are separately de-duplicated by source and stable ID.
    pub public_match_count: usize,
    pub public_returned_count: usize,
    pub public_truncated: bool,
    pub references: Vec<EvidenceProgramReference>,
    pub reference_omitted_count: usize,
    pub human_review_required: bool,
}

/// A lane in the evidence program. Empty tracks remain present so missing coverage is visible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramLane {
    pub specialty: Specialty,
    pub tracks: Vec<EvidenceProgramTrack>,
    pub track_count: usize,
    pub non_empty_track_count: usize,
    pub empty_track_ids: Vec<String>,
}

/// Digest-bound evidence agenda for a request and one or two validated public snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceProgramReport {
    pub schema_version: String,
    pub program_digest: String,
    pub request_digest: String,
    pub generated_at: String,
    pub query: EvidenceProgramQuery,
    pub lanes: Vec<EvidenceProgramLane>,
    pub specialty_count: usize,
    pub non_empty_lane_count: usize,
    pub empty_lane_specialties: Vec<Specialty>,
    pub real_data_digest: Option<String>,
    pub public_literature_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_freshness: Option<RealDataFreshnessReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_freshness: Option<RealDataFreshnessReport>,
    /// Digest of the persisted case-asset review ledger used to gate this program. `None`
    /// preserves the legacy projection path where no disposition ledger was supplied.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_disposition_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_pending_item_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_resolved_decision_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_unresolved_decision_count: Option<usize>,
    pub total_track_count: usize,
    pub non_empty_track_count: usize,
    pub reference_count: usize,
    pub reference_omitted_count: usize,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl EvidenceProgramReport {
    /// Validate the self-contained program contract before it is persisted or handed to another
    /// worker. The digest is reproducible rather than secret-signed, so shape and canonical
    /// protocol checks are required in addition to recomputing the hash.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != EVIDENCE_PROGRAM_SCHEMA_VERSION {
            return Err(NeurosurgeryError::UnsupportedSchema {
                found: self.schema_version.clone(),
                expected: EVIDENCE_PROGRAM_SCHEMA_VERSION,
            });
        }
        if !is_sha256_hex(&self.program_digest) || !is_sha256_hex(&self.request_digest) {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program digests must be 64-character SHA-256 hex values"
                    .to_string(),
            });
        }
        if self.generated_at.trim().is_empty() {
            return Err(NeurosurgeryError::EmptyField {
                field: "evidence_program.generated_at",
            });
        }
        validate_query(&self.query)?;
        if self.lanes.is_empty() || self.lanes.len() > MAX_EVIDENCE_PROGRAM_SPECIALTIES {
            return Err(NeurosurgeryError::TooMany {
                field: "evidence_program.lanes",
                found: self.lanes.len(),
                max: MAX_EVIDENCE_PROGRAM_SPECIALTIES,
            });
        }
        if self.specialty_count != self.lanes.len()
            || self.non_empty_lane_count
                != self
                    .lanes
                    .iter()
                    .filter(|lane| lane.non_empty_track_count > 0)
                    .count()
            || self.total_track_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.track_count)
                    .sum::<usize>()
            || self.non_empty_track_count
                != self
                    .lanes
                    .iter()
                    .map(|lane| lane.non_empty_track_count)
                    .sum::<usize>()
            || self.reference_count
                != self
                    .lanes
                    .iter()
                    .flat_map(|lane| lane.tracks.iter())
                    .map(|track| track.references.len())
                    .sum::<usize>()
            || self.reference_omitted_count
                != self
                    .lanes
                    .iter()
                    .flat_map(|lane| lane.tracks.iter())
                    .map(|track| track.reference_omitted_count)
                    .sum::<usize>()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program aggregate counts are inconsistent".to_string(),
            });
        }
        let mut previous_specialty = None;
        let mut expected_empty_lanes = Vec::new();
        for lane in &self.lanes {
            if previous_specialty.is_some_and(|previous| previous >= lane.specialty) {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "evidence program lanes must be sorted and unique".to_string(),
                });
            }
            previous_specialty = Some(lane.specialty);
            let expected_specs = specs(lane.specialty);
            let expected_track_count = expected_specs.len().min(self.query.max_tracks_per_lane);
            if lane.track_count != expected_track_count || lane.tracks.len() != expected_track_count
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "evidence program track count is not canonical for {}",
                        lane.specialty.display_name()
                    ),
                });
            }
            let expected_non_empty = lane
                .tracks
                .iter()
                .filter(|track| track.real_match_count + track.public_match_count > 0)
                .count();
            if lane.non_empty_track_count != expected_non_empty
                || lane.empty_track_ids
                    != lane
                        .tracks
                        .iter()
                        .filter(|track| track.real_match_count + track.public_match_count == 0)
                        .map(|track| track.track_id.clone())
                        .collect::<Vec<_>>()
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "evidence program empty-track projection is inconsistent".to_string(),
                });
            }
            if lane.non_empty_track_count == 0 {
                expected_empty_lanes.push(lane.specialty);
            }
            for (track, spec) in lane.tracks.iter().zip(expected_specs.iter()) {
                validate_track(
                    track,
                    lane.specialty,
                    spec,
                    self.query.max_references_per_track,
                )?;
            }
        }
        if self.empty_lane_specialties != expected_empty_lanes {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program empty-lane projection is inconsistent".to_string(),
            });
        }
        if self
            .real_data_digest
            .as_deref()
            .is_some_and(|digest| !is_sha256_hex(digest))
            || self
                .public_literature_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program bundle digests must be SHA-256 hex values".to_string(),
            });
        }
        if self.real_data_freshness.as_ref().is_some_and(|freshness| {
            self.real_data_digest.as_deref() != Some(freshness.bundle_digest.as_str())
        }) || self
            .public_literature_freshness
            .as_ref()
            .is_some_and(|freshness| {
                self.public_literature_digest.as_deref() != Some(freshness.bundle_digest.as_str())
            })
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program freshness reports must bind their bundle digest"
                    .to_string(),
            });
        }
        let disposition_fields_present = self.case_asset_review_disposition_digest.is_some();
        let disposition_counts_present = self.case_asset_review_pending_item_count.is_some()
            && self.case_asset_review_resolved_decision_count.is_some()
            && self.case_asset_review_unresolved_decision_count.is_some();
        if disposition_fields_present != disposition_counts_present
            || self
                .case_asset_review_disposition_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program provider-free provenance contract is invalid".to_string(),
            });
        }
        if self.program_digest != digest_report(self)? {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program digest does not match its report contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild the program from the exact request, snapshots, asset projection, and optional
    /// reviewer disposition ledger. This prevents a validly-shaped report from being rebound to
    /// a different source snapshot or query.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        case_assets: Option<&CaseAssetManifestReport>,
        dispositions: Option<&crate::CaseAssetReviewDispositionReport>,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = match dispositions {
            Some(dispositions) => build_evidence_program_with_asset_report_and_dispositions(
                request,
                real_data,
                public_literature,
                case_assets,
                dispositions,
                &self.query,
            )?,
            None => build_evidence_program_with_asset_report(
                request,
                real_data,
                public_literature,
                case_assets,
                &self.query,
            )?,
        };
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "evidence program is not bound to the supplied request and snapshots"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct TrackSpec {
    id: &'static str,
    label: &'static str,
    objective: &'static str,
    terms: &'static [&'static str],
    observations: &'static [crate::ObservationKind],
}

const I: crate::ObservationKind = crate::ObservationKind::Imaging;
const H: crate::ObservationKind = crate::ObservationKind::Histology;
const M: crate::ObservationKind = crate::ObservationKind::Molecular;
const N: crate::ObservationKind = crate::ObservationKind::Neuroanatomy;
const F: crate::ObservationKind = crate::ObservationKind::NeurologicFunction;
const D: crate::ObservationKind = crate::ObservationKind::DevelopmentalTrajectory;
const S: crate::ObservationKind = crate::ObservationKind::SpinalDysraphism;
const C: crate::ObservationKind = crate::ObservationKind::CraniocervicalJunction;
const SH: crate::ObservationKind = crate::ObservationKind::SurgicalHistory;
const L: crate::ObservationKind = crate::ObservationKind::LongitudinalOutcome;

fn specs(specialty: Specialty) -> Vec<TrackSpec> {
    match specialty {
        Specialty::Glioma => vec![
            TrackSpec { id: "histomolecular_identity", label: "Integrated histomolecular identity", objective: "Review how the corpus characterizes molecular and histologic identity, assay scope, and specimen context", terms: &["IDH", "1p/19q", "MGMT", "H3", "EGFR", "TERT"], observations: &[H, M] },
            TrackSpec { id: "imaging_phenotype", label: "Imaging phenotype and registration", objective: "Review imaging-derived phenotype, acquisition details, and links to molecular or functional observations", terms: &["MRI", "radiogenomic", "spectroscopy", "perfusion"], observations: &[I, N] },
            TrackSpec { id: "surgery_function", label: "Surgery and function context", objective: "Review source descriptions of resection context, eloquent structures, and functional outcomes without inferring a corridor", terms: &["resection", "awake", "eloquent", "intraoperative"], observations: &[I, N, F, SH] },
            TrackSpec { id: "response_endpoints", label: "Response and longitudinal endpoints", objective: "Review endpoint definitions, progression or treatment-effect language, and follow-up windows", terms: &["temozolomide", "radiotherapy", "progression", "survival"], observations: &[L] },
            TrackSpec { id: "invasion_microenvironment", label: "Invasion and microenvironment", objective: "Review how studies describe invasion, immune context, and tumour microenvironment mechanisms", terms: &["invasion", "tumor microenvironment", "macrophage", "microglia"], observations: &[H, M] },
            TrackSpec { id: "translation_trials", label: "Translation and trial design", objective: "Review study design, eligibility, and translational boundaries before any applicability discussion", terms: &["clinical trial", "phase", "randomized", "translational"], observations: &[] },
        ],
        Specialty::CranialBase => vec![
            TrackSpec { id: "compartment_anatomy", label: "Compartment and skull-base anatomy", objective: "Review how studies delimit skull-base compartments and adjacent structures", terms: &["skull base", "clivus", "cavernous sinus", "orbit"], observations: &[I, N] },
            TrackSpec { id: "vascular_nerve_context", label: "Vascular and cranial-nerve context", objective: "Review source-reported vascular and cranial-nerve observations and their measurement limits", terms: &["cranial nerve", "carotid", "vascular", "cavernous"], observations: &[N, I] },
            TrackSpec { id: "approach_reconstruction", label: "Approach and reconstruction literature", objective: "Review descriptions of approaches, reconstruction, and postoperative anatomy as reported", terms: &["endoscopic", "endonasal", "reconstruction", "approach"], observations: &[SH, I] },
            TrackSpec { id: "functional_outcomes", label: "Functional outcomes and follow-up", objective: "Review functional endpoint definitions, complications, and follow-up windows", terms: &["visual", "complication", "outcome", "follow-up"], observations: &[F, L] },
            TrackSpec { id: "pathology_context", label: "Pathology and compartment separation", objective: "Keep pathology labels separate from anatomic location while reviewing the cited corpus", terms: &["meningioma", "chordoma", "schwannoma"], observations: &[H, N] },
            TrackSpec { id: "imaging_modalities", label: "Imaging modality coverage", objective: "Inventory modality, plane, and artifact reporting in the source records", terms: &["MRI", "CT", "angiography"], observations: &[I] },
        ],
        Specialty::Craniosynostosis => vec![
            TrackSpec { id: "suture_phenotype", label: "Suture phenotype and syndromic context", objective: "Review directly measured suture patterns and declared genetic or syndromic scope", terms: &["craniosynostosis", "suture", "syndromic", "genetic"], observations: &[D, I] },
            TrackSpec { id: "growth_shape", label: "Growth and head-shape trajectory", objective: "Review age-aligned growth, shape, and reference-standard reporting", terms: &["cranial", "cephalic", "head shape", "growth"], observations: &[D] },
            TrackSpec { id: "intracranial_venous", label: "Intracranial and venous observations", objective: "Review reported intracranial-volume, pressure, and venous findings with measurement context", terms: &["intracranial pressure", "venous", "volume", "papilledema"], observations: &[I, N] },
            TrackSpec { id: "airway_vision_development", label: "Airway, vision, and development", objective: "Review functional assessment coverage across airway, vision, hearing, and development", terms: &["airway", "sleep", "vision", "ophthalm"], observations: &[F, D] },
            TrackSpec { id: "repair_outcomes", label: "Repair and longitudinal outcomes", objective: "Review repair descriptions and age-aligned outcome or follow-up endpoints", terms: &["surgery", "remodeling", "distraction", "outcome"], observations: &[SH, L] },
            TrackSpec { id: "guideline_consensus", label: "Guideline and consensus context", objective: "Review consensus and synthesis sources separately from primary cohorts", terms: &["guideline", "consensus", "systematic review"], observations: &[] },
        ],
        Specialty::Encephalocele => vec![
            TrackSpec { id: "defect_content", label: "Defect and tissue-content characterization", objective: "Review how defect boundaries and neural or meningeal content are measured and reported", terms: &["encephalocele", "meningocele", "neural", "defect"], observations: &[I, N] },
            TrackSpec { id: "prenatal_neonatal", label: "Prenatal and neonatal course", objective: "Review prenatal, fetal, neonatal, and congenital timepoint coverage", terms: &["prenatal", "fetal", "neonatal", "congenital"], observations: &[D] },
            TrackSpec { id: "skullbase_csf", label: "Skull-base and CSF relationships", objective: "Review source-reported skull-base, sinus, vascular, and CSF relationships", terms: &["skull base", "transcranial", "endoscopic", "CSF"], observations: &[I, N] },
            TrackSpec { id: "associated_anomalies", label: "Associated anomalies", objective: "Review independently sourced associated-anomaly and developmental assessments", terms: &["hydrocephalus", "anomaly", "syndrome", "genetic"], observations: &[D, I] },
            TrackSpec { id: "repair_outcomes", label: "Repair and longitudinal outcomes", objective: "Review repair descriptions, complications, and follow-up definitions", terms: &["repair", "surgery", "outcome", "follow-up"], observations: &[SH, L] },
            TrackSpec { id: "imaging_coverage", label: "Imaging coverage", objective: "Inventory MRI, ultrasound, CT, and stated limitations in the cited records", terms: &["MRI", "ultrasound", "CT"], observations: &[I] },
        ],
        Specialty::SpinaBifida => vec![
            TrackSpec { id: "dysraphism_phenotype", label: "Dysraphism phenotype and level", objective: "Review reported phenotype, level, and neural-tissue characterization", terms: &["spina bifida", "myelomeningocele", "dysraphism", "tethered cord"], observations: &[S, I] },
            TrackSpec { id: "level_function", label: "Level and neurologic function", objective: "Review time-aligned motor, sensory, and neurologic function measurement", terms: &["motor", "sensory", "neurologic", "level"], observations: &[F, S] },
            TrackSpec { id: "prenatal_closure", label: "Prenatal and fetal intervention context", objective: "Review prenatal, fetal, and closure study design without converting it into a care recommendation", terms: &["prenatal", "fetal", "in utero", "closure"], observations: &[D, SH] },
            TrackSpec { id: "urologic_orthopedic", label: "Urologic and orthopedic function", objective: "Review bladder, bowel, urologic, orthopedic, and rehabilitation endpoint coverage", terms: &["urolog", "bladder", "bowel", "orthopedic"], observations: &[F, L] },
            TrackSpec { id: "tethering_longitudinal", label: "Tethering and longitudinal imaging", objective: "Review tethering, retethering, cord, and syrinx observations across timepoints", terms: &["tethered", "retethering", "cord", "syrinx"], observations: &[I, L] },
            TrackSpec { id: "functional_outcomes", label: "Quality of life and outcomes", objective: "Review functional outcome definitions and follow-up windows", terms: &["quality of life", "outcome", "follow-up", "functional"], observations: &[F, L] },
        ],
        Specialty::ChiariMalformation => vec![
            TrackSpec { id: "junction_phenotype", label: "Craniocervical-junction phenotype", objective: "Review measurements, reference conventions, and stated landmarks", terms: &["chiari", "tonsillar", "foramen magnum", "craniocervical"], observations: &[C, I] },
            TrackSpec { id: "csf_dynamics", label: "CSF dynamics and flow", objective: "Review CSF-space, cine, flow, and acquisition context", terms: &["cerebrospinal fluid", "CSF", "cine", "flow"], observations: &[I, C] },
            TrackSpec { id: "associated_findings", label: "Associated findings", objective: "Review independently observed syringomyelia, scoliosis, sleep, and swallowing assessments", terms: &["syringomyelia", "scoliosis", "sleep", "swallow"], observations: &[I, F] },
            TrackSpec { id: "symptom_function", label: "Symptoms and objective function", objective: "Review symptom definitions and time-aligned neurologic or functional measures", terms: &["headache", "neurologic", "pain", "function"], observations: &[F, C] },
            TrackSpec { id: "decompression_outcomes", label: "Decompression and outcomes", objective: "Review decompression descriptions, reoperation reporting, and follow-up endpoints", terms: &["decompression", "outcome", "follow-up", "reoperation"], observations: &[SH, L] },
            TrackSpec { id: "measurement_conventions", label: "Measurement conventions", objective: "Review plane, landmark, unit, and measurement-convention reporting", terms: &["measurement", "midsagittal", "mm", "landmark"], observations: &[I, C] },
        ],
    }
}

/// Build a program from the caller's validated snapshots. At least one snapshot is required;
/// glioma may use both planes, while the other lanes use the cross-specialty PubMed snapshot.
pub fn build_evidence_program(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceProgramQuery,
) -> Result<EvidenceProgramReport, NeurosurgeryError> {
    build_evidence_program_with_asset_report(request, real_data, public_literature, None, query)
}

/// Build a source-grounded program while attaching an already validated digest-only asset
/// projection. The raw manifest and its bytes remain outside this function.
pub fn build_evidence_program_with_asset_report(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_assets: Option<&CaseAssetManifestReport>,
    query: &EvidenceProgramQuery,
) -> Result<EvidenceProgramReport, NeurosurgeryError> {
    validate_query(query)?;
    validate_program_request(request)?;
    if real_data.is_none() && public_literature.is_none() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "evidence program requires a validated real-data or public-literature bundle"
                .to_string(),
        });
    }
    if let Some(data) = real_data {
        data.validate()?;
    }
    if let Some(literature) = public_literature {
        literature.validate()?;
    }
    if let Some(asset_report) = case_assets {
        asset_report.validate_for_request(request)?;
    }
    let specialties = selected_specialties(request.specialty, query)?;
    if real_data.is_some()
        && specialties
            .iter()
            .any(|specialty| *specialty != Specialty::Glioma)
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "real glioma population data can only project the glioma evidence-program lane"
                .to_string(),
        });
    }
    if request.specialty != Specialty::Glioma && real_data.is_some() && public_literature.is_none()
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "non-glioma evidence programs require the public-literature bundle".to_string(),
        });
    }
    let real_freshness = match (real_data, query.freshness.as_ref()) {
        (Some(data), Some(freshness)) => Some(data.freshness_report(freshness)?),
        _ => None,
    };
    let public_freshness = match (public_literature, query.freshness.as_ref()) {
        (Some(data), Some(freshness)) => Some(data.freshness_report(freshness)?),
        _ => None,
    };
    let mut lanes = Vec::with_capacity(specialties.len());
    for specialty in specialties.iter().copied() {
        let mut tracks = Vec::new();
        for spec in specs(specialty).into_iter().take(query.max_tracks_per_lane) {
            tracks.push(project_track(
                spec,
                specialty,
                request,
                case_assets,
                real_data,
                public_literature,
                query,
            )?);
        }
        let empty_track_ids = tracks
            .iter()
            .filter(|track| track.real_match_count + track.public_match_count == 0)
            .map(|track| track.track_id.clone())
            .collect::<Vec<_>>();
        let non_empty_track_count = tracks.len() - empty_track_ids.len();
        lanes.push(EvidenceProgramLane {
            specialty,
            track_count: tracks.len(),
            non_empty_track_count,
            empty_track_ids,
            tracks,
        });
    }
    let empty_lane_specialties = lanes
        .iter()
        .filter(|lane| lane.non_empty_track_count == 0)
        .map(|lane| lane.specialty)
        .collect::<Vec<_>>();
    let total_track_count = lanes.iter().map(|lane| lane.track_count).sum();
    let non_empty_track_count = lanes.iter().map(|lane| lane.non_empty_track_count).sum();
    let reference_count = lanes
        .iter()
        .flat_map(|lane| lane.tracks.iter())
        .map(|track| track.references.len())
        .sum();
    let reference_omitted_count = lanes
        .iter()
        .flat_map(|lane| lane.tracks.iter())
        .map(|track| track.reference_omitted_count)
        .sum();
    let request_digest = digest_json(request)?;
    let mut report = EvidenceProgramReport {
        schema_version: EVIDENCE_PROGRAM_SCHEMA_VERSION.to_string(),
        program_digest: String::new(),
        request_digest,
        generated_at: real_data.map(|data| data.generated_at.clone()).or_else(|| public_literature.map(|data| data.generated_at.clone())).unwrap_or_default(),
        query: query.clone(),
        lanes,
        specialty_count: specialties.len(),
        non_empty_lane_count: specialties.len() - empty_lane_specialties.len(),
        empty_lane_specialties,
        real_data_digest: real_data.map(|data| data.summary()).transpose()?.map(|summary| summary.bundle_digest),
        public_literature_digest: public_literature.map(|data| data.summary()).transpose()?.map(|summary| summary.bundle_digest),
        real_data_freshness: real_freshness,
        public_literature_freshness: public_freshness,
        case_asset_review_disposition_digest: None,
        case_asset_review_pending_item_count: None,
        case_asset_review_resolved_decision_count: None,
        case_asset_review_unresolved_decision_count: None,
        total_track_count,
        non_empty_track_count,
        reference_count,
        reference_omitted_count,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "track matches are lexical retrieval observations over the supplied snapshots; they are not relevance scores, evidence grades, diagnoses, prognoses, or treatment rules".to_string(),
            "empty and truncated tracks remain unknown and must be reviewed against the source authority".to_string(),
            "population and citation records are never promoted to case findings; caller observations and source applicability remain separate".to_string(),
            "the program never fetches URLs, invokes a model, opens credentials, reads patient files, or emits a clinical or operative instruction".to_string(),
        ],
    };
    report.program_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

/// Build an evidence program while binding the exact persisted case-asset review ledger used by
/// the caller. The disposition report is metadata-only; raw asset bytes and reviewer identities
/// never enter the evidence program. Exact digest/count equality prevents a stale ledger from
/// being silently paired with a different manifest projection.
pub fn build_evidence_program_with_asset_report_and_dispositions(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    case_assets: Option<&CaseAssetManifestReport>,
    dispositions: &crate::CaseAssetReviewDispositionReport,
    query: &EvidenceProgramQuery,
) -> Result<EvidenceProgramReport, NeurosurgeryError> {
    let asset_report = case_assets.ok_or_else(|| NeurosurgeryError::SessionRejected {
        reason: "case-asset dispositions require a projected case-asset manifest".to_string(),
    })?;
    dispositions.validate_integrity()?;
    let candidate_item_count = asset_report
        .review_items
        .len()
        .checked_add(asset_report.omitted_review_item_count)
        .ok_or_else(|| NeurosurgeryError::RealDataRejected {
            reason: "case-asset review candidate count overflows its bound".to_string(),
        })?;
    if dispositions.report_digest != asset_report.report_digest
        || dispositions.returned_item_count != asset_report.review_items.len()
        || dispositions.omitted_item_count != asset_report.omitted_review_item_count
        || dispositions.candidate_item_count != candidate_item_count
    {
        return Err(NeurosurgeryError::SessionRejected {
            reason: "disposition ledger does not bind to the supplied case-asset projection"
                .to_string(),
        });
    }
    let mut report = build_evidence_program_with_asset_report(
        request,
        real_data,
        public_literature,
        Some(asset_report),
        query,
    )?;
    report.case_asset_review_disposition_digest = Some(dispositions.disposition_digest.clone());
    report.case_asset_review_pending_item_count = Some(dispositions.pending_item_count);
    report.case_asset_review_resolved_decision_count = Some(dispositions.resolved_decision_count);
    report.case_asset_review_unresolved_decision_count =
        Some(dispositions.unresolved_decision_count);
    report.program_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

/// Keep the public builder fail-closed even when called without the higher-level agent facade.
/// The MCP and SDK paths also validate through `NeurosurgicalAgent`, but this boundary prevents a
/// direct library caller from turning a source agenda into a clinical or identified workflow.
fn validate_program_request(request: &CaseRequest) -> Result<(), NeurosurgeryError> {
    if request.schema_version != NEUROSURGERY_SCHEMA_VERSION {
        return Err(NeurosurgeryError::UnsupportedSchema {
            found: request.schema_version.clone(),
            expected: NEUROSURGERY_SCHEMA_VERSION,
        });
    }
    if request.request_use.is_clinical() {
        return Err(NeurosurgeryError::ClinicalUseRefused {
            use_case: request.request_use,
            description: request.request_use.description(),
        });
    }
    if !request.direct_identifier_fields.is_empty() {
        return Err(NeurosurgeryError::DirectIdentifiers {
            fields: request.direct_identifier_fields.clone(),
        });
    }
    if request.request_use == crate::RequestUse::SyntheticCaseSimulation {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "evidence program cannot run for synthetic_case_simulation".to_string(),
        });
    }
    Ok(())
}

fn validate_track(
    track: &EvidenceProgramTrack,
    specialty: Specialty,
    spec: &TrackSpec,
    max_references: usize,
) -> Result<(), NeurosurgeryError> {
    if track.track_id != spec.id
        || track.label != spec.label
        || track.review_objective != spec.objective
        || track.search_terms
            != spec
                .terms
                .iter()
                .map(|term| (*term).to_string())
                .collect::<Vec<_>>()
        || track.required_observation_kinds != spec.observations
        || track
            .observation_coverage
            .iter()
            .map(|coverage| coverage.observation_kind)
            .ne(spec.observations.iter().copied())
        || track.reviewer_roles != specialty.profile().human_review_roles
        || track.references.len() > max_references
        || track.real_returned_count > track.real_match_count
        || track.public_returned_count > track.public_match_count
        || !track.human_review_required
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!("evidence program track {} is not canonical", track.track_id),
        });
    }
    if track.observation_coverage.len() != spec.observations.len()
        || track.missing_observation_kinds
            != track
                .observation_coverage
                .iter()
                .filter(|coverage| coverage.state != crate::EvidenceState::Measured)
                .map(|coverage| coverage.observation_kind)
                .collect::<Vec<_>>()
        || track.observation_coverage_complete != track.missing_observation_kinds.is_empty()
        || track.observation_provenance_complete
            != track
                .observation_coverage
                .iter()
                .all(|coverage| coverage.provenance_complete_count == coverage.observed_count)
        || track.review_worklist
            != build_review_worklist(&track.observation_coverage, track.asset_coverage.as_deref())
    {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: format!(
                "evidence program observation coverage is invalid for {}",
                track.track_id
            ),
        });
    }
    for coverage in &track.observation_coverage {
        if coverage.observed_count < coverage.provenance_complete_count
            || coverage.provenance_gap_count
                != coverage.observed_count - coverage.provenance_complete_count
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "evidence program provenance counts are invalid for {}",
                    track.track_id
                ),
            });
        }
    }
    match (&track.asset_coverage, track.asset_coverage_complete) {
        (None, None) if track.missing_asset_kinds.is_empty() => {}
        (Some(rows), Some(complete)) => {
            let mut seen_kinds = BTreeSet::new();
            let mut expected_missing = BTreeSet::new();
            for row in rows {
                if !seen_kinds.insert(row.observation_kind)
                    || row.observed_count > row.total_count
                    || row.provenance_complete_count > row.observed_count
                    || (row.state == EvidenceProgramAssetCoverageState::Missing
                        && row.total_count != 0)
                    || (row.state == EvidenceProgramAssetCoverageState::PresentNotObserved
                        && (row.total_count == 0 || row.observed_count != 0))
                    || (row.state == EvidenceProgramAssetCoverageState::Observed
                        && row.observed_count == 0)
                {
                    return Err(NeurosurgeryError::RealDataRejected {
                        reason: format!(
                            "evidence program asset coverage is invalid for {}",
                            track.track_id
                        ),
                    });
                }
                if row.state == EvidenceProgramAssetCoverageState::Missing {
                    expected_missing.insert(row.asset_kind);
                }
            }
            if track.missing_asset_kinds != expected_missing.into_iter().collect::<Vec<_>>()
                || complete != track.missing_asset_kinds.is_empty()
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: format!(
                        "evidence program asset completeness is invalid for {}",
                        track.track_id
                    ),
                });
            }
        }
        _ => {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "evidence program asset projection is invalid for {}",
                    track.track_id
                ),
            });
        }
    }
    let mut seen = BTreeSet::new();
    for reference in &track.references {
        if reference.source_id.trim().is_empty()
            || (!reference.record_kind.is_empty() && reference.record_kind.trim().is_empty())
            || reference.record_id.trim().is_empty()
            || reference.title.trim().is_empty()
            || reference.uri.trim().is_empty()
            || reference.phases.len() > 16
            || reference.intervention_names.len() > 128
            || reference.phases.iter().any(|phase| phase.trim().is_empty())
            || reference
                .intervention_names
                .iter()
                .any(|name| name.trim().is_empty())
            || reference
                .last_update
                .as_deref()
                .is_some_and(|date| !is_calendar_date(date))
            || reference
                .publication_date
                .as_deref()
                .is_some_and(|date| !is_calendar_date(date))
            || reference
                .study_type
                .as_deref()
                .is_some_and(|value| value.trim().is_empty())
            || reference
                .enrollment_count
                .is_some_and(|count| count > 10_000_000)
            || (!reference.record_kind.is_empty()
                && reference.record_kind != "clinical_trial"
                && (!reference.phases.is_empty()
                    || reference.last_update.is_some()
                    || reference.study_type.is_some()
                    || reference.enrollment_count.is_some()
                    || !reference.intervention_names.is_empty()))
            || (!reference.record_kind.is_empty()
                && reference.record_kind != "portal_study"
                && reference.sample_count.is_some())
            || (!reference.record_kind.is_empty()
                && reference.record_kind != "literature_article"
                && reference.publication_date.is_some())
            || !seen.insert((
                reference.source,
                reference.source_id.as_str(),
                reference.record_id.as_str(),
            ))
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "evidence program references are invalid for {}",
                    track.track_id
                ),
            });
        }
    }
    Ok(())
}

fn project_track(
    spec: TrackSpec,
    specialty: Specialty,
    request: &CaseRequest,
    case_assets: Option<&CaseAssetManifestReport>,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    query: &EvidenceProgramQuery,
) -> Result<EvidenceProgramTrack, NeurosurgeryError> {
    let observation_audit = crate::observation_audit_items(request, spec.observations);
    let observation_coverage = observation_audit
        .iter()
        .map(|item| EvidenceProgramObservationCoverage {
            observation_kind: item.observation_kind,
            state: item.state,
            observed_count: item.observed_count,
            provenance_complete_count: item.provenance_complete_count,
            provenance_gap_count: item
                .observed_count
                .saturating_sub(item.provenance_complete_count),
        })
        .collect::<Vec<_>>();
    let missing_observation_kinds = observation_audit
        .iter()
        .filter(|item| item.state != crate::EvidenceState::Measured)
        .map(|item| item.observation_kind)
        .collect::<Vec<_>>();
    let observation_coverage_complete = missing_observation_kinds.is_empty();
    let observation_provenance_complete = observation_audit
        .iter()
        .all(|item| item.provenance_complete_count == item.observed_count);
    let (asset_coverage, missing_asset_kinds, asset_coverage_complete) =
        project_asset_coverage(spec.observations, case_assets);
    let review_worklist = build_review_worklist(&observation_coverage, asset_coverage.as_deref());
    let mut real_match_count = 0usize;
    let mut real_returned_count = 0usize;
    let mut real_truncated = false;
    let mut public_match_count = 0usize;
    let mut public_returned_count = 0usize;
    let mut public_truncated = false;
    let mut references = Vec::new();
    let mut seen = BTreeSet::new();
    let max_refs = query.max_references_per_track;
    if let Some(data) = real_data {
        for term in spec.terms {
            let result = data.query(&RealDataQuery {
                text: Some((*term).to_string()),
                limit: max_refs.min(crate::real_data::MAX_QUERY_HITS_PUBLIC),
                ..Default::default()
            })?;
            real_match_count = real_match_count.saturating_add(result.total_matches);
            real_returned_count = real_returned_count.saturating_add(result.returned_matches);
            real_truncated |= result.truncated;
            for hit in result.hits {
                if seen.insert((
                    EvidenceProgramSource::RealGliomaPopulation,
                    hit.record_id.clone(),
                )) && references.len() < max_refs
                {
                    references.push(real_reference(&hit, query.include_abstracts));
                }
            }
        }
    }
    if let Some(literature) = public_literature {
        for term in spec.terms {
            let result = literature.query(&PublicLiteratureQuery {
                specialty: Some(specialty),
                text: Some((*term).to_string()),
                limit: max_refs.min(crate::public_literature::MAX_QUERY_HITS_PUBLIC),
                ..Default::default()
            })?;
            public_match_count = public_match_count.saturating_add(result.total_matches);
            public_returned_count = public_returned_count.saturating_add(result.returned_matches);
            public_truncated |= result.truncated;
            for hit in result.hits {
                let id = format!("PMID-{}", hit.pmid);
                if seen.insert((EvidenceProgramSource::PublicLiterature, id.clone()))
                    && references.len() < max_refs
                {
                    references.push(EvidenceProgramReference {
                        source: EvidenceProgramSource::PublicLiterature,
                        source_id: hit.source_id,
                        record_kind: "literature_article".to_string(),
                        record_id: id,
                        title: hit.title,
                        uri: hit.record_uri,
                        abstract_excerpt: query
                            .include_abstracts
                            .then_some(hit.abstract_excerpt)
                            .flatten(),
                        status: None,
                        phases: Vec::new(),
                        last_update: None,
                        study_type: None,
                        enrollment_count: None,
                        intervention_names: Vec::new(),
                        sample_count: None,
                        publication_date: hit.publication_date,
                    });
                }
            }
        }
    }
    let total_unique_matches = seen.len();
    Ok(EvidenceProgramTrack {
        track_id: spec.id.to_string(),
        label: spec.label.to_string(),
        review_objective: spec.objective.to_string(),
        search_terms: spec.terms.iter().map(|term| (*term).to_string()).collect(),
        required_observation_kinds: spec.observations.to_vec(),
        observation_coverage,
        missing_observation_kinds,
        observation_coverage_complete,
        observation_provenance_complete,
        asset_coverage,
        missing_asset_kinds,
        asset_coverage_complete,
        review_worklist,
        reviewer_roles: specialty.profile().human_review_roles,
        real_match_count,
        real_returned_count,
        real_truncated,
        public_match_count,
        public_returned_count,
        public_truncated,
        references,
        reference_omitted_count: total_unique_matches.saturating_sub(max_refs),
        human_review_required: true,
    })
}

/// Project protocol observation classes onto the caller's digest-only asset inventory.
///
/// This is intentionally a metadata join: the evidence program never opens asset bytes and
/// never treats an observed export as a clinical finding. A single asset class can support more
/// than one observation class (for example imaging supports both imaging and spinal dysraphism),
/// so rows remain keyed by observation kind while the missing-kind list is de-duplicated.
fn project_asset_coverage(
    observations: &[crate::ObservationKind],
    case_assets: Option<&CaseAssetManifestReport>,
) -> (
    Option<Vec<EvidenceProgramAssetCoverage>>,
    Vec<CaseAssetKind>,
    Option<bool>,
) {
    let Some(report) = case_assets else {
        return (None, Vec::new(), None);
    };

    let mut coverage_by_kind = std::collections::BTreeMap::new();
    for coverage in &report.coverage {
        coverage_by_kind.insert(coverage.kind, coverage);
    }

    let mut rows = Vec::with_capacity(observations.len());
    let mut missing = BTreeSet::new();
    for observation_kind in observations.iter().copied() {
        let asset_kind = asset_kind_for_observation(observation_kind);
        let coverage = coverage_by_kind.get(&asset_kind);
        let (total_count, observed_count, provenance_complete_count) = coverage
            .map(|item| {
                (
                    item.total_count,
                    item.observed_count,
                    item.provenance_complete_count,
                )
            })
            .unwrap_or((0, 0, 0));
        let state = if observed_count > 0 {
            EvidenceProgramAssetCoverageState::Observed
        } else if total_count > 0 {
            EvidenceProgramAssetCoverageState::PresentNotObserved
        } else {
            missing.insert(asset_kind);
            EvidenceProgramAssetCoverageState::Missing
        };
        rows.push(EvidenceProgramAssetCoverage {
            observation_kind,
            asset_kind,
            state,
            total_count,
            observed_count,
            provenance_complete_count,
        });
    }
    let missing_asset_kinds = missing.into_iter().collect::<Vec<_>>();
    let complete = missing_asset_kinds.is_empty();
    (Some(rows), missing_asset_kinds, Some(complete))
}

fn asset_kind_for_observation(observation_kind: crate::ObservationKind) -> CaseAssetKind {
    match observation_kind {
        crate::ObservationKind::Imaging
        | crate::ObservationKind::SpinalDysraphism
        | crate::ObservationKind::CraniocervicalJunction => CaseAssetKind::ImagingSeries,
        crate::ObservationKind::Histology => CaseAssetKind::PathologyReport,
        crate::ObservationKind::Molecular => CaseAssetKind::MolecularAssay,
        crate::ObservationKind::Neuroanatomy => CaseAssetKind::AnatomicalModel,
        crate::ObservationKind::NeurologicFunction => CaseAssetKind::NeurofunctionalAssessment,
        crate::ObservationKind::DevelopmentalTrajectory => CaseAssetKind::DevelopmentalAssessment,
        crate::ObservationKind::SurgicalHistory => CaseAssetKind::OperativeNote,
        crate::ObservationKind::LongitudinalOutcome => CaseAssetKind::LongitudinalOutcome,
    }
}

fn build_review_worklist(
    observation_coverage: &[EvidenceProgramObservationCoverage],
    asset_coverage: Option<&[EvidenceProgramAssetCoverage]>,
) -> Vec<EvidenceProgramWorkItem> {
    let mut items = Vec::new();
    for coverage in observation_coverage {
        if coverage.state != crate::EvidenceState::Measured {
            items.push(EvidenceProgramWorkItem {
                code: "observation_coverage_gap".to_string(),
                observation_kind: Some(coverage.observation_kind),
                asset_kind: None,
                detail: "review or obtain caller-supplied metadata for this required observation class; no value is inferred".to_string(),
            });
        }
        if coverage.provenance_gap_count > 0 {
            items.push(EvidenceProgramWorkItem {
                code: "observation_provenance_gap".to_string(),
                observation_kind: Some(coverage.observation_kind),
                asset_kind: None,
                detail: "review observed caller metadata with a source identifier before human use"
                    .to_string(),
            });
        }
    }
    if let Some(asset_coverage) = asset_coverage {
        for coverage in asset_coverage {
            match coverage.state {
                EvidenceProgramAssetCoverageState::Missing => items.push(EvidenceProgramWorkItem {
                    code: "asset_class_missing".to_string(),
                    observation_kind: Some(coverage.observation_kind),
                    asset_kind: Some(coverage.asset_kind),
                    detail: "register a real de-identified export for this protocol class; asset bytes remain outside the agent".to_string(),
                }),
                EvidenceProgramAssetCoverageState::PresentNotObserved => items.push(EvidenceProgramWorkItem {
                    code: "asset_class_not_observed".to_string(),
                    observation_kind: Some(coverage.observation_kind),
                    asset_kind: Some(coverage.asset_kind),
                    detail: "review the supplied asset status with a human reviewer before treating the class as observed".to_string(),
                }),
                EvidenceProgramAssetCoverageState::Observed => {}
            }
            if coverage.observed_count > coverage.provenance_complete_count {
                items.push(EvidenceProgramWorkItem {
                    code: "asset_provenance_gap".to_string(),
                    observation_kind: Some(coverage.observation_kind),
                    asset_kind: Some(coverage.asset_kind),
                    detail: "bind each observed export to a source identifier and content digest before review".to_string(),
                });
            }
        }
    }
    items
}

fn real_reference(hit: &RealDataQueryHit, include_abstracts: bool) -> EvidenceProgramReference {
    EvidenceProgramReference {
        source: EvidenceProgramSource::RealGliomaPopulation,
        source_id: hit.source_id.clone(),
        record_kind: hit.record_kind.slug().to_string(),
        record_id: hit.record_id.clone(),
        title: hit.title.clone(),
        uri: hit.source_uri.clone(),
        abstract_excerpt: include_abstracts
            .then_some(hit.abstract_excerpt.clone())
            .flatten(),
        status: hit.status.clone(),
        phases: hit.phases.clone(),
        last_update: hit.last_update.clone(),
        study_type: hit.study_type.clone(),
        enrollment_count: hit.enrollment_count,
        intervention_names: hit.intervention_names.clone(),
        sample_count: hit.sample_count,
        publication_date: hit.publication_date.clone(),
    }
}

fn selected_specialties(
    request_specialty: Specialty,
    query: &EvidenceProgramQuery,
) -> Result<Vec<Specialty>, NeurosurgeryError> {
    let specialties = query
        .specialties
        .clone()
        .unwrap_or_else(|| vec![request_specialty]);
    if specialties.is_empty() || specialties.len() > MAX_EVIDENCE_PROGRAM_SPECIALTIES {
        return Err(NeurosurgeryError::RealDataRejected { reason: format!("evidence program specialties must contain 1..={MAX_EVIDENCE_PROGRAM_SPECIALTIES} lanes") });
    }
    let mut seen = BTreeSet::new();
    if specialties.iter().any(|specialty| !seen.insert(*specialty)) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "evidence program specialties must be unique".to_string(),
        });
    }
    let mut specialties = specialties;
    specialties.sort_unstable();
    Ok(specialties)
}

fn validate_query(query: &EvidenceProgramQuery) -> Result<(), NeurosurgeryError> {
    if !(1..=MAX_EVIDENCE_PROGRAM_TRACKS_PER_LANE).contains(&query.max_tracks_per_lane) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_program.max_tracks_per_lane",
            found: query.max_tracks_per_lane,
            max: MAX_EVIDENCE_PROGRAM_TRACKS_PER_LANE,
        });
    }
    if !(1..=MAX_EVIDENCE_PROGRAM_REFERENCES_PER_TRACK).contains(&query.max_references_per_track) {
        return Err(NeurosurgeryError::TooMany {
            field: "evidence_program.max_references_per_track",
            found: query.max_references_per_track,
            max: MAX_EVIDENCE_PROGRAM_REFERENCES_PER_TRACK,
        });
    }
    Ok(())
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_report(report: &EvidenceProgramReport) -> Result<String, NeurosurgeryError> {
    let mut copy = report.clone();
    copy.program_digest.clear();
    digest_json(&copy)
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_calendar_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || ![0usize, 1, 2, 3, 5, 6, 8, 9]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit())
        || bytes[4] != b'-'
        || bytes[7] != b'-'
    {
        return false;
    }
    let year = u16::from(bytes[0] - b'0') * 1_000
        + u16::from(bytes[1] - b'0') * 100
        + u16::from(bytes[2] - b'0') * 10
        + u16::from(bytes[3] - b'0');
    let month = (bytes[5] - b'0') * 10 + (bytes[6] - b'0');
    let day = (bytes[8] - b'0') * 10 + (bytes[9] - b'0');
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => 0,
    };
    day >= 1 && day <= days_in_month
}
