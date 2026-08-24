//! Omission-aware multimodal replication evidence sets.
//!
//! Atlas feature: `AFA-evalengine-P15-F02`.
//!
//! This is a manifest-level replication product. It never reads raw imaging or
//! omics bytes; it first proves that each study's declared modality contracts
//! are comparable, then aggregates only the comparable observations while
//! retaining every omitted, contradictory, and null result.

use bioprism_foundation::{
    CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState, LossSeverity,
    ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-evalengine-P15-F02";
pub const FEATURE_VERSION: &str = "0.1.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityReceipt {
    pub schema: String,
    pub unit: String,
    pub coordinate_system: String,
    pub data_digest: ContentHash,
    pub qc_digest: Option<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalReplicationObservation {
    pub study_id: String,
    pub site: String,
    pub assay: String,
    pub modalities: BTreeMap<String, ModalityReceipt>,
    pub outcome: super::replication::ReplicationOutcome,
    pub effect: Option<f64>,
    pub uncertainty: Option<f64>,
    pub independent: bool,
    pub preregistered: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalReplicationPolicy {
    pub minimum_independent_sites: usize,
    pub require_preregistered: bool,
    pub require_qc: bool,
    pub max_effect_disagreement: Option<f64>,
}

impl Default for MultimodalReplicationPolicy {
    fn default() -> Self {
        Self {
            minimum_independent_sites: 2,
            require_preregistered: true,
            require_qc: true,
            max_effect_disagreement: Some(0.5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalReplicationRequest {
    pub capability_id: String,
    pub claim: String,
    pub required_modalities: BTreeSet<String>,
    pub observations: Vec<MultimodalReplicationObservation>,
    pub policy: MultimodalReplicationPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyComparability {
    pub study_id: String,
    pub site: String,
    pub assay: String,
    pub comparable: bool,
    pub omitted_modalities: Vec<String>,
    pub reasons: Vec<String>,
    pub modality_digests: BTreeMap<String, ContentHash>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalReplicationDisposition {
    Replicated,
    PartiallyReplicated,
    Contradicted,
    NullResult,
    InsufficientEvidence,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalReplicationSummary {
    pub disposition: MultimodalReplicationDisposition,
    pub total_observations: usize,
    pub comparable_observations: usize,
    pub independent_sites: usize,
    pub positive_count: usize,
    pub null_count: usize,
    pub negative_count: usize,
    pub inconclusive_count: usize,
    pub omitted_observations: usize,
    pub contradictory_observations: usize,
    pub effect_min: Option<f64>,
    pub effect_max: Option<f64>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MultimodalReplicationReport {
    pub schema_version: String,
    pub feature_id: String,
    pub capability_id: String,
    pub request_digest: ContentHash,
    pub summary: MultimodalReplicationSummary,
    pub studies: Vec<StudyComparability>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl MultimodalReplicationReport {
    pub fn validate(&self) -> Result<(), MultimodalReplicationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.feature_id != FEATURE_ID
        {
            return Err(MultimodalReplicationError::InvalidField(
                "schema or feature".into(),
            ));
        }
        if self.capability_id.trim().is_empty()
            || self.summary.total_observations == 0
            || self.studies.len() != self.summary.total_observations
            || self.summary.reasons.is_empty()
        {
            return Err(MultimodalReplicationError::InvalidField(
                "report identity, study count, or reasons".into(),
            ));
        }
        if self.summary.comparable_observations > self.summary.total_observations
            || self.summary.omitted_observations > self.summary.total_observations
        {
            return Err(MultimodalReplicationError::InvalidField(
                "summary counts".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(MultimodalReplicationError::InvalidField("boundary".into()));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalReplicationError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalReplicationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalReplicationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalReplicationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum MultimodalReplicationError {
    #[error("invalid multimodal replication field: {0}")]
    InvalidField(String),
    #[error("duplicate observation for {study_id}/{site}/{assay}")]
    DuplicateObservation {
        study_id: String,
        site: String,
        assay: String,
    },
    #[error("required modality set is empty")]
    EmptyRequiredModalities,
    #[error("invalid multimodal replication measurement: {0}")]
    InvalidMeasurement(String),
    #[error("artifact error: {0}")]
    Artifact(String),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn multimodal_replication_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_VERSION.into(),
        owner_crate: "evalengine".into(),
        consumers: ["bioinformatician".into(), "replication scientist".into()].into(),
        behavior: "checks multimodal study comparability before aggregating independent replication evidence and preserves omitted, null, negative, and contradictory results".into(),
        value: "prevents incompatible imaging and omics studies from being averaged into a false replication claim".into(),
        inputs: vec![TypedPort {
            name: "multimodal_replication_request".into(),
            schema: "MultimodalReplicationRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "multimodal_replication_report".into(),
            schema: "MultimodalReplicationReport@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:institution-local-replication-manifests".into(), "write:local-research-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "fixture:multimodal-replication-evidence-set".into(),
            state: EvidenceState::Supported,
            locator: Some("fixtures/multimodal-replication".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: bioprism_foundation::AutonomyTier::A0,
        surfaces: [ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn evaluate_multimodal_replication(
    request: &MultimodalReplicationRequest,
) -> Result<MultimodalReplicationReport, MultimodalReplicationError> {
    validate_request(request)?;
    let request_value = serde_json::to_value(request)
        .map_err(|error| MultimodalReplicationError::Serialization(error.to_string()))?;
    let request_digest = ContentHash::of_value(&request_value)
        .map_err(|error| MultimodalReplicationError::Serialization(error.to_string()))?;

    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        (&left.study_id, &left.site, &left.assay).cmp(&(&right.study_id, &right.site, &right.assay))
    });
    let reference = reference_signatures(&observations, &request.required_modalities);
    let mut studies = Vec::with_capacity(observations.len());
    let mut independent_sites = BTreeSet::new();
    let mut effects = Vec::new();
    let mut positive_count = 0;
    let mut null_count = 0;
    let mut negative_count = 0;
    let mut inconclusive_count = 0;
    let mut comparable_observations = 0;
    let mut omitted_observations = 0;
    let mut contradictory_observations = 0;

    for observation in &observations {
        let mut omitted_modalities = request
            .required_modalities
            .difference(&observation.modalities.keys().cloned().collect())
            .cloned()
            .collect::<Vec<_>>();
        let mut reasons = Vec::new();
        omitted_modalities.sort();
        if !omitted_modalities.is_empty() {
            reasons.push(format!(
                "required modalities omitted: {}",
                omitted_modalities.join(", ")
            ));
        }
        if request.policy.require_qc
            && observation.modalities.iter().any(|(modality, receipt)| {
                request.required_modalities.contains(modality) && receipt.qc_digest.is_none()
            })
        {
            reasons.push("required modality lacks a QC digest".into());
        }
        for modality in &request.required_modalities {
            if let (Some(expected), Some(actual)) = (
                reference.get(modality),
                observation.modalities.get(modality),
            ) {
                if expected.schema != actual.schema
                    || expected.unit != actual.unit
                    || expected.coordinate_system != actual.coordinate_system
                {
                    reasons.push(format!("modality contract conflict: {modality}"));
                }
            }
        }
        if request.policy.require_preregistered && !observation.preregistered {
            reasons.push("observation lacks preregistration evidence".into());
        }
        let comparable = reasons.is_empty();
        if comparable {
            comparable_observations += 1;
            if observation.independent {
                independent_sites.insert(observation.site.clone());
            }
            if let Some(effect) = observation.effect {
                effects.push(effect);
            }
            match observation.outcome {
                super::replication::ReplicationOutcome::Positive => positive_count += 1,
                super::replication::ReplicationOutcome::Null => null_count += 1,
                super::replication::ReplicationOutcome::Negative => negative_count += 1,
                super::replication::ReplicationOutcome::Inconclusive => inconclusive_count += 1,
            }
        } else {
            omitted_observations += 1;
        }
        if reasons.iter().any(|reason| reason.contains("conflict")) {
            contradictory_observations += 1;
        }
        studies.push(StudyComparability {
            study_id: observation.study_id.clone(),
            site: observation.site.clone(),
            assay: observation.assay.clone(),
            comparable,
            omitted_modalities,
            reasons,
            modality_digests: observation
                .modalities
                .iter()
                .map(|(name, receipt)| (name.clone(), receipt.data_digest.clone()))
                .collect(),
        });
    }
    effects.sort_by(f64::total_cmp);
    let effect_min = effects.first().copied();
    let effect_max = effects.last().copied();
    let disagreement = effect_min.zip(effect_max).map(|(min, max)| max - min);
    let effect_consistent = request
        .policy
        .max_effect_disagreement
        .map_or(true, |threshold| {
            disagreement.map_or(true, |delta| delta <= threshold)
        });
    let preregistration_ok = !request.policy.require_preregistered
        || studies.iter().all(|study| {
            study.comparable
                && !study
                    .reasons
                    .iter()
                    .any(|reason| reason.contains("preregistration"))
        });
    let mut reasons = Vec::new();
    if omitted_observations > 0 {
        reasons.push(format!("{omitted_observations} observations omitted from aggregation due to comparability gates"));
    }
    if independent_sites.len() < request.policy.minimum_independent_sites {
        reasons.push(format!(
            "independent-site floor unmet: {} < {}",
            independent_sites.len(),
            request.policy.minimum_independent_sites
        ));
    }
    if !effect_consistent {
        reasons.push("comparable effect estimates exceed the disagreement threshold".into());
    }
    if !preregistration_ok {
        reasons.push("one or more comparable observations lack preregistration evidence".into());
    }
    let disposition = if negative_count > 0 || contradictory_observations > 0 || !effect_consistent
    {
        reasons.push("negative or contradictory multimodal evidence is retained".into());
        MultimodalReplicationDisposition::Contradicted
    } else if comparable_observations > 0
        && positive_count == 0
        && null_count == comparable_observations
    {
        reasons
            .push("all comparable observations are null; null result remains publishable".into());
        MultimodalReplicationDisposition::NullResult
    } else if positive_count > 0
        && preregistration_ok
        && effect_consistent
        && independent_sites.len() >= request.policy.minimum_independent_sites
        && inconclusive_count == 0
        && omitted_observations == 0
    {
        reasons.push("positive effects reproduced across comparable independent sites".into());
        MultimodalReplicationDisposition::Replicated
    } else if positive_count > 0 {
        reasons.push(
            "positive observations exist but one or more multimodal replication gates remain open"
                .into(),
        );
        MultimodalReplicationDisposition::PartiallyReplicated
    } else {
        reasons.push("no decisive positive multimodal replication claim is supported".into());
        MultimodalReplicationDisposition::InsufficientEvidence
    };
    if reasons.is_empty() {
        reasons.push("no comparable evidence was available".into());
    }
    let summary = MultimodalReplicationSummary {
        disposition,
        total_observations: observations.len(),
        comparable_observations,
        independent_sites: independent_sites.len(),
        positive_count,
        null_count,
        negative_count,
        inconclusive_count,
        omitted_observations,
        contradictory_observations,
        effect_min,
        effect_max,
        reasons,
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "capability_id": request.capability_id,
        "claim": request.claim,
        "request_digest": request_digest,
        "required_modalities": request.required_modalities,
        "summary": summary,
        "studies": studies,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = observations
        .iter()
        .flat_map(|observation| {
            observation
                .modalities
                .iter()
                .map(move |(modality, receipt)| ProvenanceLink {
                    source_id: format!(
                        "{}:{}:{}",
                        observation.study_id, observation.site, modality
                    ),
                    relation: "multimodal-replication-modality-digest".into(),
                    digest: receipt.data_digest.clone(),
                })
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("multimodal-replication:{}", request.capability_id),
        "application/vnd.aurora.multimodal-replication+json",
        &payload,
        vec![SemanticLoss {
            field: "raw_measurements".into(),
            reason: "raw imaging and omics bytes remain institution-local; the report exports only typed modality and QC digests".into(),
            severity: LossSeverity::Bounded,
        }],
        provenance,
    )
    .map_err(|error| MultimodalReplicationError::Artifact(error.to_string()))?;
    let report = MultimodalReplicationReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        capability_id: request.capability_id.clone(),
        request_digest,
        summary,
        studies,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

fn reference_signatures(
    observations: &[MultimodalReplicationObservation],
    required_modalities: &BTreeSet<String>,
) -> BTreeMap<String, ModalityReceipt> {
    let mut reference = BTreeMap::new();
    for observation in observations {
        for modality in required_modalities {
            if let Some(receipt) = observation.modalities.get(modality) {
                reference
                    .entry(modality.clone())
                    .or_insert_with(|| receipt.clone());
            }
        }
    }
    reference
}

fn validate_request(
    request: &MultimodalReplicationRequest,
) -> Result<(), MultimodalReplicationError> {
    if request.capability_id.trim().is_empty()
        || request.claim.trim().is_empty()
        || request.observations.is_empty()
    {
        return Err(MultimodalReplicationError::InvalidField(
            "capability_id, claim, and observations are required".into(),
        ));
    }
    if request.required_modalities.is_empty() {
        return Err(MultimodalReplicationError::EmptyRequiredModalities);
    }
    if request.policy.minimum_independent_sites == 0 {
        return Err(MultimodalReplicationError::InvalidField(
            "minimum_independent_sites must be positive".into(),
        ));
    }
    if request
        .policy
        .max_effect_disagreement
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        return Err(MultimodalReplicationError::InvalidMeasurement(
            "max_effect_disagreement must be finite and non-negative".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for observation in &request.observations {
        if observation.study_id.trim().is_empty()
            || observation.site.trim().is_empty()
            || observation.assay.trim().is_empty()
        {
            return Err(MultimodalReplicationError::InvalidField(
                "study_id, site, and assay are required".into(),
            ));
        }
        if !keys.insert((
            observation.study_id.clone(),
            observation.site.clone(),
            observation.assay.clone(),
        )) {
            return Err(MultimodalReplicationError::DuplicateObservation {
                study_id: observation.study_id.clone(),
                site: observation.site.clone(),
                assay: observation.assay.clone(),
            });
        }
        if observation.effect.is_some_and(|value| !value.is_finite())
            || observation
                .uncertainty
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(MultimodalReplicationError::InvalidMeasurement(
                "effect must be finite and uncertainty finite/non-negative".into(),
            ));
        }
        for (modality, receipt) in &observation.modalities {
            if modality.trim().is_empty()
                || receipt.schema.trim().is_empty()
                || receipt.unit.trim().is_empty()
                || receipt.coordinate_system.trim().is_empty()
            {
                return Err(MultimodalReplicationError::InvalidField(
                    "modality contracts need names, schemas, units, and coordinates".into(),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn modality(label: &str, unit: &str) -> ModalityReceipt {
        ModalityReceipt {
            schema: "ome-ngff/0.5".into(),
            unit: unit.into(),
            coordinate_system: "physical-zxy".into(),
            data_digest: digest(label),
            qc_digest: Some(digest(&format!("qc-{label}"))),
        }
    }

    fn request() -> MultimodalReplicationRequest {
        let mut first = BTreeMap::new();
        first.insert("image".into(), modality("image-a", "micrometer"));
        first.insert(
            "rna".into(),
            ModalityReceipt {
                schema: "anndata/0.9".into(),
                unit: "counts".into(),
                coordinate_system: "cell-id".into(),
                data_digest: digest("rna-a"),
                qc_digest: Some(digest("qc-rna-a")),
            },
        );
        let mut second = first.clone();
        second.get_mut("image").unwrap().data_digest = digest("image-b");
        second.get_mut("rna").unwrap().data_digest = digest("rna-b");
        MultimodalReplicationRequest {
            capability_id: "capability:multimodal-replication".into(),
            claim: "organoid mechanism reproduces across sites".into(),
            required_modalities: ["image".into(), "rna".into()].into(),
            observations: vec![
                MultimodalReplicationObservation {
                    study_id: "study-a".into(),
                    site: "site-a".into(),
                    assay: "paired".into(),
                    modalities: first,
                    outcome: super::super::replication::ReplicationOutcome::Positive,
                    effect: Some(0.2),
                    uncertainty: Some(0.1),
                    independent: true,
                    preregistered: true,
                },
                MultimodalReplicationObservation {
                    study_id: "study-b".into(),
                    site: "site-b".into(),
                    assay: "paired".into(),
                    modalities: second,
                    outcome: super::super::replication::ReplicationOutcome::Positive,
                    effect: Some(0.25),
                    uncertainty: Some(0.1),
                    independent: true,
                    preregistered: true,
                },
            ],
            policy: MultimodalReplicationPolicy::default(),
        }
    }

    #[test]
    fn comparable_sites_can_replicate() {
        let report = evaluate_multimodal_replication(&request()).unwrap();
        assert_eq!(
            report.summary.disposition,
            MultimodalReplicationDisposition::Replicated
        );
        assert_eq!(report.summary.comparable_observations, 2);
    }

    #[test]
    fn missing_modality_is_not_averaged_away() {
        let mut request = request();
        request.observations[1].modalities.remove("rna");
        let report = evaluate_multimodal_replication(&request).unwrap();
        assert_eq!(report.summary.omitted_observations, 1);
        assert_eq!(
            report.summary.disposition,
            MultimodalReplicationDisposition::PartiallyReplicated
        );
    }

    #[test]
    fn unit_conflict_is_contradictory_and_fail_closed() {
        let mut request = request();
        request.observations[1]
            .modalities
            .get_mut("image")
            .unwrap()
            .unit = "pixel".into();
        let report = evaluate_multimodal_replication(&request).unwrap();
        assert_eq!(report.summary.contradictory_observations, 1);
        assert_eq!(
            report.summary.disposition,
            MultimodalReplicationDisposition::Contradicted
        );
    }
}
