//! Deterministic specialty evidence coverage for the provider-free neurosurgical agent.
//!
//! The ordinary route already reports missing inputs per tool. This module adds the higher-level
//! specialist view a reviewer needs: each lane is decomposed into identity, spatial, functional,
//! and temporal evidence dimensions, with explicit status, provenance, and time coverage. It is
//! an inventory of caller-supplied observations, never a diagnosis, risk score, or operative plan.

use crate::{
    CaseRequest, GliomaEvidenceState, NeurosurgeryError, Observation, ObservationKind,
    ObservationStatus, RequestUse, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const SPECIALTY_EVIDENCE_MAP_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-specialty-evidence-map/0.1";

/// Aggregate state for one specialist evidence dimension. These states describe input coverage,
/// not clinical severity, probability, or readiness for an intervention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialtyEvidenceMapState {
    Complete,
    Partial,
    NotCollected,
    Uninterpretable,
    Conflicting,
}

/// One domain-specific evidence dimension and its source/time coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialtyEvidenceDimension {
    pub key: String,
    pub label: String,
    pub required_observation_kinds: Vec<ObservationKind>,
    pub required_kind_count: usize,
    pub covered_kind_count: usize,
    pub observed_observation_count: usize,
    pub not_collected_observation_count: usize,
    pub uninterpretable_observation_count: usize,
    pub conflicting_observation_count: usize,
    pub missing_provenance_count: usize,
    pub timestamped_observation_count: usize,
    pub timepoint_count: usize,
    pub source_ids: Vec<String>,
    pub state: SpecialtyEvidenceMapState,
    pub reviewer_question: String,
}

/// Digest-bound specialist evidence map. It contains labels and metadata only; observation values
/// remain in the caller-owned request and are never copied into this report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialtyEvidenceMapReport {
    pub schema_version: String,
    pub map_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub dimensions: Vec<SpecialtyEvidenceDimension>,
    pub required_dimension_count: usize,
    pub complete_dimension_count: usize,
    pub partial_dimension_count: usize,
    pub not_collected_dimension_count: usize,
    pub uninterpretable_dimension_count: usize,
    pub conflicting_dimension_count: usize,
    pub observed_observation_count: usize,
    pub evidence_record_count: usize,
    pub verified_evidence_record_count: usize,
    pub missing_provenance_count: usize,
    pub timestamped_observation_count: usize,
    pub reviewer_questions: Vec<String>,
    pub state: SpecialtyEvidenceMapState,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone)]
struct DimensionSpec {
    key: &'static str,
    label: &'static str,
    kinds: Vec<ObservationKind>,
    reviewer_question: &'static str,
}

#[derive(Debug, Default)]
struct KindCoverage {
    covered: bool,
    observed_count: usize,
    not_collected_count: usize,
    uninterpretable_count: usize,
    conflicting_count: usize,
    missing_provenance_count: usize,
    timestamped_count: usize,
    timepoints: BTreeSet<String>,
    source_ids: BTreeSet<String>,
}

#[derive(Serialize)]
struct SpecialtyEvidenceMapDigestInput<'a> {
    schema_version: &'a str,
    request_digest: &'a str,
    specialty: Specialty,
    dimensions: &'a [SpecialtyEvidenceDimension],
    required_dimension_count: usize,
    complete_dimension_count: usize,
    partial_dimension_count: usize,
    not_collected_dimension_count: usize,
    uninterpretable_dimension_count: usize,
    conflicting_dimension_count: usize,
    observed_observation_count: usize,
    evidence_record_count: usize,
    verified_evidence_record_count: usize,
    missing_provenance_count: usize,
    timestamped_observation_count: usize,
    reviewer_questions: &'a [String],
    state: SpecialtyEvidenceMapState,
}

