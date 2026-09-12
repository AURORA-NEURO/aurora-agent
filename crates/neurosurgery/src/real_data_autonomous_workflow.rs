//! Deterministic autonomous review waves over a validated public glioma snapshot.
//!
//! The workflow is deliberately an orchestration layer, not a second inference engine. It
//! composes the existing source-bound evidence packet and turns explicit metadata obligations
//! into ordered, resumable reviewer work. No record is ranked for clinical value, no abstract is
//! interpreted, and no source is fetched. A caller can persist the returned wave and feed a
//! human-owned disposition report into the next run without server-side state or an API key.

use crate::{
    NeurosurgeryError, RealDataEvidencePacketQuery, RealDataEvidencePacketReport,
    RealDataRecordKind, RealDataReviewDisposition, RealDataReviewDispositionReport,
    RealDataReviewItem, RealDataReviewKind, RealGliomaBundle, RealSourceKind,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const REAL_DATA_AUTONOMOUS_WORKFLOW_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-autonomous-workflow/0.1";
pub const MAX_REAL_DATA_AUTONOMOUS_ACTIONS: usize = 256;

fn default_max_actions() -> usize {
    64
}

/// Caller-owned controls for one deterministic real-data review wave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataAutonomousWorkflowQuery {
    #[serde(default)]
    pub packet: RealDataEvidencePacketQuery,
    #[serde(default)]
    pub dispositions: Option<RealDataReviewDispositionReport>,
    #[serde(default = "default_max_actions")]
    pub max_actions: usize,
}

impl Default for RealDataAutonomousWorkflowQuery {
    fn default() -> Self {
        Self {
            packet: RealDataEvidencePacketQuery::default(),
            dispositions: None,
            max_actions: default_max_actions(),
        }
    }
}

/// Structural phase of the next review wave. These are workflow dependencies, never clinical
/// urgency or evidence-quality grades.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataAutonomousWorkflowStage {
    Provenance,
    Completeness,
    Context,
    HumanSignoff,
}

/// Action emitted from one explicit source-backed metadata obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataAutonomousActionKind {
    ExpandReviewQueue,
    ExpandEvidenceProjection,
    ReconcileIdentifiers,
    ResolvePublicationCrosswalk,
    VerifyLiteratureContext,
    VerifySourceMetadata,
    RefreshSourceSnapshot,
    InspectMolecularInventory,
    InspectCohortLandscape,
    HumanSynthesisGate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataAutonomousActionStatus {
    Pending,
    Unresolved,
}

/// One bounded next action. Stable identifiers and source metadata let a caller replay or hand
/// off the item without copying abstracts, sample values, or patient data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataAutonomousAction {
    pub action_id: String,
    pub stage: RealDataAutonomousWorkflowStage,
    pub kind: RealDataAutonomousActionKind,
    pub status: RealDataAutonomousActionStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_kind: Option<RealSourceKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_kind: Option<RealDataRecordKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataAutonomousWorkflowState {
    NeedsSnapshotExpansion,
    NeedsMetadataReview,
    ReadyForHumanSynthesis,
}

