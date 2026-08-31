//! Multimodal interpretation and visualization assurance.
//!
//! Atlas feature: `AFA-hubapi-P14-F26`.
//!
//! The hub API is the consortium-facing release surface for an interpretation assembled from
//! several studies and modalities.  This module does not manufacture an explanation: it ranks
//! supplied candidates, checks comparability/provenance/replay and protected-closure gates, and
//! returns a signed-artifact-ready receipt in which supported, unknown, contradicted and omitted
//! interpretations remain distinguishable.  Raw experimental data is never moved by this
//! capability; only local, content-addressed metadata is evaluated.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState as FoundationEvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-hubapi-P14-F26";
pub const CONTRACT_VERSION: &str = "hubapi-multimodal-interpretation-assurance/1.0";
pub const MAX_CANDIDATES: usize = 4096;
pub const MAX_STUDIES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

/// A candidate interpretation already computed by a local research workflow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalInterpretationCandidate {
    pub interpretation_id: String,
    pub result_id: String,
    pub visualization_id: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub scope: String,
    pub support_milli: u16,
    pub state: InterpretationState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub baseline_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub competing_explanations: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub reproducible: bool,
    pub boundary: String,
}

/// A bounded, typed request for a cross-study interpretation release assessment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalInterpretationAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub scope: String,
    pub required_study_count: usize,
    pub required_modality_ids: Vec<String>,
    pub minimum_support_milli: u16,
    pub max_admissions: usize,
    pub candidates: Vec<MultimodalInterpretationCandidate>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub budget: u64,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalInterpretationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub scope: String,
    pub disposition: InterpretationDisposition,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub result_order: Vec<String>,
    pub visualization_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub comparability_order: Vec<ContentHash>,
    pub baseline_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: Option<ContentHash>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationAssuranceError {
    #[error("invalid multimodal interpretation assurance request: {0}")]
    Invalid(String),
    #[error("multimodal interpretation assurance artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal interpretation assurance serialization failed: {0}")]
    Serialization(String),
}

