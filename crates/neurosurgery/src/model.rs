//! Versioned wire and report types for the neurosurgical agent.

use crate::NEUROSURGERY_SCHEMA_VERSION;
use crate::{GliomaMolecularPanel, PublicLiteratureSummary, RealDataQueryResult};
use serde::{Deserialize, Serialize};

fn default_schema_version() -> String {
    NEUROSURGERY_SCHEMA_VERSION.to_string()
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

/// Specialty routing is explicit so a caller can inspect the route before any synthesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Specialty {
    Glioma,
    CranialBase,
    Craniosynostosis,
    Encephalocele,
    SpinaBifida,
    ChiariMalformation,
}

impl Specialty {
    pub const ALL: [Self; 6] = [
        Self::Glioma,
        Self::CranialBase,
        Self::Craniosynostosis,
        Self::Encephalocele,
        Self::SpinaBifida,
        Self::ChiariMalformation,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Glioma => "glioma",
            Self::CranialBase => "cranial_base",
            Self::Craniosynostosis => "craniosynostosis",
            Self::Encephalocele => "encephalocele",
            Self::SpinaBifida => "spina_bifida",
            Self::ChiariMalformation => "chiari_malformation",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::Glioma => "glioma and neuro-oncology",
            Self::CranialBase => "cranial-base surgery",
            Self::Craniosynostosis => "craniosynostosis and craniofacial reconstruction",
            Self::Encephalocele => "encephalocele and skull-base congenital anomalies",
            Self::SpinaBifida => "spina bifida and spinal dysraphism",
            Self::ChiariMalformation => "Chiari malformation and the craniocervical junction",
        }
    }

    /// Returns the machine-readable research focus areas that make each lane more specific than
    /// a disease-name match. These are protocol labels for evidence collection and review, not
    /// diagnostic categories or recommendations.
    pub const fn focus_areas(self) -> &'static [SpecialtyFocusArea] {
        match self {
            Self::Glioma => &[
                SpecialtyFocusArea::GliomaHistomolecularIdentity,
                SpecialtyFocusArea::GliomaImagingPhenotype,
                SpecialtyFocusArea::GliomaFunctionalNetwork,
                SpecialtyFocusArea::GliomaTreatmentEffect,
                SpecialtyFocusArea::GliomaCohortAndTrialProvenance,
            ],
            Self::CranialBase => &[
                SpecialtyFocusArea::CranialBaseCompartment,
                SpecialtyFocusArea::CranialNerveAndVascularContext,
                SpecialtyFocusArea::CranialBaseCsFAndReconstruction,
            ],
            Self::Craniosynostosis => &[
                SpecialtyFocusArea::CraniosynostosisSuturePattern,
                SpecialtyFocusArea::CraniosynostosisSyndromicDevelopment,
                SpecialtyFocusArea::CraniosynostosisPressureAndFunction,
            ],
            Self::Encephalocele => &[
                SpecialtyFocusArea::EncephaloceleDefectAndContents,
                SpecialtyFocusArea::EncephaloceleAssociatedAnomalies,
                SpecialtyFocusArea::EncephaloceleCsFAndRepair,
            ],
            Self::SpinaBifida => &[
                SpecialtyFocusArea::SpinaBifidaDysraphismLevel,
                SpecialtyFocusArea::SpinaBifidaCordAndTethering,
                SpecialtyFocusArea::SpinaBifidaMotorBladderAndDevelopment,
            ],
            Self::ChiariMalformation => &[
                SpecialtyFocusArea::ChiariCraniocervicalMeasurements,
                SpecialtyFocusArea::ChiariCsFAndSyrinx,
                SpecialtyFocusArea::ChiariSpinalAndFunctionalContext,
            ],
        }
    }

    /// Returns the bounded research protocol for a specialty. These are questions and
    /// confounders for evidence review, not diagnostic criteria or operative instructions.
    pub fn profile(self) -> SpecialtyProfile {
        let strings = |values: &[&str]| values.iter().map(|value| (*value).to_string()).collect();
        match self {
            Self::Glioma => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "integrated histomolecular identity and the exact assay scope",
                    "IDH, 1p/19q, H3, MGMT, TERT, EGFR and other caller-declared markers kept distinct from unrun assays",
                    "tumour, matched-normal and specimen-timepoint provenance",
                ]),
                spatial_axes: strings(&[
                    "lesion compartment, eloquent cortex, white-matter, ventricular and vascular relationships",
                    "multimodal imaging registration, sequence availability and acquisition limitations",
                ]),
                temporal_axes: strings(&[
                    "pre-treatment baseline versus post-treatment interval",
                    "acquisition dates, intervention context and delayed-entry windows",
                    "progression, treatment-effect and sampling hypotheses kept separate",
                ]),
                evidence_questions: strings(&[
                    "Which assertions are patient-level observations and which are population-level evidence?",
                    "What independent source supports each molecular or imaging assertion?",
                    "Which missing assay, timepoint or modality would discriminate competing research hypotheses?",
                    "Are cohort inclusion criteria, endpoint definitions and follow-up windows compatible?",
                    "Are trial status, registry update timestamps and public-study flags current?",
                ]),
                confounders: strings(&[
                    "intratumour heterogeneity and sampling bias",
                    "radiographic change without tissue confirmation",
                    "platform, batch and reference-build effects",
                    "censoring, delayed entry and treatment-era shifts",
                    "duplicate cohorts, publication overlap and non-independent samples",
                ]),
                human_review_roles: strings(&[
                    "neuro-oncology",
                    "neuroradiology",
                    "neuropathology",
                    "neurosurgery",
                    "biostatistics and data governance",
                ]),
            },
            Self::CranialBase => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "compartment and skull-base interface labels supplied by the caller",
                    "pathology or lesion identity kept separate from anatomic location",
                ]),
                spatial_axes: strings(&[
                    "bone, dura, orbit, sinonasal, cavernous-sinus, clival and craniovertebral relationships",
                    "vascular and cranial-nerve adjacency represented only when observed and sourced",
                ]),
                temporal_axes: strings(&[
                    "serial imaging alignment and interval change",
                    "prior intervention, reconstruction and artifact context",
                ]),
                evidence_questions: strings(&[
                    "Which compartments and adjacent structures are actually resolved by the supplied modality?",
                    "Are laterality, orientation and registration conventions explicit?",
                    "Which functional or vascular assessments remain unmeasured?",
                ]),
                confounders: strings(&[
                    "partial-volume and motion artifact",
                    "postoperative distortion and hardware artifact",
                    "ambiguous compartment boundaries",
                    "unresolved cranial-nerve or vascular status",
                ]),
                human_review_roles: strings(&[
                    "skull-base neurosurgery",
                    "neuroradiology",
                    "otolaryngology or head-and-neck surgery",
                    "neuro-ophthalmology",
                ]),
            },
            Self::Craniosynostosis => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "suture pattern and syndromic status as caller-declared hypotheses",
                    "developmental stage, growth trajectory and genetic-test scope",
                ]),
                spatial_axes: strings(&[
                    "calvarial, cranial-base, orbital and midface relationships",
                    "intracranial-volume and venous-anatomy observations with measurement context",
                ]),
                temporal_axes: strings(&[
                    "age-aligned growth and head-shape observations",
                    "serial imaging and functional assessments across developmental windows",
                ]),
                evidence_questions: strings(&[
                    "Which suture and developmental observations are directly measured?",
                    "Are age, reference standards and measurement units recorded?",
                    "Which airway, vision, hearing, neurologic or developmental assessments are absent?",
                ]),
                confounders: strings(&[
                    "age-dependent normal variation",
                    "measurement-plane and reference-standard differences",
                    "syndromic label inferred without a declared assay",
                    "developmental assessments missing or time-misaligned",
                ]),
                human_review_roles: strings(&[
                    "craniofacial surgery",
                    "pediatric neurosurgery",
                    "neuroradiology",
                    "genetics",
                    "developmental pediatrics",
                ]),
            },
            Self::Encephalocele => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "defect location and caller-described neural or meningeal content",
                    "congenital history, associated anomalies and imaging scope",
                ]),
                spatial_axes: strings(&[
                    "calvarial or skull-base defect boundaries",
                    "neural, vascular, sinus and cerebrospinal-fluid relationships when resolved",
                ]),
                temporal_axes: strings(&[
                    "prenatal, neonatal and later imaging timepoints",
                    "growth, neurologic function and prior repair context",
                ]),
                evidence_questions: strings(&[
                    "Is the defect measured in a stated plane and modality?",
                    "Which tissue-content assertions are observed versus inferred?",
                    "Are developmental, neurologic and cerebrospinal-fluid observations independently sourced?",
                ]),
                confounders: strings(&[
                    "limited-resolution tissue characterization",
                    "post-repair anatomy and scar or hardware artifact",
                    "age-dependent anatomy",
                    "incomplete associated-anomaly assessment",
                ]),
                human_review_roles: strings(&[
                    "pediatric neurosurgery",
                    "craniofacial surgery",
                    "neuroradiology",
                    "neonatology or pediatrics",
                ]),
            },
            Self::SpinaBifida => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "spinal-dysraphism phenotype and level as caller-declared observations",
                    "neural-tissue, tethering and associated anomaly evidence kept separate",
                ]),
                spatial_axes: strings(&[
                    "vertebral, canal, cord, conus and nerve-root relationships",
                    "craniospinal and urologic or orthopedic context only when supplied and sourced",
                ]),
                temporal_axes: strings(&[
                    "developmental and postoperative serial imaging",
                    "time-aligned motor, sensory, bladder, bowel and functional observations",
                ]),
                evidence_questions: strings(&[
                    "Is neurologic function measured at the same time as the anatomic observation?",
                    "Are level, laterality, units and imaging planes explicit?",
                    "Which functional domains are not collected rather than normal?",
                ]),
                confounders: strings(&[
                    "age and developmental-stage differences",
                    "examiner and instrument variation",
                    "postoperative tethering or scar effects",
                    "anatomic-functional discordance",
                ]),
                human_review_roles: strings(&[
                    "pediatric neurosurgery",
                    "neuroradiology",
                    "neurology or rehabilitation",
                    "urology",
                    "orthopedics",
                ]),
            },
            Self::ChiariMalformation => SpecialtyProfile {
                specialty: self,
                focus_areas: self.focus_areas().to_vec(),
                identity_axes: strings(&[
                    "craniocervical-junction measurements and the exact reference convention",
                    "associated findings kept distinct from a caller's symptom description",
                ]),
                spatial_axes: strings(&[
                    "foramen magnum, brainstem, tonsils, upper cervical cord and CSF-space relationships",
                    "spinal-axis and cranial-base context when actually imaged",
                ]),
                temporal_axes: strings(&[
                    "symptom, function and imaging time alignment",
                    "prior intervention, positional and acquisition context",
                ]),
                evidence_questions: strings(&[
                    "Which measurements, planes and reference landmarks are documented?",
                    "Are associated findings independently observed or merely queried?",
                    "Which neurologic, sleep, swallowing or functional assessments are absent?",
                ]),
                confounders: strings(&[
                    "measurement convention and plane differences",
                    "positional, motion and flow artifact",
                    "symptom attribution without time-aligned objective evidence",
                    "incomplete whole-spine or cranial-base coverage",
                ]),
                human_review_roles: strings(&[
                    "neurosurgery",
                    "neuroradiology",
                    "neurology",
                    "sleep or swallowing specialists when relevant",
                ]),
            },
        }
    }
}

