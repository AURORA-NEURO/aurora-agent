use crate::error::{invalid_spec, CampaignError};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_CAMPAIGN_STAGES: usize = 64;
pub const MAX_STAGE_DEPENDENCIES: usize = 16;
pub const MAX_CAMPAIGN_ACTIONS: u16 = 64;
pub const MAX_CAMPAIGN_EVENTS: usize = MAX_CAMPAIGN_ACTIONS as usize * 3;
pub const MAX_CAMPAIGN_ID_BYTES: usize = 128;
pub const MAX_STAGE_ID_BYTES: usize = 128;
pub const MAX_OBJECTIVE_BYTES: usize = 4096;
pub const MAX_RECONCILIATION_AUTHORITY_ID_BYTES: usize = 128;
pub const MAX_RECONCILIATION_AUTHORITY_VERSION_BYTES: usize = 64;

/// Stable identity and configuration commitment for the execution journal allowed to reconcile
/// uncertain campaign actions. This is an authority selector, not a signature or credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignReconciliationAuthorityDocument {
    pub authority_id: String,
    pub protocol_version: String,
    pub config_digest: String,
}

/// The four existing kernels that a campaign may compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignActionKind {
    SyntheticResearch,
    BrainPlan,
    AutopilotDrive,
    NeurosurgeryResearch,
}

impl CampaignActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SyntheticResearch => "synthetic_research",
            Self::BrainPlan => "brain_plan",
            Self::AutopilotDrive => "autopilot_drive",
            Self::NeurosurgeryResearch => "neurosurgery_research",
        }
    }

    /// Whether this build contains the native verifier needed to settle this action.
    pub fn adapter_availability(self) -> CampaignAdapterAvailability {
        match self {
            Self::NeurosurgeryResearch if !cfg!(feature = "neurosurgery-adapter") => {
                CampaignAdapterAvailability::FeatureDisabled {
                    required_feature: "neurosurgery-adapter",
                }
            }
            _ => CampaignAdapterAvailability::Available,
        }
    }
}

/// Explicit build-time availability; an absent verifier is not reported as a failed measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignAdapterAvailability {
    Available,
    FeatureDisabled { required_feature: &'static str },
}

/// Serde-facing stage shape. Every listed stage is required.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CampaignStageDocument {
    pub stage_id: String,
    pub kind: CampaignActionKind,
    /// Digest of the private input that the native receipt verifier must rebind.
    pub input_digest: String,
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Serde-facing campaign shape. The objective remains caller-owned and is absent from checkpoints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchCampaignSpecDocument {
    pub campaign_id: String,
    pub objective: String,
    pub reconciliation_authority: CampaignReconciliationAuthorityDocument,
    pub stages: Vec<CampaignStageDocument>,
    pub max_actions: u16,
}

/// A validated stage. Private fields prevent a caller from bypassing DAG and digest validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CampaignStageSpec {
    stage_id: String,
    kind: CampaignActionKind,
    input_digest: String,
    depends_on: Vec<String>,
}

impl CampaignStageSpec {
    pub fn stage_id(&self) -> &str {
        &self.stage_id
    }

    pub fn kind(&self) -> CampaignActionKind {
        self.kind
    }

    pub fn input_digest(&self) -> &str {
        &self.input_digest
    }

    pub fn depends_on(&self) -> &[String] {
        &self.depends_on
    }
}

/// A bounded, acyclic campaign specification with a stable digest and topological order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchCampaignSpec {
    campaign_id: String,
    objective: String,
    stages: BTreeMap<String, CampaignStageSpec>,
    topological_order: Vec<String>,
    max_actions: u16,
    reconciliation_authority: CampaignReconciliationAuthorityDocument,
    spec_digest: String,
}

impl ResearchCampaignSpec {
    pub fn parse(value: &serde_json::Value) -> Result<Self, CampaignError> {
        let document: ResearchCampaignSpecDocument = serde_json::from_value(value.clone())
            .map_err(|error| {
                invalid_spec(format!("document does not match the schema: {error}"))
            })?;
        Self::try_from(document)
    }

    pub fn campaign_id(&self) -> &str {
        &self.campaign_id
    }

    pub fn objective(&self) -> &str {
        &self.objective
    }