impl MultimodalInterpretationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.ranked_order.is_empty()
            || (self.effect_receipts.is_empty()
                && self.disposition != InterpretationDisposition::Qualified)
        {
            return Err(InterpretationAssuranceError::Invalid(
                "identity, ranking, locality, effects, or boundary is incomplete".into(),
            ));
        }
        if self.support_order.len() != self.ranked_order.len()
            || self.support_order.iter().any(|support| *support > 1_000)
            || self.ranked_order.windows(2).any(|pair| pair[0] == pair[1])
            || self
                .admitted_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(InterpretationAssuranceError::Invalid(
                "support or disposition linkage is incomplete".into(),
            ));
        }
        let ranked = self.ranked_order.iter().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().collect::<BTreeSet<_>>();
        let unknown = self.unknown_order.iter().collect::<BTreeSet<_>>();
        let classified = admitted.union(&blocked).collect::<BTreeSet<_>>();
        if admitted.intersection(&blocked).next().is_some()
            || classified.iter().any(|id| !ranked.contains(*id))
            || ranked.iter().any(|id| !classified.contains(id))
            || unknown.iter().any(|id| !blocked.contains(id))
        {
            return Err(InterpretationAssuranceError::Invalid(
                "interpretation dispositions do not partition the ranking".into(),
            ));
        }
        for values in [
            &self.blocked_order,
            &self.unknown_order,
            &self.result_order,
            &self.visualization_order,
            &self.study_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationAssuranceError::Invalid(
                    "multimodal interpretation ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
            &self.comparability_order,
            &self.baseline_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationAssuranceError::Invalid(
                    "multimodal interpretation digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("evaluate:interpretation-assurance:")
                && effect != "block:unsafe-release"
        }) {
            return Err(InterpretationAssuranceError::Invalid(
                "effect is outside the interpretation release gate".into(),
            ));
        }
        let expected_effects = if self.admitted_order.is_empty() {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "evaluate:interpretation-assurance:{}",
                self.request_id
            )]
        };
        if self.effect_receipts != expected_effects {
            return Err(InterpretationAssuranceError::Invalid(
                "interpretation effects do not match admission state".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("multimodal-interpretation-assurance:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.multimodal-interpretation-assurance+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(InterpretationAssuranceError::Artifact(
                "interpretation artifact identity or provenance is invalid".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))?;
        let payload = json!({
            "schema_version": self.schema_version,
            "contract_version": self.contract_version,
            "feature_id": self.feature_id,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "objective_id": self.objective_id,
            "scope": self.scope,
            "disposition": self.disposition,
            "ranked_order": self.ranked_order,
            "admitted_order": self.admitted_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "result_order": self.result_order,
            "visualization_order": self.visualization_order,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "support_order": self.support_order,
            "semantic_order": self.semantic_order,
            "artifact_order": self.artifact_order,
            "evidence_order": self.evidence_order,
            "provenance_order": self.provenance_order,
            "comparability_order": self.comparability_order,
            "baseline_order": self.baseline_order,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "replay_identity": self.replay_identity,
            "benchmark_digest": self.benchmark_digest,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "hubapi".into(),
        consumers: [
            "consortium interpretation administrator".into(),
            "multimodal research workbench".into(),
            "publication release steward".into(),
        ]
        .into(),
        behavior: "assures cross-study, multimodal interpretation and visualization candidates against comparability, evidence, provenance, baseline, replay, omission, policy, federation, locality, and approval gates without upgrading uncertainty into a conclusion".into(),
        value: "provides a deterministic consortium release harness whose admissible interpretation metadata can be replayed across local and federated hub surfaces".into(),
        inputs: vec![TypedPort {
            name: "multimodal_interpretation_assurance_request".into(),
            schema: "MultimodalInterpretationAssuranceRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "multimodal_interpretation_assurance_receipt".into(),
            schema: "MultimodalInterpretationAssuranceReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]
            .into(),
        permissions: [
            "read:local-research-metadata".into(),
            "evaluate:interpretation-release".into(),
            "write:local-interpretation-receipt".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: FoundationEvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: FoundationEvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
            },
            EvidenceReference {
                source_id: "ome-ngff-rfc5".into(),
                state: FoundationEvidenceState::Supported,
                locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()),
            },
            EvidenceReference {
                source_id: "ga4gh-drs-1.3".into(),
                state: FoundationEvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "consortium interpretation reviewer".into(),
            reason: "approve cross-study interpretation release after policy and replay gates".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_multimodal_interpretations(
    request: &MultimodalInterpretationAssuranceRequest,
) -> Result<MultimodalInterpretationAssuranceReceipt, InterpretationAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then_with(|| left.interpretation_id.cmp(&right.interpretation_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.interpretation_id.clone())
        .collect::<Vec<_>>();
    let support_order = candidates
        .iter()
        .map(|candidate| candidate.support_milli)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut visualizations = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut comparability = BTreeSet::new();
    let mut baselines = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let budget_cost = candidate
            .interpretation_id
            .len()
            .checked_add(candidate.result_id.len())
            .and_then(|total| total.checked_add(candidate.visualization_id.len()))
            .and_then(|total| total.checked_add(candidate.study_ids.len()))
            .and_then(|total| total.checked_add(candidate.modality_ids.len()))
            .and_then(|total| u64::try_from(total).ok())
            .and_then(|total| total.checked_add(1))
            .ok_or_else(|| {
                InterpretationAssuranceError::Invalid(
                    "interpretation candidate cost exceeds the representable budget range".into(),
                )
            })?;
        let next_spent = spent.checked_add(budget_cost);
        let budget_ok = next_spent.is_some_and(|total| total <= request.budget);
        let has_modalities = request
            .required_modality_ids
            .iter()
            .all(|required| candidate.modality_ids.contains(required));
        let complete = candidate.state == InterpretationState::Supported
            && candidate.scope == request.scope
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.study_ids.len() >= request.required_study_count
            && has_modalities
            && !candidate.result_id.trim().is_empty()
            && !candidate.visualization_id.trim().is_empty()
            && candidate.baseline_digest.is_some()
            && candidate.comparability_digest.is_some()
            && !candidate.competing_explanations.is_empty()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.negative_evidence.is_empty()
            && candidate.raw_data_local
            && candidate.reproducible
            && candidate.replay_identity == request.replay_identity
            && budget_ok;
        let admitted_now = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && complete
            && admitted.len() < request.max_admissions;
        if admitted_now {
            spent = next_spent.ok_or_else(|| {
                InterpretationAssuranceError::Invalid(
                    "interpretation budget accounting overflowed before admission".into(),
                )
            })?;
            admitted.push(candidate.interpretation_id.clone());
            results.insert(candidate.result_id.clone());
            visualizations.insert(candidate.visualization_id.clone());
            studies.extend(candidate.study_ids.iter().cloned());
            modalities.extend(candidate.modality_ids.iter().cloned());
            semantics.insert(candidate.semantic_digest.clone());
            artifacts.insert(candidate.artifact_digest.clone());
            evidence.insert(candidate.evidence_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
            if let Some(digest) = &candidate.comparability_digest {
                comparability.insert(digest.clone());
            }
            if let Some(baseline) = &candidate.baseline_digest {
                baselines.insert(baseline.clone());
            }
        } else {
            blocked.insert(candidate.interpretation_id.clone());
            if matches!(
                candidate.state,
                InterpretationState::Unknown | InterpretationState::Unmeasured
            ) {
                unknown.insert(candidate.interpretation_id.clone());
                uncertainty.insert(
                    format!(
                        "interpretation:{}:state-{:?}-not-admitted",
                        candidate.interpretation_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == InterpretationState::Contradicted {
                negative.insert(format!(
                    "interpretation:{}:contradicted-negative-evidence",
                    candidate.interpretation_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "interpretation:{}:scope-mismatch",
                    candidate.interpretation_id
                ));
            }
            if candidate.support_milli < request.minimum_support_milli {
                uncertainty.insert(format!(
                    "interpretation:{}:support-below-threshold",
                    candidate.interpretation_id
                ));
            }
            if candidate.study_ids.len() < request.required_study_count {
                omissions.insert(format!(
                    "interpretation:{}:study-coverage-incomplete",
                    candidate.interpretation_id
                ));
            }
            for required in &request.required_modality_ids {
                if !candidate.modality_ids.contains(required) {
                    omissions.insert(format!(
                        "interpretation:{}:modality-missing:{}",
                        candidate.interpretation_id, required
                    ));
                }
            }
            if candidate.baseline_digest.is_none() {
                omissions.insert(format!(
                    "interpretation:{}:baseline-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.result_id.trim().is_empty() {
                omissions.insert(format!(
                    "interpretation:{}:result-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.visualization_id.trim().is_empty() {
                omissions.insert(format!(
                    "interpretation:{}:visualization-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.competing_explanations.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:competing-explanations-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.comparability_digest.is_none() {
                omissions.insert(format!(
                    "interpretation:{}:comparability-missing",
                    candidate.interpretation_id
                ));
            }
            if !candidate.omissions.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:protected-closure-incomplete",
                    candidate.interpretation_id
                ));
            }
            if !candidate.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:uncertainty-unresolved",
                    candidate.interpretation_id
                ));
            }
            if !candidate.negative_evidence.is_empty() {
                negative.insert(format!(
                    "interpretation:{}:negative-evidence-present",
                    candidate.interpretation_id
                ));
            }
            if !candidate.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "interpretation:{}:raw-data-locality-failed",
                    candidate.interpretation_id
                ));
            }
            if !candidate.reproducible || candidate.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "interpretation:{}:replay-not-reproducible",
                    candidate.interpretation_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "interpretation:{}:budget-exhausted",
                    candidate.interpretation_id
                ));
            }
            if admitted.len() >= request.max_admissions {
                omissions.insert(format!(
                    "interpretation:{}:admission-limit",
                    candidate.interpretation_id
                ));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    if request.benchmark_digest.is_none() {
        uncertainty.insert("request:benchmark-missing".into());
    }
    let disposition = if !request.policy_allow
        || !request.federation_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        InterpretationDisposition::Blocked
    } else if admitted.is_empty() {
        InterpretationDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        InterpretationDisposition::Qualified
    } else {
        InterpretationDisposition::Partial
    };
    let effect_receipts = if admitted.is_empty() {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "evaluate:interpretation-assurance:{}",
            request.request_id
        )]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "objective_id": request.objective_id,
        "scope": request.scope,
        "disposition": disposition,
        "ranked_order": ranked_order,
        "admitted_order": admitted,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "result_order": results,
        "visualization_order": visualizations,
        "study_order": studies,
        "modality_order": modalities,
        "support_order": support_order,
        "semantic_order": semantics,
        "artifact_order": artifacts,
        "evidence_order": evidence,
        "provenance_order": provenance,
        "comparability_order": comparability,
        "baseline_order": baselines,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("multimodal-interpretation-assurance:{}", request.request_id),
        "application/vnd.aurora.multimodal-interpretation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))?;
    let receipt = MultimodalInterpretationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        scope: request.scope.clone(),
        disposition,
        ranked_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        result_order: results.into_iter().collect(),
        visualization_order: visualizations.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        support_order,
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        comparability_order: comparability.into_iter().collect(),
        baseline_order: baselines.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &MultimodalInterpretationAssuranceRequest,
) -> Result<(), InterpretationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_study_count == 0
        || request.required_study_count > MAX_STUDIES
        || request.required_modality_ids.is_empty()
        || request.minimum_support_milli > 1000
        || request.max_admissions == 0
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationAssuranceError::Invalid(
            "request identity, modality/study coverage, threshold, limits, candidates, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut required_modalities = BTreeSet::new();
    for modality in &request.required_modality_ids {
        if modality.trim().is_empty() || !required_modalities.insert(modality) {
            return Err(InterpretationAssuranceError::Invalid(
                "required modality identities must be non-empty and unique".into(),
            ));
        }
    }
    for candidate in &request.candidates {
        let mut studies = BTreeSet::new();
        let mut modalities = BTreeSet::new();
        if candidate.interpretation_id.trim().is_empty()
            || candidate.result_id.trim().is_empty()
            || candidate.visualization_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.study_ids.is_empty()
            || candidate.modality_ids.is_empty()
            || candidate.study_ids.len() > MAX_STUDIES
            || candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.interpretation_id.clone())
            || candidate
                .study_ids
                .iter()
                .any(|id| id.trim().is_empty() || !studies.insert(id))
            || candidate
                .modality_ids
                .iter()
                .any(|id| id.trim().is_empty() || !modalities.insert(id))
        {
            return Err(InterpretationAssuranceError::Invalid(format!(
                "interpretation {} is invalid or duplicated",
                candidate.interpretation_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(
        id: &str,
        support_milli: u16,
        state: InterpretationState,
    ) -> MultimodalInterpretationCandidate {
        MultimodalInterpretationCandidate {
            interpretation_id: id.into(),
            result_id: format!("result:{id}"),
            visualization_id: format!("visualization:{id}"),
            study_ids: vec!["study:a".into(), "study:b".into()],
            modality_ids: vec!["imaging".into(), "transcriptomics".into()],
            scope: "organoid:neural".into(),
            support_milli,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash(&format!("comparability:{id}"))),
            baseline_digest: Some(hash("baseline")),
            replay_identity: hash("replay"),
            competing_explanations: vec!["alternative:one".into()],
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            reproducible: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(
        candidates: Vec<MultimodalInterpretationCandidate>,
    ) -> MultimodalInterpretationAssuranceRequest {
        MultimodalInterpretationAssuranceRequest {
            request_id: "request:interpretation".into(),
            workflow_id: "workflow:multimodal-visualization".into(),
            objective_id: "objective:organoid".into(),
            scope: "organoid:neural".into(),
            required_study_count: 2,
            required_modality_ids: vec!["imaging".into(), "transcriptomics".into()],
            minimum_support_milli: 700,
            max_admissions: 4,
            candidates,
            replay_identity: hash("replay"),
            benchmark_digest: Some(hash("benchmark")),
            budget: 10_000,
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_a1_and_nonclinical() {
        let manifest = capability_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(manifest.capability_id, FEATURE_ID);
    }

    #[test]
    fn supported_multimodal_interpretations_are_qualified() {
        let receipt = assure_multimodal_interpretations(&request(vec![
            candidate("interpretation:b", 800, InterpretationState::Supported),
            candidate("interpretation:a", 900, InterpretationState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Qualified);
        assert_eq!(
            receipt.ranked_order,
            vec!["interpretation:a", "interpretation:b"]
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn unknown_and_contradicted_candidates_remain_visible() {
        let receipt = assure_multimodal_interpretations(&request(vec![
            candidate("interpretation:a", 900, InterpretationState::Supported),
            candidate("interpretation:b", 800, InterpretationState::Unknown),
            candidate("interpretation:c", 700, InterpretationState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Partial);
        assert!(receipt.unknown_order.contains(&"interpretation:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("interpretation:c")));
    }

    #[test]
    fn missing_comparability_and_modality_are_omitted() {
        let mut input = request(vec![candidate(
            "interpretation:a",
            900,
            InterpretationState::Supported,
        )]);
        input.candidates[0].modality_ids = vec!["imaging".into()];
        input.candidates[0].comparability_digest = None;
        let receipt = assure_multimodal_interpretations(&input).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("modality-missing")));
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("comparability-missing")));
    }

    #[test]
    fn federation_denial_blocks_release() {
        let mut input = request(vec![candidate(
            "interpretation:a",
            900,
            InterpretationState::Supported,
        )]);
        input.federation_allow = false;
        let receipt = assure_multimodal_interpretations(&input).unwrap();
        assert_eq!(receipt.disposition, InterpretationDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn duplicate_interpretations_are_rejected() {
        let result = assure_multimodal_interpretations(&request(vec![
            candidate("interpretation:a", 900, InterpretationState::Supported),
            candidate("interpretation:a", 800, InterpretationState::Supported),
        ]));
        assert!(result.is_err());
    }

    #[test]
    fn tampered_interpretation_artifact_is_rejected() {
        let mut receipt = assure_multimodal_interpretations(&request(vec![candidate(
            "interpretation:a",
            900,
            InterpretationState::Supported,
        )]))
        .unwrap();
        receipt.result_order.push("result:tampered".into());
        receipt.result_order.sort();
        assert!(receipt.validate().is_err());
    }
}