/// A closed, machine-readable research focus for a specialty lane. Focuses identify which
/// evidence plane a reviewer may want to inspect; they never assert that a finding is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpecialtyFocusArea {
    GliomaHistomolecularIdentity,
    GliomaImagingPhenotype,
    GliomaFunctionalNetwork,
    GliomaTreatmentEffect,
    GliomaCohortAndTrialProvenance,
    CranialBaseCompartment,
    CranialNerveAndVascularContext,
    CranialBaseCsFAndReconstruction,
    CraniosynostosisSuturePattern,
    CraniosynostosisSyndromicDevelopment,
    CraniosynostosisPressureAndFunction,
    EncephaloceleDefectAndContents,
    EncephaloceleAssociatedAnomalies,
    EncephaloceleCsFAndRepair,
    SpinaBifidaDysraphismLevel,
    SpinaBifidaCordAndTethering,
    SpinaBifidaMotorBladderAndDevelopment,
    ChiariCraniocervicalMeasurements,
    ChiariCsFAndSyrinx,
    ChiariSpinalAndFunctionalContext,
}

impl SpecialtyFocusArea {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GliomaHistomolecularIdentity => "glioma histomolecular identity",
            Self::GliomaImagingPhenotype => "glioma imaging phenotype",
            Self::GliomaFunctionalNetwork => "glioma functional network and eloquent anatomy",
            Self::GliomaTreatmentEffect => "glioma treatment effect and longitudinal change",
            Self::GliomaCohortAndTrialProvenance => "glioma cohort and trial provenance",
            Self::CranialBaseCompartment => "cranial-base compartment anatomy",
            Self::CranialNerveAndVascularContext => "cranial-nerve and vascular context",
            Self::CranialBaseCsFAndReconstruction => "cranial-base CSF and reconstruction context",
            Self::CraniosynostosisSuturePattern => "craniosynostosis suture pattern",
            Self::CraniosynostosisSyndromicDevelopment => {
                "craniosynostosis syndromic and developmental context"
            }
            Self::CraniosynostosisPressureAndFunction => {
                "craniosynostosis pressure and functional assessment"
            }
            Self::EncephaloceleDefectAndContents => {
                "encephalocele defect and tissue-content context"
            }
            Self::EncephaloceleAssociatedAnomalies => "encephalocele associated anomalies",
            Self::EncephaloceleCsFAndRepair => "encephalocele CSF and repair context",
            Self::SpinaBifidaDysraphismLevel => "spina-bifida dysraphism level",
            Self::SpinaBifidaCordAndTethering => "spina-bifida cord and tethering context",
            Self::SpinaBifidaMotorBladderAndDevelopment => {
                "spina-bifida motor, bladder, and developmental context"
            }
            Self::ChiariCraniocervicalMeasurements => "Chiari craniocervical-junction measurements",
            Self::ChiariCsFAndSyrinx => "Chiari CSF-flow and syrinx context",
            Self::ChiariSpinalAndFunctionalContext => "Chiari spinal and functional context",
        }
    }
}