    pub fn stage(&self, stage_id: &str) -> Option<&CampaignStageSpec> {
        self.stages.get(stage_id)
    }

    pub fn stages(&self) -> impl Iterator<Item = &CampaignStageSpec> {
        self.topological_order
            .iter()
            .filter_map(|stage_id| self.stages.get(stage_id))
    }

    pub fn max_actions(&self) -> u16 {
        self.max_actions
    }

    pub fn reconciliation_authority(&self) -> &CampaignReconciliationAuthorityDocument {
        &self.reconciliation_authority
    }

    pub fn spec_digest(&self) -> &str {
        &self.spec_digest
    }

    pub(crate) fn topological_order(&self) -> &[String] {
        &self.topological_order
    }
}

impl TryFrom<ResearchCampaignSpecDocument> for ResearchCampaignSpec {
    type Error = CampaignError;

    fn try_from(document: ResearchCampaignSpecDocument) -> Result<Self, Self::Error> {
        let ResearchCampaignSpecDocument {
            campaign_id,
            objective,
            reconciliation_authority,
            stages: stage_documents,
            max_actions,
        } = document;
        bounded_text(&campaign_id, "campaign_id", MAX_CAMPAIGN_ID_BYTES)?;
        bounded_text(&objective, "objective", MAX_OBJECTIVE_BYTES)?;
        validate_reconciliation_authority(&reconciliation_authority)?;
        if stage_documents.is_empty() || stage_documents.len() > MAX_CAMPAIGN_STAGES {
            return Err(invalid_spec(format!(
                "stages must contain 1..={MAX_CAMPAIGN_STAGES} entries"
            )));
        }
        if max_actions == 0 || max_actions > MAX_CAMPAIGN_ACTIONS {
            return Err(invalid_spec(format!(
                "max_actions must be within 1..={MAX_CAMPAIGN_ACTIONS}"
            )));
        }
        if usize::from(max_actions) < stage_documents.len() {
            return Err(invalid_spec(
                "max_actions cannot be smaller than the required stage count",
            ));
        }

        let mut stages = BTreeMap::new();
        for stage in stage_documents {
            bounded_text(&stage.stage_id, "stage_id", MAX_STAGE_ID_BYTES)?;
            ContentHash::parse(stage.input_digest.clone()).map_err(|_| {
                invalid_spec(format!(
                    "stage {:?} input_digest must be a lowercase SHA-256 digest",
                    stage.stage_id
                ))
            })?;
            if stage.depends_on.len() > MAX_STAGE_DEPENDENCIES {
                return Err(invalid_spec(format!(
                    "stage {:?} has more than {MAX_STAGE_DEPENDENCIES} dependencies",
                    stage.stage_id
                )));
            }
            let mut dependencies = BTreeSet::new();
            for dependency in &stage.depends_on {
                bounded_text(dependency, "dependency", MAX_STAGE_ID_BYTES)?;
                if dependency == &stage.stage_id {
                    return Err(invalid_spec(format!(
                        "stage {:?} depends on itself",
                        stage.stage_id
                    )));
                }
                if !dependencies.insert(dependency.clone()) {
                    return Err(invalid_spec(format!(
                        "stage {:?} repeats dependency {:?}",
                        stage.stage_id, dependency
                    )));
                }
            }
            let validated = CampaignStageSpec {
                stage_id: stage.stage_id.clone(),
                kind: stage.kind,
                input_digest: stage.input_digest,
                depends_on: dependencies.into_iter().collect(),
            };
            if stages.insert(stage.stage_id.clone(), validated).is_some() {
                return Err(invalid_spec(format!(
                    "stage_id {:?} is duplicated",
                    stage.stage_id
                )));
            }
        }
        for stage in stages.values() {
            for dependency in &stage.depends_on {
                if !stages.contains_key(dependency) {
                    return Err(invalid_spec(format!(
                        "stage {:?} depends on unknown stage {:?}",
                        stage.stage_id, dependency
                    )));
                }
            }
        }
        let topological_order = topological_order(&stages)?;
        let canonical_document = ResearchCampaignSpecDocument {
            campaign_id: campaign_id.clone(),
            objective: objective.clone(),
            reconciliation_authority: reconciliation_authority.clone(),
            stages: topological_order
                .iter()
                .map(|stage_id| {
                    let stage = stages.get(stage_id).expect("ordered stage exists");
                    CampaignStageDocument {
                        stage_id: stage.stage_id.clone(),
                        kind: stage.kind,
                        input_digest: stage.input_digest.clone(),
                        depends_on: stage.depends_on.clone(),
                    }
                })
                .collect(),
            max_actions,
        };
        let serialized = serde_json::to_value(canonical_document).map_err(|error| {
            CampaignError::Canonicalisation {
                reason: error.to_string(),
            }
        })?;
        let spec_digest = ContentHash::of_value(&serialized)
            .map_err(|error| CampaignError::Canonicalisation {
                reason: error.to_string(),
            })?
            .to_string();
        Ok(Self {
            campaign_id,
            objective,
            stages,
            topological_order,
            max_actions,
            reconciliation_authority,
            spec_digest,
        })
    }
}

