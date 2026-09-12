//! Deterministic natural-language intake for the neurosurgical research routes.
//!
//! This is a routing aid, not a medical classifier. It scores a bounded vocabulary of explicit
//! specialty terms, abstains when the signal is weak or ambiguous, and returns only the closed
//! research route plus the evidence bundle types a caller may consider supplying. It does not
//! retain the question text in the plan, call a model, fetch a source, or infer a diagnosis.

use crate::catalogue::required_capabilities;
use crate::{CaseRequest, NeurosurgeryError, Specialty, ToolCapability, ToolEffect};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

/// Version of the provider-free specialty-intake contract.
pub const NEUROSURGERY_INTAKE_SCHEMA_VERSION: &str = "bioprism-neurosurgery-intake-plan/0.1";
/// Version of the question-to-mission composition contract.
pub const NEUROSURGERY_INTAKE_MISSION_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-intake-mission/0.1";
/// Version of the multi-specialty intake portfolio contract.
pub const NEUROSURGERY_INTAKE_PORTFOLIO_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-intake-portfolio/0.1";

const MAX_QUESTION_BYTES: usize = 4_000;
const MAX_CANDIDATES: usize = 6;
const MAX_MATCHED_TERMS: usize = 16;
const MIN_CONFIDENCE_BPS: u16 = 250;
const MIN_MARGIN_BPS: u16 = 100;

/// A short natural-language research question used only for deterministic specialty routing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakeQuery {
    pub question: String,
    /// An explicit specialty bypasses lexical ambiguity but does not bypass the research boundary.
    #[serde(default)]
    pub specialty: Option<Specialty>,
    #[serde(default = "default_max_candidates")]
    pub max_candidates: usize,
    /// Optional de-identified structured case to carry into the guarded mission. The case is
    /// validated by the agent only after intake selects a route and is never serialized in the
    /// intake plan/result; callers receive a digest and bounded review output instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub case_request: Option<CaseRequest>,
}

fn default_max_candidates() -> usize {
    MAX_CANDIDATES
}

impl Default for NeurosurgicalIntakeQuery {
    fn default() -> Self {
        Self {
            question: String::new(),
            specialty: None,
            max_candidates: default_max_candidates(),
            case_request: None,
        }
    }
}

/// Bounded natural-language intake controls for a cross-specialty public-evidence portfolio.
/// `include_all_specialties` is an explicit corpus-reconnaissance override; it never authorizes
/// clinical use and does not turn an ambiguous question into a single specialty decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakePortfolioQuery {
    #[serde(flatten)]
    pub intake: NeurosurgicalIntakeQuery,
    #[serde(default)]
    pub include_all_specialties: bool,
    #[serde(default = "default_portfolio_hits_per_lane")]
    pub max_hits_per_lane: usize,
    #[serde(default = "default_portfolio_review_items_per_lane")]
    pub max_review_items_per_lane: usize,
    #[serde(default = "default_portfolio_issues_per_lane")]
    pub max_issues_per_lane: usize,
    #[serde(default = "default_portfolio_session_steps")]
    pub max_session_steps: usize,
}

fn default_portfolio_hits_per_lane() -> usize {
    16
}

fn default_portfolio_review_items_per_lane() -> usize {
    32
}

fn default_portfolio_issues_per_lane() -> usize {
    128
}

fn default_portfolio_session_steps() -> usize {
    crate::MAX_SESSION_STEPS
}

impl Default for NeurosurgicalIntakePortfolioQuery {
    fn default() -> Self {
        Self {
            intake: NeurosurgicalIntakeQuery::default(),
            include_all_specialties: false,
            max_hits_per_lane: default_portfolio_hits_per_lane(),
            max_review_items_per_lane: default_portfolio_review_items_per_lane(),
            max_issues_per_lane: default_portfolio_issues_per_lane(),
            max_session_steps: default_portfolio_session_steps(),
        }
    }
}

/// One lexical specialty candidate. The score is basis points in the closed interval 0..=1000;
/// it is a routing signal and must not be interpreted as probability, severity, or confidence in
/// a clinical finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakeCandidate {
    pub specialty: Specialty,
    pub score_bps: u16,
    pub matched_terms: Vec<String>,
}