impl KindCoverage {
    fn absorb_observation(&mut self, observation: &Observation) {
        self.covered = true;
        match observation.status {
            ObservationStatus::Observed => self.observed_count += 1,
            ObservationStatus::NotCollected => self.not_collected_count += 1,
            ObservationStatus::Uninterpretable => self.uninterpretable_count += 1,
            ObservationStatus::Conflicting => self.conflicting_count += 1,
        }
        if observation.source_id.is_none() {
            self.missing_provenance_count += 1;
        } else if let Some(source_id) = observation.source_id.as_ref() {
            self.source_ids.insert(source_id.clone());
        }
        if observation.observed_at.is_some() {
            self.timestamped_count += 1;
        }
        if let Some(timepoint) = observation.timepoint.as_ref() {
            self.timepoints.insert(timepoint.clone());
        }
    }

    fn absorb_panel_observation(
        &mut self,
        state: GliomaEvidenceState,
        source_id: Option<&str>,
        observed_at: Option<&str>,
    ) {
        self.covered = true;
        match state {
            GliomaEvidenceState::Present | GliomaEvidenceState::Absent => self.observed_count += 1,
            GliomaEvidenceState::NotCollected => self.not_collected_count += 1,
            GliomaEvidenceState::Uninterpretable => self.uninterpretable_count += 1,
            GliomaEvidenceState::Conflicting => self.conflicting_count += 1,
        }
        if let Some(source_id) = source_id {
            self.source_ids.insert(source_id.to_string());
        } else if matches!(
            state,
            GliomaEvidenceState::Present | GliomaEvidenceState::Absent
        ) {
            self.missing_provenance_count += 1;
        }
        if observed_at.is_some() {
            self.timestamped_count += 1;
        }
    }
}

impl SpecialtyEvidenceMapReport {
    /// Re-derive the map from the exact request before accepting it into a mission envelope.
    /// Structural validation alone is insufficient because the digest is intentionally
    /// reproducible rather than secret-signed.
    pub fn validate_for_request(&self, request: &CaseRequest) -> Result<(), NeurosurgeryError> {
        let expected = build_specialty_evidence_map(request)?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "specialty evidence map does not match the supplied request".to_string(),
            });
        }
        Ok(())
    }

    /// Validate the stable shape and digest format before a caller persists or joins the map.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        let expected_map_digest = digest_map(self)?;
        let expected_dimension_keys = dimension_specs(self.specialty)
            .into_iter()
            .map(|spec| spec.key)
            .collect::<Vec<_>>();
        if self.schema_version != SPECIALTY_EVIDENCE_MAP_SCHEMA_VERSION
            || self.request_digest.len() != 64
            || self.map_digest.len() != 64
            || !self
                .request_digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
            || !self.map_digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            || self.required_dimension_count != self.dimensions.len()
            || self.complete_dimension_count
                + self.partial_dimension_count
                + self.not_collected_dimension_count
                + self.uninterpretable_dimension_count
                + self.conflicting_dimension_count
                != self.dimensions.len()
            || !self.provenance_bound
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || !self.human_review_required
            || self.map_digest != expected_map_digest
            || self.dimensions.len() != expected_dimension_keys.len()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "specialty evidence map integrity fields are invalid".to_string(),
            });
        }
        let mut keys = BTreeSet::new();
        for (dimension, expected_key) in self.dimensions.iter().zip(expected_dimension_keys) {
            if dimension.key.trim().is_empty()
                || !keys.insert(dimension.key.as_str())
                || dimension.key != expected_key
                || dimension.required_kind_count != dimension.required_observation_kinds.len()
                || dimension.covered_kind_count > dimension.required_kind_count
                || dimension
                    .source_ids
                    .windows(2)
                    .any(|window| window[0] >= window[1])
            {
                return Err(NeurosurgeryError::RealDataRejected {
                    reason: "specialty evidence map dimensions are not canonical".to_string(),
                });
            }
        }
        Ok(())
    }
}