/// Research protocol metadata for one specialty. It intentionally describes what a reviewer
/// should interrogate, never what a clinician should diagnose or do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecialtyProfile {
    pub specialty: Specialty,
    /// Machine-readable review focuses for this lane. Older persisted profiles may omit this
    /// field; the live catalogue always fills it from `Specialty::focus_areas()`.
    #[serde(default)]
    pub focus_areas: Vec<SpecialtyFocusArea>,
    pub identity_axes: Vec<String>,
    pub spatial_axes: Vec<String>,
    pub temporal_axes: Vec<String>,
    pub evidence_questions: Vec<String>,
    pub confounders: Vec<String>,
    pub human_review_roles: Vec<String>,
}

/// Permitted research/education purposes and the clinical uses that are refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestUse {
    ResearchSynthesis,
    SyntheticCaseSimulation,
    EducationalReview,
    IndividualDiagnosis,
    IndividualPrognosis,
    TreatmentRecommendation,
    CareTriage,
    UrgentClinicalAlert,
    InterventionPlanning,
}

impl RequestUse {
    pub const fn is_clinical(self) -> bool {
        !matches!(
            self,
            Self::ResearchSynthesis | Self::SyntheticCaseSimulation | Self::EducationalReview
        )
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::ResearchSynthesis => "research synthesis",
            Self::SyntheticCaseSimulation => "synthetic case simulation",
            Self::EducationalReview => "educational review",
            Self::IndividualDiagnosis => "individual diagnosis",
            Self::IndividualPrognosis => "individual prognosis",
            Self::TreatmentRecommendation => "treatment recommendation",
            Self::CareTriage => "care triage",
            Self::UrgentClinicalAlert => "urgent clinical alert",
            Self::InterventionPlanning => "intervention planning",
        }
    }
}

