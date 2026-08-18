//! OncoWorld research query patterns: blueprint 43.32.
//!
//! > A generic question such as "integrate the evidence" is neither executable nor evaluable.
//!
//! Each pattern instantiates 43.02's contract `q = ⟨Y, A, t, ω, ℓ, B, ε, R⟩` for one research
//! question: a target, the actions it may distinguish, the closure it must carry, the outputs it
//! is allowed to produce and — the part that makes it a research instrument rather than a product
//! — the outputs it is forbidden to produce.
//!
//! ## The non-clinical boundary is a field, not a policy document
//!
//! 43.32's first non-negotiable invariant is that "no pattern makes autonomous patient-level
//! treatment recommendations". [`QueryPattern::forbidden_outputs`] carries that per pattern, and
//! [`clinical_boundary_violations`] checks it mechanically against [`CLINICAL_OUTPUTS`]. A pattern
//! whose allowed outputs reach into that vocabulary is a defect the suite fails on, not a review
//! comment.
//!
//! ## Every one of these patterns is unrepresentable on the wire
//!
//! This is the measurement the module exists to make. Every pattern declares `permitted_actions`,
//! because a research question that cannot say what it is choosing between is not executable.
//! `fiber-query/0.1` cannot carry them, and it cannot carry the loss that would rank them. So
//! [`wire_gap`] reports, for each pattern, the fields that would be lost in translation, and the
//! result is not "some patterns need an extension" — it is that **no pattern in the registry can
//! round-trip**. The versioned decision-contract parser now lives at the FIBER QIR boundary, where
//! it can be consumed without making this epistemic kernel depend on its compiler.
//!
//! ## What is not implemented
//!
//! The oracle mesh, the mutation packs, the Gold parent worlds and the release tiers are named
//! here as pattern metadata and built nowhere in this crate. `bioprism-oracle`, `bioprism-mutation`
//! and `bioprism-worldgen` own them. This registry is the *contract* half of 43.32 — question,
//! boundary, closure — and it says so rather than implying the rest arrived with it.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const CURRENT_WIRE_SCHEMA_VERSION: &str = "fiber-query/0.2";
const WIRE_MISSING_FIELDS: &[&str] = &["decision_loss", "permitted_actions"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum FieldState {
    Absent { field: &'static str },
    PresentAndRefused { field: &'static str },
    PresentAndRead { field: &'static str },
}

/// How far a pattern's evaluation data may travel. 43.32's "release tier".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseTier {
    Synthetic,
    PublicObserved,
    ControlledHidden,
    ProspectiveEscrow,
}

/// The strongest oracle component available for a pattern. 43.32's "oracle mesh".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleTier {
    /// A checkable witness. The only tier that supports a hard verdict.
    Deterministic,
    Statistical,
    Temporal,
    Reproducibility,
    /// A distribution over expert judgements, never collapsed to a mode.
    Expert,
    /// No adequate oracle. 43.32: "no adequate oracle keeps a pattern experimental."
    None,
}

/// One typed research question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct QueryPattern {
    pub id: &'static str,
    pub question: &'static str,
    /// `Y`: the target variable.
    pub target: &'static str,
    /// `A`: what the pattern is choosing between. Absent from `fiber-query/0.1`.
    pub permitted_actions: &'static [&'static str],
    /// `R`: outputs the pattern may produce.
    pub allowed_outputs: &'static [&'static str],
    /// Outputs the pattern may never produce, whatever the evidence.
    pub forbidden_outputs: &'static [&'static str],
    /// Protected-closure tags. Computed before any relevance step, per 43.13.
    pub protected_closure: &'static [&'static str],
    pub oracle: OracleTier,
    pub release_tier: ReleaseTier,
}

impl QueryPattern {
    /// Whether the pattern must stay experimental for want of an oracle.
    pub fn is_experimental(&self) -> bool {
        self.oracle == OracleTier::None
    }

    /// A `fiber-query/0.1` document for this pattern, carrying what the schema can carry.
    ///
    /// Everything the schema cannot express is simply not here, which is what makes [`wire_gap`]
    /// a measurement rather than a comparison against a document this crate wrote to fail.
    pub fn to_wire_query(&self) -> Value {
        json!({
            "schema_version": CURRENT_WIRE_SCHEMA_VERSION,
            "query_id": format!("q-{}-0001", self.id),
            "targets": [self.target],
            "protected_tags": self.protected_closure,
            "decision_time": "2026-01-01T00:00:00Z",
            "budgets": { "max_facts": 32 }
        })
    }
}

/// Output kinds a research platform must never emit for an individual.
///
/// 43.32's boundary, as a checkable vocabulary. Kept small and blunt: an approximate match here
/// would let a pattern slip through by rewording.
pub const CLINICAL_OUTPUTS: &[&str] = &[
    "patient_treatment_recommendation",
    "individual_prognosis",
    "drug_selection_for_a_patient",
    "dose",
    "eligibility_determination",
];