/// Provider-free specialty routing and next-step handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakePlan {
    pub schema_version: String,
    pub plan_digest: String,
    pub question_digest: String,
    pub candidates: Vec<NeurosurgicalIntakeCandidate>,
    pub selected_specialty: Option<Specialty>,
    pub confidence_bps: u16,
    pub abstained: bool,
    pub reason: String,
    pub route: Vec<ToolCapability>,
    /// Names of caller-supplied snapshot classes; this is not a claim that a snapshot is present.
    pub evidence_sources: Vec<String>,
    pub reviewer_roles: Vec<String>,
    pub next_actions: Vec<String>,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: ToolEffect,
    pub limitations: Vec<String>,
}

/// Terminal status for an intake-driven mission handoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NeurosurgicalIntakeMissionStatus {
    Abstained,
    NeedsEvidence,
    ReadyForHumanReview,
}

/// Question-to-mission composition result. A selected question is converted internally into a
/// research-synthesis `CaseRequest`, or carries a caller-supplied de-identified case through the
/// same validation path; neither raw request text nor case payload is serialized in this result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakeMissionResult {
    pub schema_version: String,
    pub intake: NeurosurgicalIntakePlan,
    pub status: NeurosurgicalIntakeMissionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<crate::NeurosurgicalMissionResult>,
    /// Snapshot classes required before a mission can execute; this is a handoff obligation,
    /// never a claim that the caller supplied the snapshots.
    pub required_evidence: Vec<String>,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: ToolEffect,
    pub limitations: Vec<String>,
}

/// Cross-specialty evidence portfolio composed from one bounded question and one validated
/// public-literature snapshot. A single selected route may also carry a nested mission; broad
/// portfolios intentionally stop at citation/workbench review rather than inventing a combined
/// clinical route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeurosurgicalIntakePortfolioResult {
    pub schema_version: String,
    pub intake: NeurosurgicalIntakePlan,
    pub status: NeurosurgicalIntakeMissionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mission: Option<crate::NeurosurgicalMissionResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub portfolio: Option<crate::PublicLiteraturePortfolioReport>,
    pub selected_specialties: Vec<Specialty>,
    pub required_evidence: Vec<String>,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: ToolEffect,
    pub limitations: Vec<String>,
}