/// A clinical or biological signal supplied by a caller, never inferred by this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationKind {
    Imaging,
    Histology,
    Molecular,
    Neuroanatomy,
    NeurologicFunction,
    DevelopmentalTrajectory,
    SpinalDysraphism,
    CraniocervicalJunction,
    SurgicalHistory,
    LongitudinalOutcome,
}

/// An observation may be absent or uninterpretable; neither state is a negative finding.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ObservationStatus {
    #[default]
    Observed,
    NotCollected,
    Uninterpretable,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub kind: ObservationKind,
    pub label: String,
    pub value: String,
    #[serde(default)]
    pub status: ObservationStatus,
    #[serde(default)]
    pub source_id: Option<String>,
    /// Optional caller-supplied UTC acquisition/assessment timestamp. It is metadata for
    /// longitudinal alignment, never a timestamp inferred from the observation text.
    #[serde(default)]
    pub observed_at: Option<String>,
    /// Optional caller-supplied de-identified timepoint label (for example `baseline` or
    /// `post_intervention`). A label without `observed_at` remains explicitly date-unknown.
    #[serde(default)]
    pub timepoint: Option<String>,
}

/// Evidence tier is provenance metadata, not a claim that the evidence applies to a case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    Guideline,
    SystematicReview,
    CohortStudy,
    CaseSeries,
    ExpertConsensus,
    LocalProtocol,
    Unverified,
}

