//! Deterministic, source-linked research planning for the neurosurgical routes.
//!
//! This is a planning projection, not an acquisition engine. It turns the explicit intake audit
//! into bounded caller tasks and, when a validated snapshot is supplied, attaches local
//! population/citation candidates for human review. No source is fetched, and no population row
//! is promoted to a patient finding. The planner intentionally keeps "no matching source" apart
//! from "no source query was applicable" by carrying an optional query and optional match counts.

use crate::{
    audit as audit_evidence, CaseRequest, EvidenceAuditReport, EvidenceState, NeurosurgeryError,
    ObservationKind, PublicLiteratureBundle, PublicLiteratureQuery, RealDataQuery,
    RealDataRecordKind, RealGliomaBundle, Specialty,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const RESEARCH_PLAN_SCHEMA_VERSION: &str = "bioprism-neurosurgery-research-plan/0.1";
pub const MAX_RESEARCH_PLAN_TASKS: usize = 64;
pub const MAX_RESEARCH_PLAN_REFERENCES: usize = 16;

/// The role of a source attached to a plan. These labels prevent citation metadata from being
/// mistaken for a caller's observation or a validated clinical finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchPlanSource {
    PublicLiterature,
    RealGliomaPopulation,
}

/// A bounded local query that a human or caller-owned worker may inspect or replay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchPlanQuery {
    pub source: ResearchPlanSource,
    pub specialty: Specialty,
    /// Optional lexical narrowing. `None` is an explicit bounded scan over the selected
    /// specialty/record-kind facet, not an unbounded network search.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_kind: Option<RealDataRecordKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mesh_term: Option<String>,
    pub limit: usize,
}

/// A source-linked candidate reference. The URI points back to the supplied public snapshot's
/// authority; this object carries no patient-level value and no claim of applicability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchPlanReference {
    pub source: ResearchPlanSource,
    pub source_id: String,
    pub record_id: String,
    pub title: String,
    pub uri: String,
}

/// What a caller-owned reviewer should do next. These are research tasks, never clinical acts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchPlanTaskKind {
    AcquireCallerObservation,
    RepairProvenance,
    ResolveInterpretation,
    ReviewEvidenceCorpus,
    ReviewPopulationContext,
}

/// One ordered task in the bounded research handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchPlanTask {
    pub sequence: u16,
    pub task_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_kind: Option<ObservationKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence_state: Option<EvidenceState>,
    pub kind: ResearchPlanTaskKind,
    pub objective: String,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_query: Option<ResearchPlanQuery>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_match_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_returned_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_truncated: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_references: Vec<ResearchPlanReference>,
    pub reviewer_roles: Vec<String>,
}