/// Digest-bound, resumable review wave. Every nested packet remains independently verifiable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataAutonomousWorkflowReport {
    pub schema_version: String,
    pub workflow_digest: String,
    pub bundle_digest: String,
    pub packet_digest: String,
    pub generated_at: String,
    pub query: RealDataAutonomousWorkflowQuery,
    pub packet: RealDataEvidencePacketReport,
    pub state: RealDataAutonomousWorkflowState,
    pub candidate_action_count: usize,
    pub returned_action_count: usize,
    pub omitted_action_count: usize,
    pub truncated: bool,
    pub resolved_queue_item_count: usize,
    pub open_queue_item_count: usize,
    pub actions: Vec<RealDataAutonomousAction>,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataAutonomousWorkflowReport {
    /// Validate a persisted review wave without fetching sources or opening asset bytes.
    ///
    /// This checks the action queue, dependency closure, state/counter projection, packet
    /// binding, and workflow digest. It does not decide whether any public record is clinically
    /// relevant or whether a reviewer should accept a conclusion.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_AUTONOMOUS_WORKFLOW_SCHEMA_VERSION
            || !is_sha256_hex(&self.workflow_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || !is_sha256_hex(&self.packet_digest)
            || self.bundle_digest != self.packet.bundle_digest
            || self.packet_digest != self.packet.packet_digest
            || self.generated_at != self.packet.generated_at
            || self.candidate_action_count < self.returned_action_count
            || self.omitted_action_count
                != self
                    .candidate_action_count
                    .saturating_sub(self.returned_action_count)
            || self.actions.len() != self.returned_action_count
            || self.returned_action_count > self.query.max_actions
            || self.truncated != (self.omitted_action_count > 0)
            || self.source_count() == 0
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(workflow_rejected("autonomous workflow envelope is invalid"));
        }
        validate_query(&self.query)?;
        if self.packet.schema_version != crate::REAL_DATA_EVIDENCE_PACKET_SCHEMA_VERSION
            || !self.packet.provenance_bound
            || self.packet.synthetic_data
            || !self.packet.human_review_required
            || self.packet.provider != "none"
            || self.packet.network
            || self.packet.effect != "read_only"
            || self.packet.validate_integrity().is_err()
        {
            return Err(workflow_rejected(
                "autonomous workflow packet binding is invalid",
            ));
        }
        if let Some(dispositions) = self.query.dispositions.as_ref() {
            dispositions.validate_integrity(&self.packet.review_queue)?;
        }
        let mut action_ids = BTreeSet::new();
        let returned_ids = self
            .actions
            .iter()
            .map(|action| action.action_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut previous_key = None;
        for action in &self.actions {
            if action.action_id.trim().is_empty()
                || !action_ids.insert(action.action_id.clone())
                || action.rationale.trim().is_empty()
                || action
                    .source_id
                    .as_deref()
                    .is_some_and(|value| value.trim().is_empty())
                || action
                    .source_uri
                    .as_deref()
                    .is_some_and(|uri| !uri.starts_with("https://"))
                || action.source_kind.is_some() != action.source_id.is_some()
                || action.source_uri.is_some() != action.source_id.is_some()
                || (action.record_id.is_some() && action.record_kind.is_none())
                || action
                    .title
                    .as_deref()
                    .is_some_and(|title| title.trim().is_empty())
                || action.depends_on.iter().any(|dependency| {
                    dependency.trim().is_empty()
                        || dependency == &action.action_id
                        || !returned_ids.contains(dependency.as_str())
                })
                || {
                    let mut dependencies = BTreeSet::new();
                    action
                        .depends_on
                        .iter()
                        .any(|dependency| !dependencies.insert(dependency))
                }
                || previous_key
                    .as_ref()
                    .is_some_and(|previous| action_sort_key(previous) >= action_sort_key(action))
            {
                return Err(workflow_rejected(
                    "autonomous workflow action queue is invalid",
                ));
            }
            previous_key = Some(action.clone());
        }
        let freshness_review_required = self.packet.freshness.as_ref().is_some_and(|report| {
            report
                .sources
                .iter()
                .any(|source| source.state != crate::RealDataFreshnessState::Current)
        });
        let projection_expansion_required = packet_projection_truncated(&self.packet);
        let expected_open = self
            .packet
            .review_queue
            .returned_item_count
            .saturating_sub(self.resolved_queue_item_count)
            .saturating_add(self.packet.review_queue.omitted_item_count);
        if self.resolved_queue_item_count > self.packet.review_queue.returned_item_count
            || self.open_queue_item_count != expected_open
            || self.state
                != if self.packet.review_queue.omitted_item_count > 0 {
                    RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
                } else if projection_expansion_required || self.omitted_action_count > 0 {
                    RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
                } else if expected_open > 0 || freshness_review_required {
                    RealDataAutonomousWorkflowState::NeedsMetadataReview
                } else {
                    RealDataAutonomousWorkflowState::ReadyForHumanSynthesis
                }
            || self.workflow_digest
                != digest_workflow(
                    &self.packet,
                    &self.query,
                    self.state,
                    self.candidate_action_count,
                    self.omitted_action_count,
                    self.resolved_queue_item_count,
                    self.open_queue_item_count,
                    &self.actions,
                )?
        {
            return Err(workflow_rejected(
                "autonomous workflow state or digest is invalid",
            ));
        }
        Ok(())
    }

    /// Replay the workflow against the exact validated snapshot and persisted disposition/query.
    pub fn validate_for_inputs(&self, bundle: &RealGliomaBundle) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.autonomous_workflow(&self.query)?;
        if &expected != self {
            return Err(workflow_rejected(
                "autonomous workflow does not replay to the exact supplied snapshot",
            ));
        }
        Ok(())
    }

    fn source_count(&self) -> usize {
        self.packet.source_count
    }
}