impl EvidenceTier {
    pub const fn is_verified(self) -> bool {
        !matches!(self, Self::Unverified)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRecord {
    pub id: String,
    pub title: String,
    pub citation: String,
    pub tier: EvidenceTier,
    #[serde(default)]
    pub population: Option<String>,
    #[serde(default)]
    pub year: Option<u16>,
    /// Caller-declared capability coverage; this crate does not verify the claims.
    #[serde(default)]
    pub supports: Vec<ToolCapability>,
}

/// Capabilities exposed by the built-in read-only tool set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCapability {
    SafetyGate,
    CaseIntegrity,
    EvidenceGapScan,
    ImagingReview,
    NeuroanatomyMap,
    MolecularContext,
    DifferentialMatrix,
    LongitudinalTrajectory,
    CranialBaseRiskMap,
    CraniofacialDevelopment,
    SpinalDysraphismMap,
    CraniocervicalJunctionMap,
    RealDataInventory,
    RealDataQuery,
    EvidenceSynthesis,
    HumanReviewHold,
}

impl ToolCapability {
    pub const ALL: [Self; 16] = [
        Self::SafetyGate,
        Self::CaseIntegrity,
        Self::EvidenceGapScan,
        Self::ImagingReview,
        Self::NeuroanatomyMap,
        Self::MolecularContext,
        Self::DifferentialMatrix,
        Self::LongitudinalTrajectory,
        Self::CranialBaseRiskMap,
        Self::CraniofacialDevelopment,
        Self::SpinalDysraphismMap,
        Self::CraniocervicalJunctionMap,
        Self::RealDataInventory,
        Self::RealDataQuery,
        Self::EvidenceSynthesis,
        Self::HumanReviewHold,
    ];

    pub const fn slug(self) -> &'static str {
        match self {
            Self::SafetyGate => "safety_gate",
            Self::CaseIntegrity => "case_integrity",
            Self::EvidenceGapScan => "evidence_gap_scan",
            Self::ImagingReview => "imaging_review",
            Self::NeuroanatomyMap => "neuroanatomy_map",
            Self::MolecularContext => "molecular_context",
            Self::DifferentialMatrix => "differential_matrix",
            Self::LongitudinalTrajectory => "longitudinal_trajectory",
            Self::CranialBaseRiskMap => "cranial_base_risk_map",
            Self::CraniofacialDevelopment => "craniofacial_development",
            Self::SpinalDysraphismMap => "spinal_dysraphism_map",
            Self::CraniocervicalJunctionMap => "craniocervical_junction_map",
            Self::RealDataInventory => "real_data_inventory",
            Self::RealDataQuery => "real_data_query",
            Self::EvidenceSynthesis => "evidence_synthesis",
            Self::HumanReviewHold => "human_review_hold",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::SafetyGate => "safety and permitted-use gate",
            Self::CaseIntegrity => "case integrity and provenance check",
            Self::EvidenceGapScan => "evidence-gap scan",
            Self::ImagingReview => "imaging protocol and limitation review",
            Self::NeuroanatomyMap => "neuroanatomy and corridor map",
            Self::MolecularContext => "molecular context and assay-limitation review",
            Self::DifferentialMatrix => "research differential matrix",
            Self::LongitudinalTrajectory => "longitudinal trajectory review",
            Self::CranialBaseRiskMap => "cranial-base anatomy and risk map",
            Self::CraniofacialDevelopment => "craniofacial developmental trajectory",
            Self::SpinalDysraphismMap => "spinal dysraphism map",
            Self::CraniocervicalJunctionMap => "craniocervical-junction map",
            Self::RealDataInventory => "validated real-data inventory",
            Self::RealDataQuery => "public real-data query",
            Self::EvidenceSynthesis => "provenance-aware evidence synthesis",
            Self::HumanReviewHold => "human review hold",
        }
    }
}