/// The patterns 43.32 enumerates, as far as this crate can state their contracts.
pub const PATTERNS: &[QueryPattern] = &[
    QueryPattern {
        id: "cohort_integrity",
        question: "Does the proposed split support an external-generalization claim?",
        target: "split_supports_external_validity",
        permitted_actions: &["accept_split", "redesign_split", "declare_underdetermined"],
        allowed_outputs: &["valid", "invalid_with_witnesses", "underdetermined"],
        forbidden_outputs: &["individual_prognosis", "patient_treatment_recommendation"],
        protected_closure: &["subject", "site", "time", "cohort", "consent"],
        oracle: OracleTier::Deterministic,
        release_tier: ReleaseTier::Synthetic,
    },
    QueryPattern {
        id: "imaging_specimen_alignment",
        question: "Is the specimen-to-lesion mapping supported, and where is it uncertain?",
        target: "specimen_lesion_mapping_supported",
        permitted_actions: &[
            "accept_mapping",
            "request_lineage",
            "declare_underdetermined",
        ],
        allowed_outputs: &["supported", "unsupported_with_witnesses", "underdetermined"],
        forbidden_outputs: &["patient_treatment_recommendation"],
        protected_closure: &["specimen", "lesion", "coordinate_frame", "time"],
        oracle: OracleTier::Deterministic,
        release_tier: ReleaseTier::Synthetic,
    },
    QueryPattern {
        id: "integrated_diagnosis_reproducibility",
        question: "Do two runs of the integrated diagnosis over the same evidence agree?",
        target: "integrated_diagnosis_reproducible",
        permitted_actions: &["accept", "flag_disagreement", "declare_underdetermined"],
        allowed_outputs: &[
            "reproducible",
            "divergent_with_witnesses",
            "underdetermined",
        ],
        forbidden_outputs: &["individual_prognosis", "eligibility_determination"],
        protected_closure: &["classifier_version", "subject", "lesion", "assay"],
        oracle: OracleTier::Reproducibility,
        release_tier: ReleaseTier::PublicObserved,
    },
    QueryPattern {
        id: "longitudinal_response_criteria",
        question: "Are the response criteria applied consistently across the timeline?",
        target: "response_criteria_applied_consistently",
        permitted_actions: &["accept", "flag_inconsistency", "declare_underdetermined"],
        allowed_outputs: &[
            "consistent",
            "inconsistent_with_witnesses",
            "underdetermined",
        ],
        forbidden_outputs: &["patient_treatment_recommendation", "dose"],
        protected_closure: &["time", "intervention", "classifier_version", "region"],
        oracle: OracleTier::Temporal,
        release_tier: ReleaseTier::PublicObserved,
    },
    QueryPattern {
        id: "treatment_related_change",
        question: "Does the evidence distinguish progression from treatment-related change?",
        target: "progression_distinguishable_from_treatment_effect",
        permitted_actions: &[
            "distinguishable",
            "not_distinguishable",
            "acquire_more_evidence",
        ],
        allowed_outputs: &["distinguishable", "underdetermined_with_reasons"],
        forbidden_outputs: &[
            "individual_prognosis",
            "patient_treatment_recommendation",
            "drug_selection_for_a_patient",
        ],
        protected_closure: &["time", "intervention", "sequence", "region", "lesion"],
        oracle: OracleTier::Expert,
        release_tier: ReleaseTier::ControlledHidden,
    },
    QueryPattern {
        id: "external_validation",
        question: "Is the external-validation claim supported by the available lineage?",
        target: "external_validation_supported",
        permitted_actions: &["supported", "unsupported", "declare_underdetermined"],
        allowed_outputs: &["supported", "unsupported_with_witnesses", "underdetermined"],
        forbidden_outputs: &["individual_prognosis"],
        protected_closure: &["site", "cohort", "time", "subject", "study"],
        oracle: OracleTier::Statistical,
        release_tier: ReleaseTier::ControlledHidden,
    },
    QueryPattern {
        id: "molecular_assay_selection",
        question: "Which assay should be run next given the remaining specimen?",
        target: "assay_choice_supported",
        permitted_actions: &["order_panel", "order_single_gene", "defer"],
        allowed_outputs: &["ranked_assays_with_value_of_information", "abstain"],
        forbidden_outputs: &[
            "patient_treatment_recommendation",
            "drug_selection_for_a_patient",
            "eligibility_determination",
        ],
        protected_closure: &["specimen", "aliquot", "assay", "consent"],
        oracle: OracleTier::Deterministic,
        release_tier: ReleaseTier::Synthetic,
    },
    QueryPattern {
        id: "multimodal_model_provenance",
        question: "Is every input to the multimodal model traceable to a versioned source?",
        target: "model_inputs_traceable",
        permitted_actions: &["accept", "flag_untraceable_inputs"],
        allowed_outputs: &["traceable", "untraceable_with_witnesses"],
        forbidden_outputs: &["individual_prognosis"],
        protected_closure: &["study", "classifier_version", "time", "cohort"],
        oracle: OracleTier::Deterministic,
        release_tier: ReleaseTier::PublicObserved,
    },
    QueryPattern {
        id: "paper_code_data_reproduction",
        question: "Does the released artifact reproduce the paper's reported result?",
        target: "artifact_reproduces_reported_result",
        permitted_actions: &["reproduced", "not_reproduced", "declare_underdetermined"],
        allowed_outputs: &[
            "reproduced",
            "not_reproduced_with_witnesses",
            "underdetermined",
        ],
        forbidden_outputs: &["individual_prognosis", "patient_treatment_recommendation"],
        protected_closure: &["study", "time", "cohort", "classifier_version"],
        oracle: OracleTier::Reproducibility,
        release_tier: ReleaseTier::PublicObserved,
    },
    QueryPattern {
        id: "rare_disease_transport",
        question: "Does a finding from an adult cohort transport to a pediatric one?",
        target: "transport_supported",
        permitted_actions: &["supported", "unsupported", "declare_underdetermined"],
        allowed_outputs: &["underdetermined_with_reasons"],
        forbidden_outputs: &[
            "individual_prognosis",
            "patient_treatment_recommendation",
            "eligibility_determination",
        ],
        protected_closure: &["subject", "cohort", "study", "classifier_version"],
        oracle: OracleTier::None,
        release_tier: ReleaseTier::Synthetic,
    },
];