impl RealGliomaBundle {
    /// Compose one source-bound autonomous review wave and optionally resume it from a persisted
    /// human disposition report. “Autonomous” here means deterministic orchestration only: every
    /// action remains a reviewer-owned metadata task and the final gate is never auto-approved.
    pub fn autonomous_workflow(
        &self,
        query: &RealDataAutonomousWorkflowQuery,
    ) -> Result<RealDataAutonomousWorkflowReport, NeurosurgeryError> {
        validate_query(query)?;
        let packet = self.evidence_packet(&query.packet)?;
        if let Some(dispositions) = &query.dispositions {
            dispositions.validate_integrity(&packet.review_queue)?;
        }
        let decisions = query
            .dispositions
            .as_ref()
            .map(|report| {
                report
                    .decisions
                    .iter()
                    .map(|decision| (decision.task_id.as_str(), decision.disposition))
                    .collect::<BTreeMap<_, _>>()
            })
            .unwrap_or_default();
        let mut actions = Vec::new();
        // The packet is the single handoff boundary. Reuse its digest-bound reconciliation
        // projection instead of recomputing a parallel view that could drift from the context
        // delivered to a local model or reviewer.
        let reconciliation = &packet.reconciliation;
        let reconciliation_review_required = reconciliation.candidate_issue_count > 0;
        if reconciliation_review_required {
            actions.push(RealDataAutonomousAction {
                action_id: "real-autonomous-reconcile-identifiers".to_string(),
                stage: RealDataAutonomousWorkflowStage::Provenance,
                kind: RealDataAutonomousActionKind::ReconcileIdentifiers,
                status: RealDataAutonomousActionStatus::Pending,
                source_id: None,
                source_uri: None,
                source_kind: None,
                record_kind: None,
                record_id: None,
                title: Some("Cross-source PMID/DOI reconciliation".to_string()),
                depends_on: Vec::new(),
                rationale: format!(
                    "The validated snapshot contains {} exact PMID/normalized-DOI identifier review finding(s) (reconciliation digest {}). Inspect the dedicated reconciliation report before relying on cross-source links; no identifier is merged or repaired automatically.",
                    reconciliation.candidate_issue_count,
                    reconciliation.reconciliation_digest,
                ),
            });
        }
        if packet.review_queue.omitted_item_count > 0 {
            actions.push(RealDataAutonomousAction {
                action_id: "real-autonomous-expand-review-queue".to_string(),
                stage: RealDataAutonomousWorkflowStage::Provenance,
                kind: RealDataAutonomousActionKind::ExpandReviewQueue,
                status: RealDataAutonomousActionStatus::Pending,
                source_id: None,
                source_uri: None,
                source_kind: None,
                record_kind: None,
                record_id: None,
                title: None,
                depends_on: Vec::new(),
                rationale: "The review queue is truncated; rerun with a larger max_items bound before treating the wave as complete.".to_string(),
            });
        }
        for item in &packet.review_queue.items {
            let disposition = decisions.get(item.task_id.as_str()).copied();
            if matches!(
                disposition,
                Some(RealDataReviewDisposition::Reviewed)
                    | Some(RealDataReviewDisposition::NotApplicable)
            ) {
                continue;
            }
            actions.push(action_for_item(item, disposition));
        }
        let freshness_review_required = packet.freshness.as_ref().is_some_and(|report| {
            report
                .sources
                .iter()
                .any(|source| source.state != crate::RealDataFreshnessState::Current)
        });
        let projection_expansion_required = packet_projection_truncated(&packet);
        if projection_expansion_required {
            let mut truncated = Vec::new();
            if packet.data_query.truncated {
                truncated.push(format!(
                    "record_query omitted {} hit(s)",
                    packet
                        .data_query
                        .total_matches
                        .saturating_sub(packet.data_query.returned_matches)
                ));
            }
            if packet.graph.truncated {
                truncated.push(format!(
                    "evidence_graph omitted {} node/edge row(s)",
                    packet
                        .graph
                        .omitted_node_count
                        .saturating_add(packet.graph.omitted_edge_count)
                ));
            }
            if packet.trial_landscape.truncated {
                truncated.push(format!(
                    "trial_landscape omitted {} trial row(s)",
                    packet.trial_landscape.omitted_trial_count
                ));
            }
            if packet.trial_landscape.intervention_truncated {
                truncated.push(format!(
                    "trial_landscape omitted {} intervention bucket(s)",
                    packet.trial_landscape.omitted_intervention_count
                ));
            }
            if packet.molecular_coverage.truncated {
                truncated.push(format!(
                    "molecular_coverage omitted {} profile row(s)",
                    packet.molecular_coverage.omitted_profile_count
                ));
            }
            if packet.molecular_coverage.study_rows_truncated {
                truncated.push(format!(
                    "molecular_coverage omitted {} study row(s)",
                    packet.molecular_coverage.omitted_study_count
                ));
            }
            if let Some(cohort) = packet.cohort_landscape.as_ref() {
                if cohort.truncated {
                    truncated.push(format!(
                        "cohort_landscape omitted {} project row(s)",
                        cohort.omitted_project_count
                    ));
                }
            }
            actions.push(RealDataAutonomousAction {
                action_id: "real-autonomous-expand-evidence-projection".to_string(),
                stage: RealDataAutonomousWorkflowStage::Completeness,
                kind: RealDataAutonomousActionKind::ExpandEvidenceProjection,
                status: RealDataAutonomousActionStatus::Pending,
                source_id: None,
                source_uri: None,
                source_kind: None,
                record_kind: None,
                record_id: None,
                title: Some("Expand bounded evidence projections".to_string()),
                depends_on: Vec::new(),
                rationale: format!(
                    "The packet is bounded and omits part of one or more projections ({}). Rerun with larger caller-owned limits before treating the review wave as complete; omitted rows remain unknown and the workflow never silently expands a snapshot.",
                    truncated.join("; ")
                ),
            });
        }
        if let Some(freshness) = &packet.freshness {
            for source in freshness
                .sources
                .iter()
                .filter(|source| source.state != crate::RealDataFreshnessState::Current)
            {
                let bundle_source = self
                    .sources
                    .iter()
                    .find(|candidate| candidate.source_id == source.source_id);
                let (kind, title, rationale) = match source.state {
                    crate::RealDataFreshnessState::Stale => (
                        RealDataAutonomousActionKind::RefreshSourceSnapshot,
                        format!("Source freshness: stale; refresh {}", source.source_id),
                        format!(
                            "The caller's freshness policy marks source {} as stale; run the allow-listed refresh adapter, validate the candidate snapshot, and obtain human review before promotion. The workflow itself never fetches or replaces source data.",
                            source.source_id
                        ),
                    ),
                    crate::RealDataFreshnessState::FutureDated => (
                        RealDataAutonomousActionKind::VerifySourceMetadata,
                        format!("Source freshness: future-dated"),
                        format!(
                            "The caller's freshness policy marks source {} as future-dated; inspect its retrieval timestamp and caller clock before relying on this snapshot.",
                            source.source_id
                        ),
                    ),
                    crate::RealDataFreshnessState::Current => continue,
                };
                actions.push(RealDataAutonomousAction {
                    action_id: format!("real-autonomous-verify-freshness-{}", source.source_id),
                    stage: RealDataAutonomousWorkflowStage::Completeness,
                    kind,
                    status: RealDataAutonomousActionStatus::Pending,
                    source_id: Some(source.source_id.clone()),
                    source_uri: bundle_source.map(|candidate| candidate.uri.clone()),
                    source_kind: bundle_source.map(|candidate| candidate.kind),
                    record_kind: None,
                    record_id: None,
                    title: Some(title),
                    depends_on: Vec::new(),
                    rationale,
                });
            }
        }
        if packet.summary.portal_molecular_profile_count > 0 {
            let portal_source = self
                .sources
                .iter()
                .find(|source| source.kind == RealSourceKind::StudyPortal);
            actions.push(RealDataAutonomousAction {
                action_id: "real-autonomous-inspect-molecular-inventory".to_string(),
                stage: RealDataAutonomousWorkflowStage::Context,
                kind: RealDataAutonomousActionKind::InspectMolecularInventory,
                status: RealDataAutonomousActionStatus::Pending,
                source_id: portal_source.map(|source| source.source_id.clone()),
                source_uri: portal_source.map(|source| source.uri.clone()),
                source_kind: Some(RealSourceKind::StudyPortal),
                record_kind: Some(RealDataRecordKind::PortalMolecularProfile),
                record_id: None,
                title: Some("Public molecular-assay inventory".to_string()),
                depends_on: Vec::new(),
                rationale: format!(
                    "Inspect the exact public assay modalities and datatypes against the research question; the packet molecular ledger reports {} returned profiles across {} studies (coverage digest {}), and modality presence is not a patient-level molecular finding.",
                    packet.molecular_coverage.returned_profile_count,
                    packet.molecular_coverage.emitted_study_count,
                    packet.molecular_coverage.coverage_digest,
                ),
            });
        }
        if let Some(cohort) = packet.cohort_landscape.as_ref() {
            if cohort.returned_project_count > 0 {
                actions.push(RealDataAutonomousAction {
                    action_id: "real-autonomous-inspect-cohort-landscape".to_string(),
                    stage: RealDataAutonomousWorkflowStage::Context,
                    kind: RealDataAutonomousActionKind::InspectCohortLandscape,
                    status: RealDataAutonomousActionStatus::Pending,
                    source_id: cohort
                        .project_rows
                        .first()
                        .map(|row| row.source_id.clone()),
                    source_uri: cohort
                        .project_rows
                        .first()
                        .map(|row| row.source_uri.clone()),
                    source_kind: Some(RealSourceKind::GenomicCommons),
                    record_kind: Some(RealDataRecordKind::GenomicProject),
                    record_id: None,
                    title: Some("Public genomic cohort landscape".to_string()),
                    depends_on: Vec::new(),
                    rationale: format!(
                        "Inspect the aggregate project/file availability before comparing public genomic cohorts: the packet returns {} of {} project row(s), {} released-case inventory, {} shared data type(s), and {} explicit landscape review reason(s) (digest {}). Counts are source metadata only and do not establish cohort overlap, assay equivalence, eligibility, or patient-level meaning.",
                        cohort.returned_project_count,
                        cohort.total_matching_projects,
                        cohort.total_released_case_inventory,
                        cohort.shared_data_type_count,
                        cohort.review_reasons.len(),
                        cohort.landscape_digest,
                    ),
                });
            }
        }
        let resolved_queue_item_count = query
            .dispositions
            .as_ref()
            .map(|report| report.resolved_decision_count)
            .unwrap_or(0);
        let open_queue_item_count = packet
            .review_queue
            .returned_item_count
            .saturating_sub(resolved_queue_item_count)
            + packet.review_queue.omitted_item_count;
        if open_queue_item_count == 0
            && !freshness_review_required
            && !reconciliation_review_required
            && !projection_expansion_required
        {
            actions.push(RealDataAutonomousAction {
                action_id: "real-autonomous-human-synthesis-gate".to_string(),
                stage: RealDataAutonomousWorkflowStage::HumanSignoff,
                kind: RealDataAutonomousActionKind::HumanSynthesisGate,
                status: RealDataAutonomousActionStatus::Pending,
                source_id: None,
                source_uri: None,
                source_kind: None,
                record_kind: None,
                record_id: None,
                title: Some("Human review gate".to_string()),
                depends_on: actions.iter().map(|action| action.action_id.clone()).collect(),
                rationale: "All emitted metadata obligations have caller-owned dispositions; a qualified reviewer must still inspect the packet before any research synthesis is accepted.".to_string(),
            });
        }
        let provenance_ids = actions
            .iter()
            .filter(|action| action.stage == RealDataAutonomousWorkflowStage::Provenance)
            .map(|action| action.action_id.clone())
            .collect::<Vec<_>>();
        for action in &mut actions {
            if action.stage == RealDataAutonomousWorkflowStage::Context {
                action.depends_on = provenance_ids.clone();
            }
        }
        actions.sort_by(|left, right| action_sort_key(left).cmp(&action_sort_key(right)));
        let candidate_action_count = actions.len();
        let omitted_action_count = candidate_action_count.saturating_sub(query.max_actions);
        actions.truncate(query.max_actions);
        let returned_action_ids = actions
            .iter()
            .map(|action| action.action_id.clone())
            .collect::<BTreeSet<_>>();
        for action in &mut actions {
            action
                .depends_on
                .retain(|dependency| returned_action_ids.contains(dependency.as_str()));
        }
        let returned_action_count = actions.len();
        let state = if packet.review_queue.omitted_item_count > 0
            || projection_expansion_required
            || omitted_action_count > 0
        {
            RealDataAutonomousWorkflowState::NeedsSnapshotExpansion
        } else if open_queue_item_count > 0
            || freshness_review_required
            || reconciliation_review_required
        {
            RealDataAutonomousWorkflowState::NeedsMetadataReview
        } else {
            RealDataAutonomousWorkflowState::ReadyForHumanSynthesis
        };
        let workflow_digest = digest_workflow(
            &packet,
            query,
            state,
            candidate_action_count,
            omitted_action_count,
            resolved_queue_item_count,
            open_queue_item_count,
            &actions,
        )?;
        let report = RealDataAutonomousWorkflowReport {
            schema_version: REAL_DATA_AUTONOMOUS_WORKFLOW_SCHEMA_VERSION.to_string(),
            workflow_digest,
            bundle_digest: packet.bundle_digest.clone(),
            packet_digest: packet.packet_digest.clone(),
            generated_at: packet.generated_at.clone(),
            query: query.clone(),
            packet,
            state,
            candidate_action_count,
            returned_action_count,
            omitted_action_count,
            truncated: omitted_action_count > 0,
            resolved_queue_item_count,
            open_queue_item_count,
            actions,
            provenance_bound: true,
            synthetic_data: false,
            human_review_required: true,
            provider: "none".to_string(),
            network: false,
            effect: "read_only".to_string(),
            limitations: vec![
                "the workflow is deterministic orchestration over public metadata; it does not diagnose, grade, prognose, triage, recommend treatment, or issue procedural instructions".to_string(),
                "action order expresses provenance/completeness/context dependencies only; it is not clinical urgency, evidence quality, or patient risk".to_string(),
                "reviewed or not_applicable dispositions close a workflow item but never repair the source snapshot or prove a claim".to_string(),
                "the workflow never fetches URLs, invokes a model, opens credentials, exposes patient/sample values, sends notifications, or writes durable state".to_string(),
            ],
        };
        report.validate_integrity()?;
        Ok(report)
    }
}