/// Every built-in tool is read-only. There is no action-capable variant in this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolEffect {
    ReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolSpec {
    pub capability: ToolCapability,
    pub label: String,
    pub purpose: String,
    pub effect: ToolEffect,
    pub required_inputs: Vec<ObservationKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub ordinal: u16,
    pub capability: ToolCapability,
    pub purpose: String,
    pub effect: ToolEffect,
    pub requires_human_review: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceState {
    Measured,
    Unmeasured,
    Uninterpretable,
    Conflicting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceGap {
    pub capability: ToolCapability,
    pub state: EvidenceState,
    pub reason: String,
}

/// A caller-actionable, non-clinical research task derived from one explicit evidence gap.
/// `NeedsCallerEvidence` means the declared input is absent; `NeedsHumanReview` means the input
/// exists but is uninterpretable or conflicting. Neither state is a treatment or triage decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchWorkItemStatus {
    NeedsCallerEvidence,
    NeedsHumanReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkItem {
    pub sequence: u16,
    pub capability: ToolCapability,
    pub status: ResearchWorkItemStatus,
    pub evidence_state: EvidenceState,
    pub objective: String,
    pub reason: String,
    pub required_observations: Vec<ObservationKind>,
    pub reviewer_roles: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchHypothesis {
    pub label: String,
    pub status: String,
    pub discriminating_checks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolRunStatus {
    Completed,
    NeedsInput,
    HeldForHumanReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolFinding {
    pub code: String,
    pub detail: String,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolRun {
    pub capability: ToolCapability,
    pub status: ToolRunStatus,
    pub findings: Vec<ToolFinding>,
}

/// Lifecycle state for a caller-persisted, tool-by-tool research session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Planned,
    Running,
    NeedsInput,
    AwaitingHumanReview,
}

/// One deterministic session event. The event and finding digests make a checkpoint tamper
/// evident without retaining raw provider output or requiring hidden server state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionEvent {
    pub ordinal: u16,
    pub capability: ToolCapability,
    pub status: ToolRunStatus,
    pub finding_digest: String,
    pub previous_event_digest: String,
    pub event_digest: String,
}

/// Stateless resumable session state. Persist this value in the caller's store and pass it back
/// to `advance_session`; the agent verifies the request and real-data digests before progressing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalSession {
    pub schema_version: String,
    pub session_id: String,
    pub request_digest: String,
    #[serde(default)]
    pub real_data_digest: Option<String>,
    /// Digest of the optional cross-specialty PubMed bundle bound to this checkpoint.
    /// Exactly one evidence bundle may be attached; the field is optional for backwards
    /// compatibility with checkpoints created before public-literature sessions existed.
    #[serde(default)]
    pub public_literature_digest: Option<String>,
    pub specialty: Specialty,
    pub route: Vec<ToolCapability>,
    pub next_ordinal: u16,
    pub status: SessionStatus,
    pub event_chain_digest: String,
    pub events: Vec<SessionEvent>,
}

/// Result of a bounded in-process autonomous run. The caller receives both the ordinary report
/// and the terminal checkpoint, so it can retain the exact event chain without trusting hidden
/// server memory. `steps_executed` is a measurement of this run, not a promise that every input
/// was sufficient for a human review decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalRunResult {
    pub schema_version: String,
    pub steps_executed: usize,
    pub session: NeurosurgicalSession,
    pub response: AgentResponse,
}

/// Aggregate returned by the provider-free mission helper. It is deliberately a transport
/// envelope around the same typed run result rather than a second clinical interpretation layer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalMissionResult {
    pub schema: String,
    pub mission_id: String,
    pub specialty: Specialty,
    pub status: AgentStatus,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effects: Vec<ToolEffect>,
    pub catalogue: MissionCatalogue,
    /// Optional digest-bound projection of caller-owned real multimodal asset metadata. It is
    /// provenance/intake context only; the mission never opens or interprets asset bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_manifest: Option<crate::CaseAssetManifestReport>,
    /// Optional digest-bound DICOM metadata import receipt. The receipt is a provenance/intake
    /// projection only; pixel bytes and clinical interpretation never enter the mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_dicom_import: Option<crate::DicomCaseImportReport>,
    /// Optional digest-bound FHIR metadata import receipt. The receipt is a de-identification and
    /// provenance projection only; resource payloads and clinical values never enter the mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_fhir_import: Option<crate::FhirCaseImportReport>,
    /// Optional persisted reviewer state bound to the exact case-asset manifest projection.
    /// This is workflow metadata only; dispositions never encode clinical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_asset_review_disposition: Option<crate::CaseAssetReviewDispositionReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_query: Option<RealDataQueryResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_query: Option<crate::PublicLiteratureQueryResult>,
    /// Descriptive audit of the attached real snapshot; no score or clinical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_coverage: Option<crate::RealDataCoverageReport>,
    /// Digest-bound ClinicalTrials.gov metadata landscape for the attached real snapshot. This
    /// is a bounded registry inventory only; it does not rank trials, infer eligibility, or make
    /// efficacy/safety claims.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_trial_landscape: Option<crate::RealDataTrialLandscapeReport>,
    /// Digest-bound cBioPortal assay/profile availability inventory for the attached real
    /// snapshot. It is metadata-only and never exposes patient-level molecular calls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_molecular_coverage: Option<crate::RealDataMolecularCoverageReport>,
    /// Digest-bound comparative inventory of public genomic projects attached to a real-data
    /// mission. Aggregate metadata only; it never exposes patient-level values or files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_cohort_landscape: Option<crate::RealDataCohortLandscapeReport>,
    /// Domain-specific evidence coverage map copied from the terminal route response.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specialty_evidence_map: Option<crate::SpecialtyEvidenceMapReport>,
    /// Explicit metadata-review obligations derived from the attached real snapshot. The queue is
    /// caller-owned and never assigns clinical urgency or edits source facts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_review_queue: Option<crate::RealDataReviewQueueReport>,
    /// One bounded packet containing the real snapshot's summary, coverage, crosswalk, query, and
    /// review obligations for a local model or human reviewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_evidence_packet: Option<crate::RealDataEvidencePacketReport>,
    /// Deterministic, resumable metadata-review wave composed from the real evidence packet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_autonomous_workflow: Option<crate::RealDataAutonomousWorkflowReport>,
    /// Optional caller-clocked retrieval-age posture for the attached real snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_freshness: Option<crate::RealDataFreshnessReport>,
    /// Explicit stable-ID crosswalk for the attached real snapshot; graph adjacency is not a
    /// biological, causal, diagnostic, prognostic, or treatment conclusion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_evidence_graph: Option<crate::EvidenceGraphReport>,
    /// Digest-bound, source-addressable context for a caller-owned local model or reviewer. This
    /// is a bounded evidence handoff, never a generated clinical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_reasoning_context: Option<crate::RealDataReasoningContextReport>,
    /// Digest-bound, source-addressable context for a caller-owned local model or reviewer when
    /// a cross-specialty public-literature bundle backs the mission.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_reasoning_context: Option<crate::PublicLiteratureReasoningContextReport>,
    /// One bounded PMID packet for a public-literature-backed mission. It remains citation context,
    /// not patient evidence or a generated clinical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_evidence_packet: Option<crate::PublicLiteratureEvidencePacketReport>,
    /// Optional caller-clocked retrieval-age posture for the attached public-literature snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_freshness: Option<crate::RealDataFreshnessReport>,
    /// Pre-synthesis source/record completeness and identifier-hygiene audit for the requested
    /// public-literature lane; missingness remains a review obligation, never negative evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_integrity_audit: Option<crate::PublicLiteratureIntegrityAuditReport>,
    /// Caller-owned, source-linked review tasks projected from the integrity audit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_review_queue: Option<crate::PublicLiteratureReviewQueueReport>,
    /// Lane profile plus real snapshot coverage and completeness obligations for the selected
    /// public-literature specialty. This is navigation metadata, not a readiness or quality score.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_workbench: Option<crate::PublicLiteratureWorkbenchReport>,
    /// Optional bounded multi-lane public-literature handoff. It preserves exact query results,
    /// lane coverage, and reviewer queues without ranking evidence or inferring a clinical result.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_portfolio: Option<crate::PublicLiteraturePortfolioReport>,
    /// Optional exact PMID/DOI crosswalk when a glioma mission binds both the public glioma
    /// snapshot and the cross-specialty PubMed snapshot. Links are metadata reconciliation only;
    /// they do not assert cohort identity, comparability, biology, or causality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub literature_link_audit: Option<crate::LiteratureLinkAuditReport>,
    /// Cross-plane digest-bound ledger for the mission's attached case/public evidence. The
    /// ledger keeps case observations, caller evidence, population records, and citations
    /// separate; it is a reviewer handoff, never a generated clinical conclusion.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_synthesis: Option<crate::EvidenceSynthesisReport>,
    /// Ordered, source-linked next-review work derived from the exact request and optional
    /// evidence bundle. Tasks are caller-owned research proposals, never clinical instructions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_plan: Option<crate::ResearchPlanReport>,
    /// Protocol-defined, source-grounded review tracks projected from the attached public
    /// snapshots. Matches are retrieval observations only; the program never emits a clinical
    /// interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_program: Option<crate::EvidenceProgramReport>,
    /// Bounded local replay work derived from the same request and evidence bundle. This is a
    /// caller-owned checkpointable worker plan, never a fetch schedule or clinical instruction.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_acquisition: Option<crate::EvidenceAcquisitionReport>,
    /// Initial digest-bound checkpoint for the acquisition plan. The caller owns later advances
    /// and persistence; this session contains no source records or asset bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_acquisition_session: Option<crate::EvidenceAcquisitionSession>,
    /// Deterministic topic extraction over the same validated public bundle. This is a structured
    /// review handoff, not a generated medical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub research_brief: Option<crate::NeurosurgicalResearchBriefReport>,
    /// Final digest and boundary fuse over the composed mission planes. This is an integrity
    /// receipt, not a clinical quality score or readiness decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission_audit: Option<crate::MissionAuditReport>,
    pub run: NeurosurgicalRunResult,
}

/// Stable catalogue counts included in a mission response for audit and UI discovery.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissionCatalogue {
    pub specialty_count: usize,
    pub tool_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    ReadyForHumanReview,
    NeedsEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchReport {
    pub non_clinical_use_notice: String,
    pub scope: String,
    pub observed_finding_count: usize,
    pub evidence_record_count: usize,
    pub known_inputs: Vec<String>,
    pub uncertainties: Vec<String>,
    pub next_research_questions: Vec<String>,
    #[serde(default)]
    pub research_worklist: Vec<ResearchWorkItem>,
    pub prohibited_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentResponse {
    pub schema_version: String,
    /// Digest over the complete response with this field cleared. This is an integrity receipt,
    /// not a clinical quality score or authorization for downstream action.
    #[serde(default)]
    pub response_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub specialty_profile: SpecialtyProfile,
    pub status: AgentStatus,
    pub plan: Vec<PlanStep>,
    pub tool_runs: Vec<ToolRun>,
    pub evidence_gaps: Vec<EvidenceGap>,
    pub hypotheses: Vec<ResearchHypothesis>,
    pub report: ResearchReport,
    /// Population-level real-data provenance attached by `run_with_real_glioma_data`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data: Option<RealDataSummary>,
    /// Cross-specialty PubMed provenance attached by `run_with_public_literature`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature: Option<PublicLiteratureSummary>,
    /// Descriptive temporal alignment of caller-supplied observations. Missing dates and input
    /// order conflicts remain visible; this is not a trajectory, prognosis, or clinical finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal_alignment: Option<crate::TemporalAlignmentReport>,
    /// Domain-specific identity/spatial/functional/temporal coverage for the selected specialty.
    /// This is an input inventory and reviewer handoff, never a clinical interpretation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub specialty_evidence_map: Option<crate::SpecialtyEvidenceMapReport>,
    /// Optional typed glioma molecular inventory; it is evidence coverage, never a diagnosis.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glioma_molecular: Option<crate::GliomaMolecularSummary>,
}

/// A compact, verifiable summary of an authoritative public-data bundle used for a run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataSummary {
    pub bundle_schema_version: String,
    pub bundle_digest: String,
    pub source_count: usize,
    pub record_count: usize,
    pub clinical_trial_count: usize,
    pub recruiting_trial_count: usize,
    pub completed_trial_count: usize,
    pub genomic_project_count: usize,
    pub genomic_case_count: usize,
    /// Per-project aggregate case counts copied from the public genomic catalogue. These are
    /// provenance/coverage metadata and never represent caller or patient records.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genomic_project_case_counts: Vec<crate::real_data::RealGenomicProjectCaseCount>,
    /// Aggregate GDC file/data-type availability copied from the public project facets. These
    /// rows describe source coverage only and never contain sample identifiers or assay values.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub genomic_project_data_type_counts: Vec<crate::real_data::RealGenomicProjectDataTypeCount>,
    pub portal_study_count: usize,
    #[serde(default)]
    pub portal_molecular_profile_count: usize,
    /// Number of explicit study/profile/publication crosswalks derivable from the bundle.
    #[serde(default)]
    pub relationship_count: usize,
    pub portal_sample_count: usize,
    pub public_pmid_count: usize,
    pub reference_count: usize,
    #[serde(default)]
    pub literature_article_count: usize,
    #[serde(default)]
    pub literature_abstract_count: usize,
    #[serde(default)]
    pub literature_abstract_truncated_count: usize,
    #[serde(default)]
    pub portal_literature_linked_count: usize,
    #[serde(default)]
    pub portal_literature_unlinked_count: usize,
    #[serde(default)]
    pub literature_without_portal_count: usize,
    #[serde(default)]
    pub portal_study_without_pmid_count: usize,
    #[serde(default)]
    pub trial_status_counts: Vec<crate::real_data::RealTrialStatusCount>,
    #[serde(default)]
    pub portal_profile_type_counts: Vec<crate::real_data::RealMolecularProfileTypeCount>,
    #[serde(default)]
    pub latest_trial_update: Option<String>,
    /// Number of registry records carrying an explicit study type. This is source metadata
    /// coverage, not a trial-quality or eligibility measure.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trial_study_type_count: usize,
    /// Number of registry records carrying an aggregate enrollment target. It is never an
    /// enrolled-patient or patient-level count.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trial_enrollment_count: usize,
    /// Number of registry records carrying one or more intervention names. Names remain source
    /// metadata and are not ranked or interpreted as recommendations.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trial_intervention_count: usize,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
}

/// A request document accepted by the local JSON entry point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaseRequest {
    #[serde(default = "default_schema_version")]
    pub schema_version: String,
    pub case_id: String,
    pub specialty: Specialty,
    pub request_use: RequestUse,
    pub question: String,
    #[serde(default)]
    pub direct_identifier_fields: Vec<String>,
    #[serde(default)]
    pub observations: Vec<Observation>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRecord>,
    /// An empty list means the agent chooses the complete specialty route.
    #[serde(default)]
    pub requested_tools: Vec<ToolCapability>,
    /// Optional explicit query used only when `RealDataQuery` is requested with a real bundle.
    #[serde(default)]
    pub real_data_query: Option<crate::real_data::RealDataQuery>,
    /// Optional structured glioma molecular evidence; accepted only for the glioma specialty.
    #[serde(default)]
    pub glioma_molecular: Option<GliomaMolecularPanel>,
}