/// Build a digest-bound, abstaining specialty intake plan.
pub fn plan(
    query: &NeurosurgicalIntakeQuery,
) -> Result<NeurosurgicalIntakePlan, NeurosurgeryError> {
    validate_query(query)?;
    let question_digest = digest_text(&query.question);

    let mut candidates = if let Some(specialty) = query.specialty {
        vec![NeurosurgicalIntakeCandidate {
            specialty,
            score_bps: 1_000,
            matched_terms: vec!["caller_explicit_specialty".to_string()],
        }]
    } else {
        let normalized = normalize(&query.question);
        let mut scored = Specialty::ALL
            .iter()
            .copied()
            .filter_map(|specialty| {
                let mut matched = specialty_terms(specialty)
                    .iter()
                    .filter(|term| contains_term(&normalized, term))
                    .map(|term| normalize(term))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>();
                matched.sort();
                matched.truncate(MAX_MATCHED_TERMS);
                if matched.is_empty() {
                    return None;
                }
                let points = matched
                    .iter()
                    .map(|term| {
                        // Canonical disease anchors such as "glioma" and "chiari" are
                        // intentionally strong enough to select a single lane on their own.
                        // Short generic terms still require corroborating vocabulary, preserving
                        // abstention for vague questions without making the obvious one-word
                        // specialty request unusable.
                        if term.contains(' ') || term.len() >= 6 {
                            2_u16
                        } else {
                            1
                        }
                    })
                    .sum::<u16>();
                Some(NeurosurgicalIntakeCandidate {
                    specialty,
                    score_bps: points.saturating_mul(125).min(1_000),
                    matched_terms: matched,
                })
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .score_bps
                .cmp(&left.score_bps)
                .then_with(|| left.specialty.slug().cmp(right.specialty.slug()))
        });
        scored
    };
    candidates.truncate(query.max_candidates);

    let (selected_specialty, confidence_bps, abstained, reason) = if query.specialty.is_some() {
        (
            query.specialty,
            1_000,
            false,
            "explicit_specialty".to_string(),
        )
    } else if candidates.is_empty() {
        (None, 0, true, "no_matching_specialty".to_string())
    } else {
        let top = candidates[0].score_bps;
        let second = candidates.get(1).map(|candidate| candidate.score_bps);
        if top < MIN_CONFIDENCE_BPS {
            (None, top, true, "insufficient_confidence".to_string())
        } else if second.is_some_and(|score| top.saturating_sub(score) < MIN_MARGIN_BPS) {
            (None, top, true, "insufficient_margin".to_string())
        } else {
            (
                Some(candidates[0].specialty),
                top,
                false,
                "selected".to_string(),
            )
        }
    };

    let route = selected_specialty
        .map(required_capabilities)
        .unwrap_or_default();
    let evidence_sources = selected_specialty
        .map(|specialty| match specialty {
            Specialty::Glioma => vec![
                "real_glioma_snapshot".to_string(),
                "pubmed_snapshot".to_string(),
            ],
            _ => vec!["pubmed_snapshot".to_string()],
        })
        .unwrap_or_default();
    let reviewer_roles = selected_specialty
        .map(|specialty| specialty.profile().human_review_roles)
        .unwrap_or_default();
    let next_actions = next_actions(selected_specialty, abstained);
    let plan_digest = digest_plan(
        &question_digest,
        &candidates,
        selected_specialty,
        confidence_bps,
        abstained,
        &reason,
        &route,
        &evidence_sources,
        &reviewer_roles,
        &next_actions,
    )?;

    Ok(NeurosurgicalIntakePlan {
        schema_version: NEUROSURGERY_INTAKE_SCHEMA_VERSION.to_string(),
        plan_digest,
        question_digest,
        candidates,
        selected_specialty,
        confidence_bps,
        abstained,
        reason,
        route,
        evidence_sources,
        reviewer_roles,
        next_actions,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: ToolEffect::ReadOnly,
        limitations: vec![
            "routing is lexical vocabulary matching, not a diagnostic or clinical classifier"
                .to_string(),
            "scores are routing units, not probabilities, severity, evidence quality, or patient risk"
                .to_string(),
            "an explicit specialty selects a research route but never authorizes clinical use"
                .to_string(),
            "the plan does not fetch sources, inspect patient files, invoke a model, or retain the question text"
                .to_string(),
        ],
    })
}

fn validate_query(query: &NeurosurgicalIntakeQuery) -> Result<(), NeurosurgeryError> {
    if query.question.trim().is_empty() {
        return Err(NeurosurgeryError::EmptyField { field: "question" });
    }
    if query.question.len() > MAX_QUESTION_BYTES {
        return Err(NeurosurgeryError::FieldTooLong {
            field: "question",
            max: MAX_QUESTION_BYTES,
        });
    }
    if query.question.chars().any(char::is_control) {
        return Err(NeurosurgeryError::ControlCharacter { field: "question" });
    }
    if query.max_candidates == 0 || query.max_candidates > MAX_CANDIDATES {
        return Err(NeurosurgeryError::TooMany {
            field: "max_candidates",
            found: query.max_candidates,
            max: MAX_CANDIDATES,
        });
    }
    Ok(())
}