fn validate_query(query: &RealDataAutonomousWorkflowQuery) -> Result<(), NeurosurgeryError> {
    if query.max_actions == 0 || query.max_actions > MAX_REAL_DATA_AUTONOMOUS_ACTIONS {
        return Err(NeurosurgeryError::TooMany {
            field: "real_data_autonomous_workflow.max_actions",
            found: query.max_actions,
            max: MAX_REAL_DATA_AUTONOMOUS_ACTIONS,
        });
    }
    Ok(())
}

fn packet_projection_truncated(packet: &RealDataEvidencePacketReport) -> bool {
    packet.data_query.truncated
        || packet.graph.truncated
        || packet.trial_landscape.truncated
        || packet.trial_landscape.intervention_truncated
        || packet.molecular_coverage.truncated
        || packet.molecular_coverage.study_rows_truncated
        || packet
            .cohort_landscape
            .as_ref()
            .is_some_and(|report| report.truncated)
}

fn action_for_item(
    item: &RealDataReviewItem,
    disposition: Option<RealDataReviewDisposition>,
) -> RealDataAutonomousAction {
    let (stage, kind) = match item.kind {
        RealDataReviewKind::MissingPortalPublicationLink => (
            RealDataAutonomousWorkflowStage::Provenance,
            RealDataAutonomousActionKind::ResolvePublicationCrosswalk,
        ),
        RealDataReviewKind::UnlinkedLiteratureCitation => (
            RealDataAutonomousWorkflowStage::Context,
            RealDataAutonomousActionKind::ResolvePublicationCrosswalk,
        ),
        RealDataReviewKind::MissingLiteratureAbstract
        | RealDataReviewKind::TruncatedLiteratureAbstract => (
            RealDataAutonomousWorkflowStage::Completeness,
            RealDataAutonomousActionKind::VerifyLiteratureContext,
        ),
        RealDataReviewKind::MissingClinicalTrialUpdate
        | RealDataReviewKind::MissingPortalSampleCount => (
            RealDataAutonomousWorkflowStage::Completeness,
            RealDataAutonomousActionKind::VerifySourceMetadata,
        ),
    };
    let status = if disposition == Some(RealDataReviewDisposition::Unresolved) {
        RealDataAutonomousActionStatus::Unresolved
    } else {
        RealDataAutonomousActionStatus::Pending
    };
    RealDataAutonomousAction {
        action_id: format!("real-autonomous-review-{}", item.task_id),
        stage,
        kind,
        status,
        source_id: Some(item.source_id.clone()),
        source_uri: Some(item.source_uri.clone()),
        source_kind: Some(item.source_kind),
        record_kind: Some(item.record_kind),
        record_id: Some(item.record_id.clone()),
        title: Some(item.title.clone()),
        depends_on: Vec::new(),
        rationale: item.reason.clone(),
    }
}

fn action_sort_key(
    action: &RealDataAutonomousAction,
) -> (
    RealDataAutonomousWorkflowStage,
    RealDataAutonomousActionKind,
    &str,
    &str,
) {
    (
        action.stage,
        action.kind,
        action.source_id.as_deref().unwrap_or_default(),
        action
            .record_id
            .as_deref()
            .unwrap_or(action.action_id.as_str()),
    )
}

#[allow(clippy::too_many_arguments)]
fn digest_workflow(
    packet: &RealDataEvidencePacketReport,
    query: &RealDataAutonomousWorkflowQuery,
    state: RealDataAutonomousWorkflowState,
    candidate_action_count: usize,
    omitted_action_count: usize,
    resolved_queue_item_count: usize,
    open_queue_item_count: usize,
    actions: &[RealDataAutonomousAction],
) -> Result<String, NeurosurgeryError> {
    let payload = (
        &packet.packet_digest,
        query,
        state,
        candidate_action_count,
        omitted_action_count,
        resolved_queue_item_count,
        open_queue_item_count,
        actions,
    );
    let bytes = serde_json::to_vec(&payload)
        .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn workflow_rejected(reason: &str) -> NeurosurgeryError {
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