pub(crate) fn validate_reconciliation_authority(
    authority: &CampaignReconciliationAuthorityDocument,
) -> Result<(), CampaignError> {
    bounded_text(
        &authority.authority_id,
        "reconciliation_authority.authority_id",
        MAX_RECONCILIATION_AUTHORITY_ID_BYTES,
    )?;
    bounded_text(
        &authority.protocol_version,
        "reconciliation_authority.protocol_version",
        MAX_RECONCILIATION_AUTHORITY_VERSION_BYTES,
    )?;
    ContentHash::parse(authority.config_digest.clone()).map_err(|_| {
        invalid_spec("reconciliation_authority.config_digest must be a lowercase SHA-256 digest")
    })?;
    Ok(())
}

fn bounded_text(value: &str, field: &str, maximum: usize) -> Result<(), CampaignError> {
    if value.trim().is_empty() || value.contains('\0') || value.len() > maximum {
        return Err(invalid_spec(format!(
            "{field} must be non-empty, NUL-free text of at most {maximum} bytes"
        )));
    }
    Ok(())
}

fn topological_order(
    stages: &BTreeMap<String, CampaignStageSpec>,
) -> Result<Vec<String>, CampaignError> {
    let mut indegree = stages
        .iter()
        .map(|(id, stage)| (id.clone(), stage.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for stage in stages.values() {
        for dependency in &stage.depends_on {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(stage.stage_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(id.clone()))
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(stages.len());
    while let Some(id) = ready.pop_first() {
        ordered.push(id.clone());
        if let Some(children) = dependents.get(&id) {
            for child in children {
                let degree = indegree
                    .get_mut(child)
                    .expect("known dependent has an indegree");
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if ordered.len() != stages.len() {
        return Err(invalid_spec("stage dependencies contain a cycle"));
    }
    Ok(ordered)
}

/// Campaign lifecycle states. Missing input and unknown completion intentionally differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignStatus {
    Planned,
    Ready,
    InFlight,
    NeedsInput,
    AwaitingHumanReview,
    ReconciliationRequired,
    Completed,
    Exhausted,
    Refused,
}

impl CampaignStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Planned => "planned",
            Self::Ready => "ready",
            Self::InFlight => "in_flight",
            Self::NeedsInput => "needs_input",
            Self::AwaitingHumanReview => "awaiting_human_review",
            Self::ReconciliationRequired => "reconciliation_required",
            Self::Completed => "completed",
            Self::Exhausted => "exhausted",
            Self::Refused => "refused",
        }
    }

    pub fn is_terminal_or_paused(self) -> bool {
        !matches!(self, Self::Planned | Self::Ready | Self::InFlight)
    }
}

/// Meaning of a verifier-created receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignReceiptDisposition {
    Succeeded,
    CompletedWithNegativeFindings,
    MissingInput,
    UnknownCompletion,
    AwaitingHumanReview,
    Exhausted,
    Refused,
}

impl CampaignReceiptDisposition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::CompletedWithNegativeFindings => "completed_with_negative_findings",
            Self::MissingInput => "missing_input",
            Self::UnknownCompletion => "unknown_completion",
            Self::AwaitingHumanReview => "awaiting_human_review",
            Self::Exhausted => "exhausted",
            Self::Refused => "refused",
        }
    }

    pub(crate) fn allows_dependents(self) -> bool {
        matches!(self, Self::Succeeded | Self::CompletedWithNegativeFindings)
    }
}