/// Build the map from a validated caller request. This function is public so local adapters can
/// compose the same report without constructing a full agent run.
pub fn build_specialty_evidence_map(
    request: &CaseRequest,
) -> Result<SpecialtyEvidenceMapReport, NeurosurgeryError> {
    let request_digest = digest(request)?;
    let dimensions = dimension_specs(request.specialty)
        .into_iter()
        .map(|spec| build_dimension(request, spec))
        .collect::<Vec<_>>();
    let mut reviewer_questions = dimensions
        .iter()
        .filter(|dimension| dimension.state != SpecialtyEvidenceMapState::Complete)
        .map(|dimension| dimension.reviewer_question.clone())
        .collect::<Vec<_>>();
    if dimensions
        .iter()
        .any(|dimension| dimension.missing_provenance_count > 0)
    {
        reviewer_questions.push(
            "Which caller-owned source identifier independently supports each observed input?"
                .to_string(),
        );
    }
    reviewer_questions.sort();
    reviewer_questions.dedup();

    let mut complete_dimension_count = 0;
    let mut partial_dimension_count = 0;
    let mut not_collected_dimension_count = 0;
    let mut uninterpretable_dimension_count = 0;
    let mut conflicting_dimension_count = 0;
    for dimension in &dimensions {
        match dimension.state {
            SpecialtyEvidenceMapState::Complete => complete_dimension_count += 1,
            SpecialtyEvidenceMapState::Partial => partial_dimension_count += 1,
            SpecialtyEvidenceMapState::NotCollected => not_collected_dimension_count += 1,
            SpecialtyEvidenceMapState::Uninterpretable => uninterpretable_dimension_count += 1,
            SpecialtyEvidenceMapState::Conflicting => conflicting_dimension_count += 1,
        }
    }
    let observed_observation_count = request
        .observations
        .iter()
        .filter(|observation| observation.status == ObservationStatus::Observed)
        .count()
        + request
            .glioma_molecular
            .as_ref()
            .map(|panel| {
                panel
                    .observations
                    .iter()
                    .filter(|observation| {
                        matches!(
                            observation.state,
                            GliomaEvidenceState::Present | GliomaEvidenceState::Absent
                        )
                    })
                    .count()
            })
            .unwrap_or(0);
    let timestamped_observation_count = request
        .observations
        .iter()
        .filter(|observation| observation.observed_at.is_some())
        .count()
        + request
            .glioma_molecular
            .as_ref()
            .map(|panel| {
                panel
                    .observations
                    .iter()
                    .filter(|observation| observation.observed_at.is_some())
                    .count()
            })
            .unwrap_or(0);
    let missing_provenance_count = request
        .observations
        .iter()
        .filter(|observation| observation.source_id.is_none())
        .count()
        + request
            .glioma_molecular
            .as_ref()
            .map(|panel| {
                panel
                    .observations
                    .iter()
                    .filter(|observation| {
                        matches!(
                            observation.state,
                            GliomaEvidenceState::Present | GliomaEvidenceState::Absent
                        ) && observation.source_id.is_none()
                    })
                    .count()
            })
            .unwrap_or(0);
    let verified_evidence_record_count = request
        .evidence
        .iter()
        .filter(|record| record.tier.is_verified())
        .count();
    let state = if conflicting_dimension_count > 0 || uninterpretable_dimension_count > 0 {
        SpecialtyEvidenceMapState::Conflicting
    } else if complete_dimension_count == dimensions.len() {
        SpecialtyEvidenceMapState::Complete
    } else if complete_dimension_count > 0 || partial_dimension_count > 0 {
        SpecialtyEvidenceMapState::Partial
    } else {
        SpecialtyEvidenceMapState::NotCollected
    };
    let mut report = SpecialtyEvidenceMapReport {
        schema_version: SPECIALTY_EVIDENCE_MAP_SCHEMA_VERSION.to_string(),
        map_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        required_dimension_count: dimensions.len(),
        complete_dimension_count,
        partial_dimension_count,
        not_collected_dimension_count,
        uninterpretable_dimension_count,
        conflicting_dimension_count,
        observed_observation_count,
        evidence_record_count: request.evidence.len(),
        verified_evidence_record_count,
        missing_provenance_count,
        timestamped_observation_count,
        reviewer_questions,
        state,
        dimensions,
        provenance_bound: true,
        synthetic_data: request.request_use == RequestUse::SyntheticCaseSimulation,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "the map inventories caller-supplied observations and evidence metadata; it does not read asset bytes or interpret imaging, pathology, genomics, or operative text".to_string(),
            "dimension states describe coverage and review posture only; they are not a diagnosis, prognosis, urgency signal, treatment recommendation, or procedural plan".to_string(),
            "missing, uninterpretable, and conflicting inputs remain explicit and are never converted into negative findings".to_string(),
            "the report is deterministic and provider-free; no model, network, credential, patient file, or durable state is accessed".to_string(),
        ],
    };
    report.map_digest = digest_map(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn dimension_specs(specialty: Specialty) -> Vec<DimensionSpec> {
    let spec = |key, label, kinds, reviewer_question| DimensionSpec {
        key,
        label,
        kinds,
        reviewer_question,
    };
    match specialty {
        Specialty::Glioma => vec![
            spec(
                "tumor_identity",
                "Tumor identity and assay scope",
                vec![ObservationKind::Histology, ObservationKind::Molecular],
                "Which histology and molecular dimensions are directly measured, and which remain unrun or provenance-incomplete?",
            ),
            spec(
                "lesion_spatial_context",
                "Lesion spatial and corridor context",
                vec![ObservationKind::Imaging, ObservationKind::Neuroanatomy],
                "Which lesion compartments, adjacent structures, and imaging limitations are independently observed?",
            ),
            spec(
                "function_and_intervention_context",
                "Neurologic function and intervention context",
                vec![ObservationKind::NeurologicFunction, ObservationKind::SurgicalHistory],
                "Are neurologic function and prior intervention observations sourced and time-aligned?",
            ),
            spec(
                "longitudinal_context",
                "Longitudinal acquisition context",
                vec![ObservationKind::LongitudinalOutcome],
                "Which baseline, follow-up, and outcome observations carry caller-supplied timestamps?",
            ),
        ],
        Specialty::CranialBase => vec![
            spec(
                "lesion_identity",
                "Lesion identity and pathology scope",
                vec![ObservationKind::Histology],
                "Is lesion identity directly observed, or is the supplied evidence limited to location?",
            ),
            spec(
                "skull_base_geometry",
                "Skull-base geometry and adjacent structures",
                vec![ObservationKind::Imaging, ObservationKind::Neuroanatomy],
                "Which bone, dura, vascular, orbital, sinonasal, and cranial-nerve relationships are actually resolved?",
            ),
            spec(
                "functional_context",
                "Neurologic and cranial-nerve function",
                vec![ObservationKind::NeurologicFunction],
                "Which neurologic or cranial-nerve functional assessments are documented and sourced?",
            ),
            spec(
                "intervention_trajectory",
                "Prior intervention and serial context",
                vec![ObservationKind::SurgicalHistory, ObservationKind::LongitudinalOutcome],
                "Are prior interventions and serial observations aligned to explicit timepoints?",
            ),
        ],
        Specialty::Craniosynostosis => vec![
            spec(
                "developmental_identity",
                "Suture and developmental identity",
                vec![ObservationKind::DevelopmentalTrajectory, ObservationKind::Molecular],
                "Which suture, developmental, and syndromic-test observations are directly measured?",
            ),
            spec(
                "craniofacial_geometry",
                "Craniofacial and intracranial geometry",
                vec![ObservationKind::Imaging, ObservationKind::Neuroanatomy],
                "Are imaging planes, reference standards, units, and venous or orbital relationships explicit?",
            ),
            spec(
                "functional_context",
                "Airway, vision, hearing, and neurologic function",
                vec![ObservationKind::NeurologicFunction],
                "Which functional domains are measured, and which are simply not collected?",
            ),
            spec(
                "growth_and_intervention_trajectory",
                "Growth and intervention trajectory",
                vec![ObservationKind::LongitudinalOutcome, ObservationKind::SurgicalHistory],
                "Are growth windows and prior interventions anchored to caller-supplied dates?",
            ),
        ],
        Specialty::Encephalocele => vec![
            spec(
                "defect_anatomy",
                "Defect anatomy and tissue relationships",
                vec![ObservationKind::Imaging, ObservationKind::Neuroanatomy],
                "Is defect location and tissue or CSF relationship directly observed in a stated modality and plane?",
            ),
            spec(
                "developmental_context",
                "Congenital and developmental context",
                vec![ObservationKind::DevelopmentalTrajectory],
                "Which prenatal, neonatal, and developmental observations are independently sourced?",
            ),
            spec(
                "neurologic_function",
                "Neurologic function",
                vec![ObservationKind::NeurologicFunction],
                "Which neurologic functions are assessed rather than inferred from anatomy?",
            ),
            spec(
                "repair_and_trajectory",
                "Repair and longitudinal trajectory",
                vec![ObservationKind::SurgicalHistory, ObservationKind::LongitudinalOutcome],
                "Are repair history and later observations linked to explicit timepoints and source identifiers?",
            ),
        ],
        Specialty::SpinaBifida => vec![
            spec(
                "dysraphism_anatomy",
                "Spinal dysraphism anatomy",
                vec![ObservationKind::SpinalDysraphism, ObservationKind::Imaging],
                "Are level, laterality, planes, units, cord, conus, and root relationships explicitly observed?",
            ),
            spec(
                "neural_function",
                "Motor, sensory, bladder, bowel, and neurologic function",
                vec![ObservationKind::NeurologicFunction],
                "Which functional domains are measured at the same time as the anatomic observations?",
            ),
            spec(
                "associated_anatomy",
                "Associated craniospinal anatomy",
                vec![ObservationKind::Neuroanatomy],
                "Which associated craniospinal or orthopedic relationships are actually supplied?",
            ),
            spec(
                "postoperative_trajectory",
                "Postoperative and developmental trajectory",
                vec![ObservationKind::SurgicalHistory, ObservationKind::LongitudinalOutcome],
                "Are postoperative, developmental, and functional changes aligned to explicit dates?",
            ),
        ],
        Specialty::ChiariMalformation => vec![
            spec(
                "junction_geometry",
                "Craniocervical-junction geometry",
                vec![ObservationKind::CraniocervicalJunction, ObservationKind::Imaging],
                "Which measurements, reference landmarks, planes, and flow or positional conditions are documented?",
            ),
            spec(
                "neuroanatomy_context",
                "Brainstem, tonsil, cord, and CSF-space relationships",
                vec![ObservationKind::Neuroanatomy],
                "Are associated anatomic findings independently observed rather than merely queried?",
            ),
            spec(
                "symptom_function_context",
                "Symptom and neurologic-function context",
                vec![ObservationKind::NeurologicFunction],
                "Which neurologic, sleep, swallowing, or functional assessments are time-aligned and sourced?",
            ),
            spec(
                "intervention_trajectory",
                "Prior intervention and longitudinal trajectory",
                vec![ObservationKind::SurgicalHistory, ObservationKind::LongitudinalOutcome],
                "Are prior interventions and serial symptom or function observations explicitly dated?",
            ),
        ],
    }
}

fn build_dimension(request: &CaseRequest, spec: DimensionSpec) -> SpecialtyEvidenceDimension {
    let coverages = spec
        .kinds
        .iter()
        .map(|kind| kind_coverage(request, *kind))
        .collect::<Vec<_>>();
    let covered_kind_count = coverages.iter().filter(|coverage| coverage.covered).count();
    let observed_observation_count = coverages
        .iter()
        .map(|coverage| coverage.observed_count)
        .sum();
    let not_collected_observation_count = coverages
        .iter()
        .map(|coverage| coverage.not_collected_count)
        .sum();
    let uninterpretable_observation_count = coverages
        .iter()
        .map(|coverage| coverage.uninterpretable_count)
        .sum();
    let conflicting_observation_count = coverages
        .iter()
        .map(|coverage| coverage.conflicting_count)
        .sum();
    let missing_provenance_count = coverages
        .iter()
        .map(|coverage| coverage.missing_provenance_count)
        .sum();
    let timestamped_observation_count = coverages
        .iter()
        .map(|coverage| coverage.timestamped_count)
        .sum();
    let timepoints = coverages
        .iter()
        .flat_map(|coverage| coverage.timepoints.iter().cloned())
        .collect::<BTreeSet<_>>();
    let source_ids = coverages
        .iter()
        .flat_map(|coverage| coverage.source_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let state = if conflicting_observation_count > 0 {
        SpecialtyEvidenceMapState::Conflicting
    } else if covered_kind_count == spec.kinds.len()
        && observed_observation_count > 0
        && uninterpretable_observation_count == 0
    {
        SpecialtyEvidenceMapState::Complete
    } else if observed_observation_count > 0 {
        SpecialtyEvidenceMapState::Partial
    } else if uninterpretable_observation_count > 0 {
        SpecialtyEvidenceMapState::Uninterpretable
    } else {
        SpecialtyEvidenceMapState::NotCollected
    };
    SpecialtyEvidenceDimension {
        key: spec.key.to_string(),
        label: spec.label.to_string(),
        required_kind_count: spec.kinds.len(),
        required_observation_kinds: spec.kinds,
        covered_kind_count,
        observed_observation_count,
        not_collected_observation_count,
        uninterpretable_observation_count,
        conflicting_observation_count,
        missing_provenance_count,
        timestamped_observation_count,
        timepoint_count: timepoints.len(),
        source_ids: source_ids.into_iter().collect(),
        state,
        reviewer_question: spec.reviewer_question.to_string(),
    }
}

fn kind_coverage(request: &CaseRequest, kind: ObservationKind) -> KindCoverage {
    let mut coverage = request
        .observations
        .iter()
        .filter(|observation| observation.kind == kind)
        .fold(KindCoverage::default(), |mut coverage, observation| {
            coverage.absorb_observation(observation);
            coverage
        });
    if kind == ObservationKind::Molecular
        && !request
            .observations
            .iter()
            .any(|observation| observation.kind == ObservationKind::Molecular)
    {
        if let Some(panel) = &request.glioma_molecular {
            for observation in &panel.observations {
                coverage.absorb_panel_observation(
                    observation.state,
                    observation.source_id.as_deref(),
                    observation.observed_at.as_deref(),
                );
            }
        }
    }
    coverage
}

fn digest<T: Serialize>(value: &T) -> Result<String, NeurosurgeryError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_map(report: &SpecialtyEvidenceMapReport) -> Result<String, NeurosurgeryError> {
    digest(&SpecialtyEvidenceMapDigestInput {
        schema_version: report.schema_version.as_str(),
        request_digest: report.request_digest.as_str(),
        specialty: report.specialty,
        dimensions: &report.dimensions,
        required_dimension_count: report.required_dimension_count,
        complete_dimension_count: report.complete_dimension_count,
        partial_dimension_count: report.partial_dimension_count,
        not_collected_dimension_count: report.not_collected_dimension_count,
        uninterpretable_dimension_count: report.uninterpretable_dimension_count,
        conflicting_dimension_count: report.conflicting_dimension_count,
        observed_observation_count: report.observed_observation_count,
        evidence_record_count: report.evidence_record_count,
        verified_evidence_record_count: report.verified_evidence_record_count,
        missing_provenance_count: report.missing_provenance_count,
        timestamped_observation_count: report.timestamped_observation_count,
        reviewer_questions: &report.reviewer_questions,
        state: report.state,
    })
}