/// Digest-bound research handoff produced from one intake audit and an optional local snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchPlanReport {
    pub schema_version: String,
    /// Digest over the complete plan with this field cleared. This is an integrity receipt, not
    /// a quality score or an authorization to perform a clinical action.
    #[serde(default)]
    pub plan_digest: String,
    pub request_digest: String,
    pub specialty: Specialty,
    pub max_tasks: usize,
    pub max_references_per_task: usize,
    pub audit: EvidenceAuditReport,
    pub tasks: Vec<ResearchPlanTask>,
    pub candidate_task_count: usize,
    pub omitted_task_count: usize,
    pub truncated: bool,
    pub source_query_count: usize,
    pub source_candidate_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub real_data_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_literature_digest: Option<String>,
    pub coverage_complete: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl ResearchPlanReport {
    /// Validate a persisted research-plan envelope without reopening source bundles.
    ///
    /// The check proves task sequencing, source-query/reference consistency, count projections,
    /// and provider boundaries. `validate_for_inputs` performs exact replay when the original
    /// request and local snapshot are available.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != RESEARCH_PLAN_SCHEMA_VERSION
            || !is_sha256_hex(&self.plan_digest)
            || !is_sha256_hex(&self.request_digest)
            || self.specialty != self.audit.specialty
            || self.request_digest != self.audit.request_digest
            || self.tasks.len() > MAX_RESEARCH_PLAN_TASKS
            || !(1..=MAX_RESEARCH_PLAN_TASKS).contains(&self.max_tasks)
            || !(1..=MAX_RESEARCH_PLAN_REFERENCES).contains(&self.max_references_per_task)
            || self.tasks.len() != self.candidate_task_count.min(self.max_tasks)
            || self.tasks.len() + self.omitted_task_count != self.candidate_task_count
            || self.truncated != (self.omitted_task_count > 0)
            || self.coverage_complete != self.audit.coverage_complete
            || self.source_query_count
                != self
                    .tasks
                    .iter()
                    .filter(|task| task.source_query.is_some())
                    .count()
            || self.source_candidate_count
                != self
                    .tasks
                    .iter()
                    .filter_map(|task| task.source_match_count)
                    .sum::<usize>()
            || self.real_data_digest.is_some() && self.public_literature_digest.is_some()
            || self
                .real_data_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
            || self
                .public_literature_digest
                .as_deref()
                .is_some_and(|digest| !is_sha256_hex(digest))
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
        {
            return Err(invalid_plan("research plan envelope is invalid"));
        }
        validate_audit_shape(&self.audit)?;
        let expected_roles = self.specialty.profile().human_review_roles;
        let mut seen_task_ids = BTreeSet::new();
        for (index, task) in self.tasks.iter().enumerate() {
            let expected_sequence = index + 1;
            if task.sequence as usize != expected_sequence
                || task.task_id != format!("research-task-{expected_sequence:03}")
                || !seen_task_ids.insert(task.task_id.as_str())
                || task.objective.trim().is_empty()
                || task.rationale.trim().is_empty()
                || task.reviewer_roles != expected_roles
                || task.observation_kind.is_some() != task.evidence_state.is_some()
            {
                return Err(invalid_plan("research plan task projection is invalid"));
            }
            validate_task_source(task, self.specialty)?;
        }
        if self.plan_digest != digest_report(self)? {
            return Err(invalid_plan(
                "research plan digest does not match its report contents",
            ));
        }
        Ok(())
    }

    /// Rebuild this plan from the exact request and one caller-supplied snapshot. This prevents a
    /// validly shaped worklist from being rebound to a different case or source corpus.
    pub fn validate_for_inputs(
        &self,
        request: &CaseRequest,
        real_data: Option<&RealGliomaBundle>,
        public_literature: Option<&PublicLiteratureBundle>,
        max_tasks: usize,
        max_references_per_task: usize,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        if max_tasks != self.max_tasks || max_references_per_task != self.max_references_per_task {
            return Err(invalid_plan(
                "research plan replay bounds do not match the persisted report",
            ));
        }
        let expected = compile(
            request,
            real_data,
            public_literature,
            max_tasks,
            max_references_per_task,
        )?;
        if self != &expected {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "research plan is not bound to the supplied request or snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }
}

fn invalid_plan(reason: &str) -> NeurosurgeryError {
    NeurosurgeryError::RealDataRejected {
        reason: reason.to_string(),
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}

fn validate_audit_shape(audit: &EvidenceAuditReport) -> Result<(), NeurosurgeryError> {
    if audit.schema_version != crate::EVIDENCE_AUDIT_SCHEMA_VERSION
        || audit.required_observation_kinds.len() != audit.items.len()
        || audit
            .items
            .iter()
            .zip(audit.required_observation_kinds.iter())
            .any(|(item, kind)| {
                item.observation_kind != *kind
                    || !item.required_for_review
                    || item.provenance_complete_count > item.observed_count
                    || item.reviewer_note.trim().is_empty()
                    || item.state != expected_evidence_state(item)
            })
        || {
            let mut seen = BTreeSet::new();
            audit
                .missing_required_kinds
                .iter()
                .any(|kind| !audit.required_observation_kinds.contains(kind) || !seen.insert(kind))
        }
        || audit.provenance_gap_count
            != audit
                .items
                .iter()
                .map(|item| {
                    item.observed_count
                        .saturating_sub(item.provenance_complete_count)
                })
                .sum::<usize>()
        || audit.verified_evidence_count + audit.unverified_evidence_count
            > audit.evidence_record_count
        || audit.evidence_supporting_synthesis_count > audit.evidence_record_count
        || audit.coverage_complete
            != (audit.missing_required_kinds.is_empty() && audit.provenance_gap_count == 0)
        || !audit.human_review_required
        || audit.provider != "none"
        || audit.network
        || audit.effect != "read_only"
    {
        return Err(invalid_plan("research plan intake audit is inconsistent"));
    }
    Ok(())
}

fn expected_evidence_state(item: &crate::EvidenceAuditItem) -> EvidenceState {
    if item.conflicting_count > 0 {
        EvidenceState::Conflicting
    } else if item.uninterpretable_count > 0 {
        EvidenceState::Uninterpretable
    } else if item.observed_count > 0 {
        EvidenceState::Measured
    } else {
        EvidenceState::Unmeasured
    }
}

fn validate_task_source(
    task: &ResearchPlanTask,
    specialty: Specialty,
) -> Result<(), NeurosurgeryError> {
    let Some(query) = task.source_query.as_ref() else {
        if task.source_match_count.is_some()
            || task.source_returned_count.is_some()
            || task.source_truncated.is_some()
            || !task.source_references.is_empty()
        {
            return Err(invalid_plan(
                "research plan task has source metadata without a query",
            ));
        }
        return Ok(());
    };
    if query.specialty != specialty
        || !(1..=MAX_RESEARCH_PLAN_REFERENCES).contains(&query.limit)
        || task.source_match_count.is_none()
        || task.source_returned_count.is_none()
        || task.source_truncated.is_none()
        || task.source_returned_count > task.source_match_count
        || task.source_returned_count != Some(task.source_references.len())
        || task.source_references.len() > query.limit
    {
        return Err(invalid_plan(
            "research plan source query projection is invalid",
        ));
    }
    let mut references = BTreeSet::new();
    for reference in &task.source_references {
        if reference.source != query.source
            || reference.source_id.trim().is_empty()
            || reference.record_id.trim().is_empty()
            || reference.title.trim().is_empty()
            || reference.uri.trim().is_empty()
            || !references.insert((
                reference.source,
                reference.source_id.as_str(),
                reference.record_id.as_str(),
            ))
        {
            return Err(invalid_plan("research plan source reference is invalid"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct TaskSeed {
    kind: ResearchPlanTaskKind,
    observation_kind: Option<ObservationKind>,
    evidence_state: Option<EvidenceState>,
    objective: String,
    rationale: String,
}

#[derive(Debug, Clone)]
struct SourceProjection {
    query: ResearchPlanQuery,
    total_matches: usize,
    returned_matches: usize,
    truncated: bool,
    references: Vec<ResearchPlanReference>,
}

/// Compile a bounded plan after the agent has validated the request and selected evidence bundle.
/// The function is crate-visible so the public entry point remains the single validation gate.
pub(crate) fn compile(
    request: &CaseRequest,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    max_tasks: usize,
    max_references_per_task: usize,
) -> Result<ResearchPlanReport, NeurosurgeryError> {
    if real_data.is_some() && public_literature.is_some() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "research planning accepts one evidence bundle: choose real glioma data or public literature".to_string(),
        });
    }
    if !(1..=MAX_RESEARCH_PLAN_TASKS).contains(&max_tasks) {
        return Err(NeurosurgeryError::TooMany {
            field: "research_plan.max_tasks",
            found: max_tasks,
            max: MAX_RESEARCH_PLAN_TASKS,
        });
    }
    if !(1..=MAX_RESEARCH_PLAN_REFERENCES).contains(&max_references_per_task) {
        return Err(NeurosurgeryError::TooMany {
            field: "research_plan.max_references_per_task",
            found: max_references_per_task,
            max: MAX_RESEARCH_PLAN_REFERENCES,
        });
    }
    let audit = audit_evidence(request)?;
    let mut seeds = Vec::new();
    for item in &audit.items {
        let (kind, objective, rationale) = match item.state {
            EvidenceState::Unmeasured => (
                ResearchPlanTaskKind::AcquireCallerObservation,
                format!(
                    "Obtain a de-identified, caller-owned {} observation for research review",
                    observation_kind_label(item.observation_kind)
                ),
                "The required intake class has no measured observation; absence is not a negative finding".to_string(),
            ),
            EvidenceState::Uninterpretable | EvidenceState::Conflicting => (
                ResearchPlanTaskKind::ResolveInterpretation,
                format!(
                    "Have the qualified reviewer resolve the {} observation state",
                    observation_kind_label(item.observation_kind)
                ),
                format!(
                    "The intake audit reports {} evidence; no interpretation is supplied by this planner",
                    evidence_state_label(item.state)
                ),
            ),
            EvidenceState::Measured if item.provenance_complete_count < item.observed_count => (
                ResearchPlanTaskKind::RepairProvenance,
                format!(
                    "Attach a source identifier to every observed {} record",
                    observation_kind_label(item.observation_kind)
                ),
                "Observed content is present, but its caller-supplied provenance is incomplete".to_string(),
            ),
            EvidenceState::Measured => continue,
        };
        seeds.push(TaskSeed {
            kind,
            observation_kind: Some(item.observation_kind),
            evidence_state: Some(item.state),
            objective,
            rationale,
        });
    }
    if audit.evidence_record_count == 0 || audit.evidence_supporting_synthesis_count == 0 {
        seeds.push(TaskSeed {
            kind: ResearchPlanTaskKind::ReviewEvidenceCorpus,
            observation_kind: None,
            evidence_state: None,
            objective: "Assemble and verify provenance-bearing evidence before synthesis".to_string(),
            rationale: if audit.evidence_record_count == 0 {
                "No evidence records were supplied; the planner will not infer a source or a conclusion".to_string()
            } else {
                "Supplied evidence does not declare support for evidence synthesis; applicability remains unverified".to_string()
            },
        });
    }
    if real_data.is_some() || public_literature.is_some() {
        seeds.push(TaskSeed {
            kind: ResearchPlanTaskKind::ReviewPopulationContext,
            observation_kind: None,
            evidence_state: None,
            objective: "Review source-linked population/citation context without promoting it to case evidence".to_string(),
            rationale: "The supplied snapshot is a bounded research context only; a human must verify quality, applicability, and cohort identity".to_string(),
        });
    }

    let candidate_task_count = seeds.len();
    let omitted_task_count = candidate_task_count.saturating_sub(max_tasks);
    let truncated = omitted_task_count > 0;
    let reviewer_roles = request.specialty.profile().human_review_roles;
    let mut tasks = Vec::new();
    let mut source_query_count = 0usize;
    let mut source_candidate_count = 0usize;
    for (index, seed) in seeds.into_iter().take(max_tasks).enumerate() {
        let sequence = u16::try_from(index + 1).map_err(|_| NeurosurgeryError::TooMany {
            field: "research_plan.tasks",
            found: index + 1,
            max: u16::MAX as usize,
        })?;
        let source_projection = project_source(
            request.specialty,
            seed.observation_kind,
            real_data,
            public_literature,
            max_references_per_task,
        )?;
        if source_projection.is_some() {
            source_query_count = source_query_count.saturating_add(1);
        }
        if let Some(projection) = &source_projection {
            source_candidate_count =
                source_candidate_count.saturating_add(projection.total_matches);
        }
        let (source_query, source_match_count, source_returned_count, source_truncated, references) =
            source_projection.map_or((None, None, None, None, Vec::new()), |projection| {
                (
                    Some(projection.query),
                    Some(projection.total_matches),
                    Some(projection.returned_matches),
                    Some(projection.truncated),
                    projection.references,
                )
            });
        tasks.push(ResearchPlanTask {
            sequence,
            task_id: format!("research-task-{sequence:03}"),
            observation_kind: seed.observation_kind,
            evidence_state: seed.evidence_state,
            kind: seed.kind,
            objective: seed.objective,
            rationale: seed.rationale,
            source_query,
            source_match_count,
            source_returned_count,
            source_truncated,
            source_references: references,
            reviewer_roles: reviewer_roles.clone(),
        });
    }

    let request_digest = digest_request(request)?;
    let real_data_digest = real_data
        .map(|data| data.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let public_literature_digest = public_literature
        .map(|literature| literature.summary().map(|summary| summary.bundle_digest))
        .transpose()?;
    let mut report = ResearchPlanReport {
        schema_version: RESEARCH_PLAN_SCHEMA_VERSION.to_string(),
        plan_digest: String::new(),
        request_digest,
        specialty: request.specialty,
        max_tasks,
        max_references_per_task,
        coverage_complete: audit.coverage_complete,
        audit,
        tasks,
        candidate_task_count,
        omitted_task_count,
        truncated,
        source_query_count,
        source_candidate_count,
        real_data_digest,
        public_literature_digest,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "tasks are caller-owned research/review proposals and never clinical instructions".to_string(),
            "public literature and real-data references are source-linked metadata, not patient-level observations".to_string(),
            "a zero source_match_count means the bounded local query found no matching snapshot record; it does not prove that no evidence exists elsewhere".to_string(),
            "when a keyword scan is empty, the planner may omit text and retry the same bounded specialty/record-kind facet".to_string(),
            "the planner never fetches URLs, invokes a model, opens a credential, or writes durable state".to_string(),
        ],
    };
    report.plan_digest = digest_report(&report)?;
    report.validate_integrity()?;
    Ok(report)
}

fn project_source(
    specialty: Specialty,
    observation_kind: Option<ObservationKind>,
    real_data: Option<&RealGliomaBundle>,
    public_literature: Option<&PublicLiteratureBundle>,
    limit: usize,
) -> Result<Option<SourceProjection>, NeurosurgeryError> {
    let text = observation_kind.map(|kind| observation_kind_label(kind).to_string());
    if let Some(literature) = public_literature {
        let mut query = PublicLiteratureQuery {
            specialty: Some(specialty),
            text: text.clone(),
            publication_type: None,
            mesh_term: None,
            from_date: None,
            to_date: None,
            limit,
        };
        let mut result = literature.query(&query)?;
        if result.total_matches == 0 && query.text.is_some() {
            query.text = None;
            result = literature.query(&query)?;
        }
        return Ok(Some(SourceProjection {
            query: ResearchPlanQuery {
                source: ResearchPlanSource::PublicLiterature,
                specialty,
                text: query.text,
                record_kind: None,
                publication_type: None,
                mesh_term: None,
                limit,
            },
            total_matches: result.total_matches,
            returned_matches: result.returned_matches,
            truncated: result.truncated,
            references: result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source: ResearchPlanSource::PublicLiterature,
                    source_id: hit.source_id,
                    record_id: hit.pmid,
                    title: hit.title,
                    uri: hit.record_uri,
                })
                .collect(),
        }));
    }
    if let Some(data) = real_data {
        let record_kind = match observation_kind {
            Some(ObservationKind::Molecular) => Some(RealDataRecordKind::PortalMolecularProfile),
            Some(_) | None => Some(RealDataRecordKind::LiteratureArticle),
        };
        let mut query = RealDataQuery {
            text: text.clone(),
            status: None,
            trial_phase: None,
            trial_study_type: None,
            trial_updated_from: None,
            trial_updated_to: None,
            molecular_alteration_type: None,
            molecular_datatype: None,
            genomic_data_type: None,
            publication_type: None,
            mesh_term: None,
            publication_date_from: None,
            publication_date_to: None,
            record_kind,
            source_id: None,
            related_record_id: None,
            limit,
        };
        let mut result = data.query(&query)?;
        if result.total_matches == 0 && query.text.is_some() {
            query.text = None;
            result = data.query(&query)?;
        }
        return Ok(Some(SourceProjection {
            query: ResearchPlanQuery {
                source: ResearchPlanSource::RealGliomaPopulation,
                specialty,
                text: query.text,
                record_kind,
                publication_type: None,
                mesh_term: None,
                limit,
            },
            total_matches: result.total_matches,
            returned_matches: result.returned_matches,
            truncated: result.truncated,
            references: result
                .hits
                .into_iter()
                .map(|hit| ResearchPlanReference {
                    source: ResearchPlanSource::RealGliomaPopulation,
                    source_id: hit.source_id,
                    record_id: hit.record_id,
                    title: hit.title,
                    uri: hit.source_uri,
                })
                .collect(),
        }));
    }
    Ok(None)
}

fn digest_request(request: &CaseRequest) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(request)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn digest_report(report: &ResearchPlanReport) -> Result<String, NeurosurgeryError> {
    let mut unsigned = report.clone();
    unsigned.plan_digest.clear();
    let bytes = serde_json::to_vec(&unsigned)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn observation_kind_label(kind: ObservationKind) -> &'static str {
    match kind {
        ObservationKind::Imaging => "imaging",
        ObservationKind::Histology => "histology",
        ObservationKind::Molecular => "molecular",
        ObservationKind::Neuroanatomy => "neuroanatomy",
        ObservationKind::NeurologicFunction => "neurologic function",
        ObservationKind::DevelopmentalTrajectory => "developmental trajectory",
        ObservationKind::SpinalDysraphism => "spinal dysraphism",
        ObservationKind::CraniocervicalJunction => "craniocervical junction",
        ObservationKind::SurgicalHistory => "surgical history",
        ObservationKind::LongitudinalOutcome => "longitudinal outcome",
    }
}

fn evidence_state_label(state: EvidenceState) -> &'static str {
    match state {
        EvidenceState::Measured => "measured",
        EvidenceState::Unmeasured => "unmeasured",
        EvidenceState::Uninterpretable => "uninterpretable",
        EvidenceState::Conflicting => "conflicting",
    }
}