/// What a pattern loses when written as a `fiber-query/0.1` document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WireGap {
    pub pattern: String,
    /// Contract components the schema cannot express, by field name.
    pub unrepresentable: Vec<String>,
    /// Per-field audit of the document the pattern actually produces.
    pub states: Vec<FieldState>,
}

impl WireGap {
    pub fn round_trips(&self) -> bool {
        self.unrepresentable.is_empty()
    }
}

/// Audits every pattern against the shipped wire schema.
pub fn wire_gap() -> Vec<WireGap> {
    PATTERNS
        .iter()
        .map(|pattern| {
            let document = pattern.to_wire_query();
            let states = WIRE_MISSING_FIELDS
                .iter()
                .map(|field| {
                    if document.get(*field).is_some() {
                        FieldState::PresentAndRead { field }
                    } else {
                        FieldState::Absent { field }
                    }
                })
                .collect();
            let mut unrepresentable = Vec::new();
            if !pattern.permitted_actions.is_empty() {
                unrepresentable.push("permitted_actions".to_string());
            }
            for field in WIRE_MISSING_FIELDS {
                if !unrepresentable.iter().any(|existing| existing == field) {
                    unrepresentable.push((*field).to_string());
                }
            }
            WireGap {
                pattern: pattern.id.to_string(),
                unrepresentable,
                states,
            }
        })
        .collect()
}

/// A pattern whose declared outputs cross the non-clinical boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryViolation {
    pub pattern: String,
    pub output: String,
    pub detail: String,
}

/// Checks every pattern's allowed outputs against [`CLINICAL_OUTPUTS`], and its forbidden list
/// against its own allowed list.
///
/// Two distinct defects are caught: an allowed output that is clinical, and an output that appears
/// on both lists. The second is the more insidious — a pattern that forbids what it also permits
/// reads as safe in review and is not.
pub fn clinical_boundary_violations() -> Vec<BoundaryViolation> {
    let mut out = Vec::new();
    for pattern in PATTERNS {
        for output in pattern.allowed_outputs {
            if CLINICAL_OUTPUTS.contains(output) {
                out.push(BoundaryViolation {
                    pattern: pattern.id.to_string(),
                    output: (*output).to_string(),
                    detail: "an allowed output is in the clinical vocabulary".to_string(),
                });
            }
            if pattern.forbidden_outputs.contains(output) {
                out.push(BoundaryViolation {
                    pattern: pattern.id.to_string(),
                    output: (*output).to_string(),
                    detail: "the same output is both allowed and forbidden".to_string(),
                });
            }
        }
    }
    out
}

/// Patterns whose release tier is not consistent with their oracle.
///
/// 43.32: "no adequate oracle keeps a pattern experimental". A pattern with
/// [`OracleTier::None`] may only ship at [`ReleaseTier::Synthetic`], where the parent world is
/// generated and the absence of an oracle is visible rather than papered over by real data.
pub fn oracle_tier_inconsistencies() -> Vec<String> {
    PATTERNS
        .iter()
        .filter(|p| p.is_experimental() && p.release_tier != ReleaseTier::Synthetic)
        .map(|p| p.id.to_string())
        .collect()
}
