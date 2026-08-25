//! Witness-bearing research-object quality-control assurance harness.
//!
//! Atlas feature: `AFA-hubapi-P07-F27`.
//!
//! This module verifies a caller-supplied quality envelope. It does not silently impute metrics,
//! fetch missing material, or convert an unresolved quality state into a release decision. Every
//! pass, failure, omission, contradiction, and negative result remains in a deterministic,
//! content-addressed `QualityVerdict`.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState as FoundationEvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-hubapi-P07-F27";
pub const CONTRACT_VERSION: &str = "hubapi-quality-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetricState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityDisposition {
    Qualified,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityMetric {
    pub metric_id: String,
    pub modality_id: String,
    pub priority_milli: u16,
    pub value_milli: Option<i32>,
    pub threshold_milli: i32,
    pub state: MetricState,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchObject {
    pub object_id: String,
    pub study_id: String,
    pub scope: String,
    pub target_schema: String,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub metrics: Vec<QualityMetric>,
    pub required_metric_ids: Vec<String>,
    pub max_metrics: usize,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityVerdict {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub object_id: String,
    pub study_id: String,
    pub scope: String,
    pub target_schema: String,
    pub disposition: QualityDisposition,
    pub ranked_metric_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityAssuranceError {
    #[error("invalid research object: {0}")]
    Invalid(String),
    #[error("quality assurance contract failed: {0}")]
    Contract(String),
}

impl QualityVerdict {
    pub fn validate(&self) -> Result<(), QualityAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.object_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.ranked_metric_order.is_empty()
            || (self.effect_receipts.is_empty()
                && self.disposition != QualityDisposition::Qualified)
        {
            return Err(QualityAssuranceError::Contract(
                "quality verdict identity, ranking, locality, effects, or boundary is incomplete"
                    .into(),
            ));
        }
        if self
            .ranked_metric_order
            .windows(2)
            .any(|pair| pair[0] == pair[1])
        {
            return Err(QualityAssuranceError::Contract(
                "quality verdict ranking contains duplicate metric identity".into(),
            ));
        }
        for values in [
            &self.passed_order,
            &self.failed_order,
            &self.unknown_order,
            &self.witness_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityAssuranceError::Contract(
                    "quality verdict ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(QualityAssuranceError::Contract(
                    "quality verdict digest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| effect != "block:unsafe-release")
        {
            return Err(QualityAssuranceError::Contract(
                "quality verdict effect is outside the unsafe-release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| QualityAssuranceError::Contract(error.to_string()))?;
        Ok(())
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "hubapi".into(),
        consumers: ["AURORA extension developer".into()].into(),
        behavior: "verifies a typed research-object quality envelope with witness-bearing metric verdicts and fail-closed release gating".into(),
        value: "increases auditable discovery rate without hiding missing, contradictory, or unmeasured quality evidence".into(),
        inputs: vec![TypedPort {
            name: "research_object".into(),
            schema: "ResearchObject3@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "quality_verdict".into(),
            schema: "QualityVerdict7@1".into(),
            required: true,
        }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "slsa-provenance-1.2".into(),
            state: FoundationEvidenceState::Supported,
            locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure(object: &ResearchObject) -> Result<QualityVerdict, QualityAssuranceError> {
    validate_object(object)?;
    let mut metrics = object.metrics.clone();
    metrics.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.metric_id.cmp(&right.metric_id))
    });
    let ranked_metric_order = metrics
        .iter()
        .map(|metric| metric.metric_id.clone())
        .collect::<Vec<_>>();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut witnesses = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for metric in &metrics {
        let cost = metric.metric_id.len() as u64 + metric.modality_id.len() as u64 + 1;
        if cost > object.budget.saturating_sub(spent) {
            unknown.insert(metric.metric_id.clone());
            omissions.insert(format!(
                "metric:{}:budget-ceiling-exceeded",
                metric.metric_id
            ));
            continue;
        }
        let (Some(artifact_digest), Some(evidence_digest), Some(provenance_digest)) = (
            object.artifact_digest.clone(),
            metric.evidence_digest.clone(),
            metric.provenance_digest.clone(),
        ) else {
            unknown.insert(metric.metric_id.clone());
            omissions.insert(format!(
                "metric:{}:artifact-evidence-or-provenance-digest-missing",
                metric.metric_id
            ));
            continue;
        };
        if !metric.omissions.is_empty() {
            unknown.insert(metric.metric_id.clone());
            omissions.extend(
                metric
                    .omissions
                    .iter()
                    .map(|item| format!("metric:{}:{item}", metric.metric_id)),
            );
            continue;
        }
        if !metric.uncertainty.is_empty() {
            unknown.insert(metric.metric_id.clone());
            uncertainty.extend(
                metric
                    .uncertainty
                    .iter()
                    .map(|item| format!("metric:{}:{item}", metric.metric_id)),
            );
            continue;
        }
        modalities.insert(metric.modality_id.clone());
        artifacts.insert(artifact_digest);
        evidence.insert(evidence_digest);
        provenance.insert(provenance_digest);
        spent = spent.saturating_add(cost);
        match metric.state {
            MetricState::Contradicted => {
                failed.insert(metric.metric_id.clone());
                negative.insert(format!(
                    "metric:{}:contradicted-quality-evidence",
                    metric.metric_id
                ));
            }
            MetricState::Unknown | MetricState::Unmeasured => {
                unknown.insert(metric.metric_id.clone());
                uncertainty.insert(
                    format!(
                        "metric:{}:state-{:?}-not-qualified",
                        metric.metric_id, metric.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            MetricState::Supported => match metric.value_milli {
                Some(value) if value >= metric.threshold_milli => {
                    passed.insert(metric.metric_id.clone());
                    witnesses.insert(format!(
                        "metric:{}:value={}:threshold={}:pass",
                        metric.metric_id, value, metric.threshold_milli
                    ));
                    if metric.negative_result {
                        negative.insert(format!(
                            "metric:{}:negative-result-retained",
                            metric.metric_id
                        ));
                    }
                }
                Some(value) => {
                    failed.insert(metric.metric_id.clone());
                    witnesses.insert(format!(
                        "metric:{}:value={}:threshold={}:fail",
                        metric.metric_id, value, metric.threshold_milli
                    ));
                    negative.insert(format!(
                        "metric:{}:below-threshold-negative-result",
                        metric.metric_id
                    ));
                }
                None => {
                    unknown.insert(metric.metric_id.clone());
                    omissions.insert(format!("metric:{}:value-unmeasured", metric.metric_id));
                }
            },
        }
    }
    for required in &object.required_metric_ids {
        if !passed.contains(required) {
            omissions.insert(format!("metric:{}:required-but-not-passed", required));
        }
    }
    if !object.policy_allow {
        failed.insert("object:policy-denied".into());
        negative.insert("object:policy-denied-no-quality-release".into());
    }
    if !object.protected_closure {
        unknown.insert("object:protected-closure-incomplete".into());
        uncertainty.insert("object:protected-closure-incomplete".into());
    }
    if !object.raw_data_local {
        failed.insert("object:raw-data-locality-required".into());
        omissions.insert("object:raw-data-locality-required".into());
    }
    let passed_order = passed.into_iter().collect::<Vec<_>>();
    let failed_order = failed.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let witness_order = witnesses.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let disposition = if !object.policy_allow || !object.raw_data_local {
        QualityDisposition::Blocked
    } else if passed_order.is_empty() {
        QualityDisposition::Unknown
    } else if !failed_order.is_empty()
        || !unknown_order.is_empty()
        || !omissions.is_empty()
        || !uncertainty.is_empty()
        || !object.protected_closure
    {
        QualityDisposition::Conditional
    } else {
        QualityDisposition::Qualified
    };
    let effect_receipts = if disposition == QualityDisposition::Qualified {
        Vec::new()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "object_id": object.object_id,
        "study_id": object.study_id,
        "scope": object.scope,
        "target_schema": object.target_schema,
        "disposition": disposition,
        "ranked_metric_order": ranked_metric_order,
        "passed_order": passed_order,
        "failed_order": failed_order,
        "unknown_order": unknown_order,
        "witness_order": witness_order,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": object.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("quality-verdict:{}", object.object_id),
        "application/vnd.aurora.quality-verdict+json",
        &payload,
        Vec::new(),
        evidence_order
            .iter()
            .map(|digest| bioprism_foundation::ProvenanceLink {
                source_id: digest.to_string(),
                relation: "quality-evidence".into(),
                digest: digest.clone(),
            })
            .collect(),
    )
    .map_err(|error| QualityAssuranceError::Contract(error.to_string()))?;
    let verdict = QualityVerdict {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        object_id: object.object_id.clone(),
        study_id: object.study_id.clone(),
        scope: object.scope.clone(),
        target_schema: object.target_schema.clone(),
        disposition,
        ranked_metric_order,
        passed_order,
        failed_order,
        unknown_order,
        witness_order,
        modality_order,
        artifact_order,
        evidence_order,
        provenance_order,
        omissions,
        uncertainty,
        negative_evidence,
        replay_identity: object.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    verdict.validate()?;
    Ok(verdict)
}

fn validate_object(object: &ResearchObject) -> Result<(), QualityAssuranceError> {
    if object.object_id.trim().is_empty()
        || object.study_id.trim().is_empty()
        || object.scope.trim().is_empty()
        || object.target_schema.trim().is_empty()
        || object.metrics.is_empty()
        || object.max_metrics == 0
        || object.budget == 0
        || object.boundary != PRECLINICAL_BOUNDARY
        || object
            .required_metric_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(QualityAssuranceError::Invalid(
            "research object identity, metrics, closure, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for metric in &object.metrics {
        if metric.metric_id.trim().is_empty()
            || metric.modality_id.trim().is_empty()
            || metric.priority_milli > 1_000
            || metric.threshold_milli < 0
            || metric.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(metric.metric_id.clone())
            || metric.omissions.windows(2).any(|pair| pair[0] >= pair[1])
            || metric.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(QualityAssuranceError::Invalid(format!(
                "metric {} is invalid or duplicated",
                metric.metric_id
            )));
        }
    }
    if object
        .required_metric_ids
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(QualityAssuranceError::Invalid(
            "required metric closure references an unknown metric".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn metric(
        id: &str,
        state: MetricState,
        value: Option<i32>,
        negative_result: bool,
    ) -> QualityMetric {
        QualityMetric {
            metric_id: id.into(),
            modality_id: if id.ends_with('a') {
                "imaging"
            } else {
                "omics"
            }
            .into(),
            priority_milli: if id.ends_with('a') { 950 } else { 800 },
            value_milli: value,
            threshold_milli: 700,
            state,
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            negative_result,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn object(metrics: Vec<QualityMetric>) -> ResearchObject {
        ResearchObject {
            object_id: "object:quality".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            target_schema: "quality-verdict/7".into(),
            artifact_digest: Some(hash("artifact")),
            provenance_digest: Some(hash("object-provenance")),
            metrics,
            required_metric_ids: vec!["metric:a".into(), "metric:b".into()],
            max_metrics: 8,
            replay_identity: hash("replay"),
            budget: 10_000,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_and_a1_deterministic() {
        let manifest = capability_manifest();
        assert!(manifest.validate().is_ok());
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }

    #[test]
    fn passing_metrics_emit_witnesses_without_release_block() {
        let verdict = assure(&object(vec![
            metric("metric:a", MetricState::Supported, Some(900), false),
            metric("metric:b", MetricState::Supported, Some(800), true),
        ]))
        .unwrap();
        assert_eq!(verdict.disposition, QualityDisposition::Qualified);
        assert_eq!(verdict.passed_order.len(), 2);
        assert!(verdict
            .witness_order
            .iter()
            .any(|item| item.contains("pass")));
        assert!(verdict.effect_receipts.is_empty());
    }

    #[test]
    fn below_threshold_metric_is_failed_with_negative_evidence() {
        let verdict = assure(&object(vec![
            metric("metric:a", MetricState::Supported, Some(900), false),
            metric("metric:b", MetricState::Supported, Some(400), false),
        ]))
        .unwrap();
        assert_eq!(verdict.disposition, QualityDisposition::Conditional);
        assert!(verdict.failed_order.contains(&"metric:b".into()));
        assert!(verdict
            .negative_evidence
            .iter()
            .any(|item| item.contains("below-threshold")));
        assert_eq!(verdict.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn unknown_metric_is_retained_with_omission() {
        let verdict = assure(&object(vec![
            metric("metric:a", MetricState::Supported, Some(900), false),
            metric("metric:b", MetricState::Unknown, None, false),
        ]))
        .unwrap();
        assert_eq!(verdict.disposition, QualityDisposition::Conditional);
        assert!(verdict.unknown_order.contains(&"metric:b".into()));
        assert!(verdict
            .effect_receipts
            .contains(&"block:unsafe-release".into()));
    }

    #[test]
    fn policy_denial_blocks_without_quality_release() {
        let mut object = object(vec![
            metric("metric:a", MetricState::Supported, Some(900), false),
            metric("metric:b", MetricState::Supported, Some(800), false),
        ]);
        object.policy_allow = false;
        let verdict = assure(&object).unwrap();
        assert_eq!(verdict.disposition, QualityDisposition::Blocked);
        assert_eq!(verdict.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn duplicate_metrics_are_rejected() {
        let result = assure(&object(vec![
            metric("metric:a", MetricState::Supported, Some(900), false),
            metric("metric:a", MetricState::Supported, Some(900), false),
        ]));
        assert!(result.is_err());
    }
}
