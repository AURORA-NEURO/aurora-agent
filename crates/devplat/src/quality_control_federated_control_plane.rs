//! Prospective high-throughput quality-control federated control plane for `AFA-devplat-P07-F31`.
//!
//! This product verifies institution-local, witness-bearing quality summaries. It never reads
//! raw images, sequencing reads, instruments, or human/clinical data. Only deterministic,
//! digest-bound aggregate quality declarations cross the federation boundary.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::{ContentHash, QualityEvidenceState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-devplat-P07-F31";
pub const CONTRACT_VERSION: &str =
    "devplat-prospective-high-throughput-quality-control-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "DevplatQualityBatchRequest5@1";
pub const OUTPUT_SCHEMA: &str = "DevplatQualityControlPlaneReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.devplat-quality-control-plane-7+json";
pub const MAX_BATCH_OBJECTS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObject4 {
    pub object_id: String,
    pub site_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub replay_identity: ContentHash,
    pub quality_report_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub modality_order: Vec<String>,
    pub passed_modality_order: Vec<String>,
    pub pass_fraction_milli: i64,
    pub evidence_state: QualityEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityControlRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_site_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub minimum_site_count: u32,
    pub minimum_pass_fraction_milli: i64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
    pub objects: Vec<ResearchObject4>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityVerdictDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityVerdict7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: QualityVerdictDisposition,
    pub observation_order: Vec<String>,
    pub selected_observation_order: Vec<String>,
    pub unresolved_observation_order: Vec<String>,
    pub blocked_observation_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub passed_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub qualified_site_count: u32,
    pub aggregate_pass_fraction_milli: i64,
    pub replay_identity: ContentHash,
    pub report_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QualityAssuranceError {
    #[error("invalid prospective high-throughput quality-control request: {0}")]
    Invalid(String),
    #[error("prospective high-throughput quality-control artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> QualityAssuranceError {
    QualityAssuranceError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

impl QualityVerdict7 {
    pub fn validate(&self) -> Result<(), QualityAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.observation_order.is_empty()
            || self.site_order.is_empty()
            || self.modality_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "quality identity, observations, sites, modalities, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.observation_order,
            &self.selected_observation_order,
            &self.unresolved_observation_order,
            &self.blocked_observation_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.modality_order,
            &self.passed_modality_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("quality verdict ordering is not canonical"));
            }
        }
        let observations = self
            .observation_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let observation_parts = self
            .selected_observation_order
            .iter()
            .chain(&self.unresolved_observation_order)
            .chain(&self.blocked_observation_order)
            .cloned()
            .collect::<Vec<_>>();
        if observations.len() != self.observation_order.len()
            || observation_parts.len() != observations.len()
            || observation_parts.iter().cloned().collect::<BTreeSet<_>>() != observations
        {
            return Err(invalid(
                "quality observation states do not form a complete partition",
            ));
        }
        let sites = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let site_parts = self
            .selected_site_order
            .iter()
            .chain(&self.unresolved_site_order)
            .chain(&self.blocked_site_order)
            .chain(&self.missing_site_order)
            .cloned()
            .collect::<Vec<_>>();
        if sites.len() != self.site_order.len()
            || site_parts.len() != sites.len()
            || site_parts.iter().cloned().collect::<BTreeSet<_>>() != sites
        {
            return Err(invalid(
                "quality site states do not form a complete partition",
            ));
        }
        let modalities = self.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let modality_parts = self
            .passed_modality_order
            .iter()
            .chain(&self.missing_modality_order)
            .cloned()
            .collect::<Vec<_>>();
        if modalities.len() != self.modality_order.len()
            || modality_parts.len() != modalities.len()
            || modality_parts.iter().cloned().collect::<BTreeSet<_>>() != modalities
        {
            return Err(invalid(
                "quality modality states do not form a complete partition",
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.report_digest)
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.content_hash != self.report_digest
        {
            return Err(QualityAssuranceError::Artifact(
                "quality artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("verify:quality:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("effect is outside the quality assurance gate"));
        }
        if self.disposition == QualityVerdictDisposition::Qualified
            && self.effect_receipts != [format!("verify:quality:{}", self.request_id)]
        {
            return Err(invalid("qualified quality effect is invalid"));
        }
        if self.disposition != QualityVerdictDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(invalid("non-qualified quality verdict must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, QualityAssuranceError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| QualityAssuranceError::Artifact(error.to_string()))?,
        )
        .map_err(|error| QualityAssuranceError::Artifact(error.to_string()))
    }
}

pub fn devplat_quality_control_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "devplat".into(),
        consumers: BTreeSet::from_iter(
            [
                "preclinical researcher".into(),
                "research administrator".into(),
                "federated quality steward".into(),
            ]
            .into_iter(),
        ),
        behavior: "qualifies bounded high-throughput batches of institution-local quality declarations through deterministic site, modality, evidence, policy, replay, budget, and adversarial gates".into(),
        value: "gives research operators a federated control-plane release decision without moving raw imaging or omics data and without hiding incomplete quality evidence".into(),
        surfaces: BTreeSet::from_iter(
            [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator]
                .into_iter(),
        ),
        inputs: vec![TypedPort {
            name: "quality_batch".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "quality_control_plane_receipt".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: BTreeSet::from_iter(
            [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into_iter(),
        ),
        permissions: BTreeSet::from_iter(
            ["evaluate:capability-runs".into(), "read:local-research-artifacts".into()]
                .into_iter(),
        ),
        determinism: Determinism::ByteStable,
        autonomy_tier: AutonomyTier::A1,
        evidence: vec![EvidenceReference {
            source_id: "slsa-provenance-1.2".into(),
            state: EvidenceState::Supported,
            locator: None,
        }],
        authority_requirements: Vec::new(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &QualityControlRequest5) -> Result<(), QualityAssuranceError> {
    if request.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_site_order.is_empty()
        || !canonical(&request.required_site_order)
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_modality_order)
        || request.minimum_site_count == 0
        || !(0..=1000).contains(&request.minimum_pass_fraction_milli)
        || !digest(&request.replay_identity)
        || !canonical(&request.adversarial_events)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.objects.is_empty()
        || request.objects.len() > MAX_BATCH_OBJECTS
    {
        return Err(invalid(
            "quality request identity, axes, threshold, replay, locality, boundary, or objects are invalid",
        ));
    }
    let mut ids = BTreeSet::new();
    for object in &request.objects {
        if object.object_id.trim().is_empty()
            || object.site_id.trim().is_empty()
            || object.study_id.trim().is_empty()
            || object.semantic_profile.trim().is_empty()
            || object.modality_order.is_empty()
            || !canonical(&object.modality_order)
            || !canonical(&object.passed_modality_order)
            || !object
                .passed_modality_order
                .iter()
                .all(|modality| object.modality_order.binary_search(modality).is_ok())
            || !(0..=1000).contains(&object.pass_fraction_milli)
            || !digest(&object.replay_identity)
            || !digest(&object.quality_report_digest)
            || !digest(&object.provenance_digest)
            || !canonical(&object.omission_order)
            || !canonical(&object.uncertainty_order)
            || !ids.insert(object.object_id.clone())
        {
            return Err(invalid(
                "research object identity, modality closure, digests, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn assure_devplat_quality_control_federated_control_plane(
    request: &QualityControlRequest5,
) -> Result<QualityVerdict7, QualityAssuranceError> {
    validate_request(request)?;
    let mut objects = request.objects.clone();
    objects.sort_by(|left, right| left.object_id.cmp(&right.object_id));
    let observation_order = objects
        .iter()
        .map(|object| object.object_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut sites = BTreeSet::from_iter(request.required_site_order.iter().cloned());
    let mut modalities = BTreeSet::from_iter(request.required_modality_order.iter().cloned());
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut by_site: BTreeMap<String, Vec<&ResearchObject4>> = BTreeMap::new();
    for object in &objects {
        sites.insert(object.site_id.clone());
        modalities.extend(object.modality_order.iter().cloned());
        by_site
            .entry(object.site_id.clone())
            .or_default()
            .push(object);
        if object.negative_result {
            negative.insert(format!("{}:negative-result", object.object_id));
        }
        omissions.extend(
            object
                .omission_order
                .iter()
                .map(|reason| format!("{}:{}", object.object_id, reason)),
        );
        uncertainty.extend(
            object
                .uncertainty_order
                .iter()
                .map(|reason| format!("{}:{}", object.object_id, reason)),
        );
        if !object.local_only || !object.aggregate_only || !object.permitted || !object.signed {
            blocked.insert(object.object_id.clone());
            omissions.insert(format!("{}:authorization-or-locality", object.object_id));
        } else if object.stale
            || object.semantic_profile != request.semantic_profile
            || object.replay_identity != request.replay_identity
            || !matches!(
                object.evidence_state,
                QualityEvidenceState::Proven | QualityEvidenceState::Supported
            )
        {
            unresolved.insert(object.object_id.clone());
            if object.stale {
                uncertainty.insert(format!("{}:stale", object.object_id));
            }
            if object.semantic_profile != request.semantic_profile {
                uncertainty.insert(format!("{}:semantic-profile-mismatch", object.object_id));
            }
            if object.replay_identity != request.replay_identity {
                uncertainty.insert(format!("{}:replay-mismatch", object.object_id));
            }
            if object.evidence_state == QualityEvidenceState::Unknown {
                uncertainty.insert(format!("{}:unknown-evidence", object.object_id));
            }
            if object.evidence_state == QualityEvidenceState::Unmeasured {
                uncertainty.insert(format!("{}:unmeasured", object.object_id));
            }
            if object.evidence_state == QualityEvidenceState::Contradicted {
                blocked.remove(&object.object_id);
                unresolved.remove(&object.object_id);
                blocked.insert(object.object_id.clone());
                negative.insert(format!("{}:contradicted", object.object_id));
            }
        } else if object.pass_fraction_milli < request.minimum_pass_fraction_milli {
            unresolved.insert(object.object_id.clone());
            omissions.insert(format!("{}:threshold-failed", object.object_id));
        } else {
            selected.insert(object.object_id.clone());
        }
    }
    let required_sites = request
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected_sites = BTreeSet::new();
    let mut unresolved_sites = BTreeSet::new();
    let mut blocked_sites = BTreeSet::new();
    let mut missing_sites = BTreeSet::new();
    let mut passed_modalities = BTreeSet::new();
    for site in &sites {
        let rows = by_site.get(site).cloned().unwrap_or_default();
        if rows.is_empty() {
            if required_sites.contains(site) {
                missing_sites.insert(site.clone());
                omissions.insert(format!("site:{}:missing", site));
            }
            continue;
        }
        let ids = rows.iter().map(|row| row.object_id.as_str());
        if ids.clone().any(|id| blocked.contains(id)) {
            blocked_sites.insert(site.clone());
        } else if ids.clone().any(|id| unresolved.contains(id)) {
            unresolved_sites.insert(site.clone());
        } else {
            selected_sites.insert(site.clone());
            for row in rows {
                passed_modalities.extend(row.passed_modality_order.iter().cloned());
            }
        }
    }
    passed_modalities.retain(|modality| modalities.contains(modality));
    let missing_modalities = request
        .required_modality_order
        .iter()
        .filter(|modality| !passed_modalities.contains(*modality))
        .cloned()
        .collect::<BTreeSet<_>>();
    if !missing_modalities.is_empty() {
        uncertainty.insert("modality:required-closure-incomplete".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.adversarial_events.is_empty() {
        negative.extend(
            request
                .adversarial_events
                .iter()
                .map(|event| format!("adversarial:{}", event)),
        );
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allow
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty();
    if global_block {
        blocked.extend(observation_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        selected_sites.clear();
        unresolved_sites.clear();
        blocked_sites.extend(sites.iter().cloned());
        omissions.insert("request:quality-release-gate-blocked".into());
    }
    let aggregate_pass_fraction_milli = if selected.is_empty() {
        0
    } else {
        objects
            .iter()
            .filter(|object| selected.contains(&object.object_id))
            .map(|object| object.pass_fraction_milli)
            .sum::<i64>()
            / selected.len() as i64
    };
    let disposition = if global_block || !blocked.is_empty() || !blocked_sites.is_empty() {
        QualityVerdictDisposition::Blocked
    } else if selected_sites.len() < request.minimum_site_count as usize
        || !missing_sites.is_empty()
        || !missing_modalities.is_empty()
        || !unresolved.is_empty()
        || !unresolved_sites.is_empty()
        || aggregate_pass_fraction_milli < request.minimum_pass_fraction_milli
    {
        QualityVerdictDisposition::Unresolved
    } else {
        QualityVerdictDisposition::Qualified
    };
    if disposition != QualityVerdictDisposition::Qualified {
        omissions.insert("request:quality-verdict-not-release-ready".into());
    }
    let effects = if disposition == QualityVerdictDisposition::Qualified {
        vec![format!("verify:quality:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "requester": request.requester,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "observation_order": observation_order,
        "selected_observation_order": selected,
        "unresolved_observation_order": unresolved,
        "blocked_observation_order": blocked,
        "site_order": sites,
        "selected_site_order": selected_sites,
        "unresolved_site_order": unresolved_sites,
        "blocked_site_order": blocked_sites,
        "missing_site_order": missing_sites,
        "modality_order": modalities,
        "passed_modality_order": passed_modalities,
        "missing_modality_order": missing_modalities,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "qualified_site_count": selected_sites.len() as u32,
        "aggregate_pass_fraction_milli": aggregate_pass_fraction_milli,
        "replay_identity": request.replay_identity,
        "effect_receipts": effects,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let report_digest = ContentHash::of_value(&payload)
        .map_err(|error| QualityAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("devplat-quality-control-plane-7:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| QualityAssuranceError::Artifact(error.to_string()))?;
    if artifact.content_hash != report_digest {
        return Err(QualityAssuranceError::Artifact(
            "quality report digest is not content-addressed".into(),
        ));
    }
    let receipt = QualityVerdict7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        observation_order: payload["observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_observation_order: payload["selected_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_observation_order: payload["unresolved_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_observation_order: payload["blocked_observation_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        site_order: payload["site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_site_order: payload["selected_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_site_order: payload["unresolved_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_site_order: payload["blocked_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_site_order: payload["missing_site_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        modality_order: payload["modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        passed_modality_order: payload["passed_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_modality_order: payload["missing_modality_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        qualified_site_count: payload["qualified_site_count"].as_u64().unwrap() as u32,
        aggregate_pass_fraction_milli,
        replay_identity: request.replay_identity.clone(),
        report_digest: report_digest.clone(),
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn object(id: &str, site: &str, replay: &ContentHash) -> ResearchObject4 {
        ResearchObject4 {
            object_id: id.into(),
            site_id: site.into(),
            study_id: "study-a".into(),
            semantic_profile: "ome-ngff:rfc5".into(),
            replay_identity: replay.clone(),
            quality_report_digest: hash(&format!("report-{id}")),
            provenance_digest: hash(&format!("provenance-{id}")),
            modality_order: vec!["imaging".into(), "omics".into()],
            passed_modality_order: vec!["imaging".into(), "omics".into()],
            pass_fraction_milli: 990,
            evidence_state: QualityEvidenceState::Supported,
            signed: true,
            permitted: true,
            local_only: true,
            aggregate_only: true,
            stale: false,
            negative_result: false,
            omission_order: Vec::new(),
            uncertainty_order: Vec::new(),
        }
    }

    fn request() -> QualityControlRequest5 {
        let replay = hash("replay");
        QualityControlRequest5 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "q1".into(),
            requester: "researcher".into(),
            purpose: "federated qc".into(),
            semantic_profile: "ome-ngff:rfc5".into(),
            required_site_order: vec!["site-a".into(), "site-b".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            minimum_site_count: 2,
            minimum_pass_fraction_milli: 950,
            replay_identity: replay.clone(),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            objects: vec![
                object("o2", "site-b", &replay),
                object("o1", "site-a", &replay),
            ],
        }
    }

    #[test]
    fn manifest_is_typed() {
        assert_eq!(
            devplat_quality_control_federated_control_plane_manifest().capability_id,
            FEATURE_ID
        );
    }

    #[test]
    fn complete_federation_qualifies() {
        let receipt = assure_devplat_quality_control_federated_control_plane(&request()).unwrap();
        assert_eq!(receipt.disposition, QualityVerdictDisposition::Qualified);
        assert_eq!(receipt.selected_site_order, vec!["site-a", "site-b"]);
        receipt.validate().unwrap();
    }

    #[test]
    fn stale_object_is_unresolved() {
        let mut req = request();
        req.objects[0].stale = true;
        let receipt = assure_devplat_quality_control_federated_control_plane(&req).unwrap();
        assert_eq!(receipt.disposition, QualityVerdictDisposition::Unresolved);
        assert!(receipt
            .uncertainty_order
            .iter()
            .any(|item| item.ends_with(":stale")));
    }

    #[test]
    fn contradicted_object_blocks() {
        let mut req = request();
        req.objects[0].evidence_state = QualityEvidenceState::Contradicted;
        let receipt = assure_devplat_quality_control_federated_control_plane(&req).unwrap();
        assert_eq!(receipt.disposition, QualityVerdictDisposition::Blocked);
        assert!(receipt
            .negative_evidence_order
            .iter()
            .any(|item| item.ends_with(":contradicted")));
    }

    #[test]
    fn federation_denial_fails_closed() {
        let mut req = request();
        req.federation_allow = false;
        let receipt = assure_devplat_quality_control_federated_control_plane(&req).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert_eq!(receipt.disposition, QualityVerdictDisposition::Blocked);
    }

    #[test]
    fn missing_site_is_explicit() {
        let mut req = request();
        req.objects.remove(0);
        let receipt = assure_devplat_quality_control_federated_control_plane(&req).unwrap();
        assert_eq!(receipt.disposition, QualityVerdictDisposition::Unresolved);
        assert_eq!(receipt.missing_site_order, vec!["site-b"]);
    }

    #[test]
    fn duplicate_object_is_rejected() {
        let mut req = request();
        req.objects[1].object_id = req.objects[0].object_id.clone();
        assert!(matches!(
            assure_devplat_quality_control_federated_control_plane(&req),
            Err(QualityAssuranceError::Invalid(_))
        ));
    }

    #[test]
    fn canonical_order_is_reproducible() {
        let first = assure_devplat_quality_control_federated_control_plane(&request()).unwrap();
        let mut reversed = request();
        reversed.objects.reverse();
        let second = assure_devplat_quality_control_federated_control_plane(&reversed).unwrap();
        assert_eq!(first, second);
    }
}