fn specialty_terms(specialty: Specialty) -> &'static [&'static str] {
    match specialty {
        Specialty::Glioma => &[
            "glioma",
            "glioblastoma",
            "diffuse glioma",
            "astrocytoma",
            "oligodendroglioma",
            "diffuse midline glioma",
            "brain tumour",
            "brain tumor",
            "neuro oncology",
            "neuro-oncology",
            "idh",
            "idh1",
            "idh2",
            "h3 k27",
            "h3 g34",
            "1p/19q",
            "mgmt",
            "tert",
            "egfr",
            "cdkn2a",
            "chromosome 7 gain",
            "chromosome 10 loss",
            "h3",
            "methylation",
            "methylation classifier",
            "radiation necrosis",
            "pseudoprogression",
            "awake mapping",
            "language mapping",
            "supratotal resection",
            "stupp",
        ],
        Specialty::CranialBase => &[
            "cranial base",
            "skull base",
            "clivus",
            "petroclival",
            "petrous apex",
            "cavernous sinus",
            "meckel cave",
            "petrous",
            "sellar",
            "sphenoid",
            "planum sphenoidale",
            "tuberculum sellae",
            "jugular foramen",
            "endoscopic endonasal",
            "cranial nerve",
            "csf leak",
        ],
        Specialty::Craniosynostosis => &[
            "craniosynostosis",
            "craniofacial",
            "cranial suture",
            "sagittal suture",
            "metopic",
            "trigonocephaly",
            "brachycephaly",
            "scaphocephaly",
            "plagiocephaly",
            "oxycephaly",
            "apert syndrome",
            "crouzon syndrome",
            "pfeiffer syndrome",
            "midface hypoplasia",
            "papilledema",
            "intracranial pressure",
        ],
        Specialty::Encephalocele => &[
            "encephalocele",
            "meningoencephalocele",
            "meningocele",
            "neural tube defect",
            "skull defect",
            "occipital encephalocele",
            "basal encephalocele",
            "nasal glioma",
            "transsphenoidal encephalocele",
            "frontonasal dysplasia",
            "csf rhinorrhea",
        ],
        Specialty::SpinaBifida => &[
            "spina bifida",
            "spinal dysraphism",
            "myelomeningocele",
            "lipomyelomeningocele",
            "tethered cord",
            "neural tube defect",
            "lipomeningocele",
            "neurogenic bladder",
            "urodynamics",
            "sacral agenesis",
            "split cord",
            "diastematomyelia",
            "hydrocephalus",
            "myelocystocele",
        ],
        Specialty::ChiariMalformation => &[
            "chiari",
            "chiari malformation",
            "craniocervical junction",
            "tonsillar ectopia",
            "syringomyelia",
            "syringobulbia",
            "tonsillar descent",
            "cerebellar tonsil",
            "basilar invagination",
            "clivo-axial angle",
            "pb-c2",
            "cine mri",
            "csf flow",
            "atlantoaxial instability",
            "foramen magnum",
        ],
    }
}

fn normalize(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if pending_space && !output.is_empty() {
                output.push(' ');
            }
            pending_space = false;
            output.push(character.to_ascii_lowercase());
        } else {
            pending_space = true;
        }
    }
    output
}

fn contains_term(normalized: &str, term: &str) -> bool {
    let needle = normalize(term);
    if needle.is_empty() {
        return false;
    }
    format!(" {normalized} ").contains(&format!(" {needle} "))
}

fn next_actions(specialty: Option<Specialty>, abstained: bool) -> Vec<String> {
    if abstained {
        return vec![
            "Resolve the specialty ambiguity or provide an explicit specialty before constructing a mission."
                .to_string(),
            "Keep the request purpose explicit as research_synthesis or educational_review."
                .to_string(),
            "Do not treat this abstention as evidence for or against any disease or procedure."
                .to_string(),
        ];
    }
    let mut actions = vec![
        "Construct a CaseRequest with an explicit research_synthesis or educational_review purpose."
            .to_string(),
        "Supply de-identified observations and provenance identifiers; missingness remains explicit."
            .to_string(),
    ];
    if specialty == Some(Specialty::Glioma) {
        actions.push(
            "Bind a validated real glioma snapshot and, when useful, an independent PubMed snapshot."
                .to_string(),
        );
    } else {
        actions
            .push("Bind a validated PubMed snapshot for the selected specialty lane.".to_string());
    }
    actions.push("Run the bounded route and stop at the required human-review hold.".to_string());
    actions
}

fn digest_text(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[allow(clippy::too_many_arguments)]
fn digest_plan(
    question_digest: &str,
    candidates: &[NeurosurgicalIntakeCandidate],
    selected_specialty: Option<Specialty>,
    confidence_bps: u16,
    abstained: bool,
    reason: &str,
    route: &[ToolCapability],
    evidence_sources: &[String],
    reviewer_roles: &[String],
    next_actions: &[String],
) -> Result<String, NeurosurgeryError> {
    let value = (
        NEUROSURGERY_INTAKE_SCHEMA_VERSION,
        question_digest,
        candidates,
        selected_specialty,
        confidence_bps,
        abstained,
        reason,
        route,
        evidence_sources,
        reviewer_roles,
        next_actions,
    );
    let bytes =
        serde_json::to_vec(&value).map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}
