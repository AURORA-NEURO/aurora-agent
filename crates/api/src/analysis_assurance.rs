//! Federated continual statistical, causal, and ML analysis assurance.
//!
//! Atlas feature: `AFA-api-P13-F28`.
//!
//! This API-owned product boundary turns analysis candidates into a qualified result only when
//! the candidate, evidence, comparability, provenance, policy, federation, replay, and authority
//! gates are closed.  It is intentionally an assurance surface rather than an analysis engine:
//! model execution stays behind typed digests and institution-local data boundaries.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const ANALYSIS_ASSURANCE_FEATURE_ID: &str = "AFA-api-P13-F28";
pub const ANALYSIS_ASSURANCE_CONTRACT_VERSION: &str = "api-federated-analysis-assurance/1.0";
pub const ANALYSIS_ASSURANCE_SCHEMA_VERSION: &str = RESEARCH_CONTRACT_SCHEMA_VERSION;
pub const ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const MAX_ANALYSIS_CANDIDATES: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub candidate_id: String,
    pub analysis_class: String,
    pub site_id: String,
    pub scope: String,
    pub estimand: String,
    pub result_digest: ContentHash,
    pub model_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub influence_digest: Option<ContentHash>,
    pub state: AnalysisEvidenceState,
    pub quality_score: u16,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub question_id: String,
    pub scope: String,
    pub estimand: String,
    pub required_analysis_classes: Vec<String>,
    pub minimum_quality_score: u16,
    pub candidates: Vec<AnalysisCandidate>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub evidence_receipt_digest: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult {
    pub result_id: String,
    pub question_id: String,
    pub estimand: String,
    pub disposition: AnalysisDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub selected_candidate: Option<String>,
    pub class_order: Vec<String>,
    pub result_order: Vec<ContentHash>,
    pub model_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub benchmark_digest: ContentHash,
    pub evidence_receipt_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub question_id: String,
    pub disposition: AnalysisDisposition,
    pub result: QualifiedAnalysisResult,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalysisAssuranceError {
    #[error("invalid analysis assurance request: {0}")]
    Invalid(String),
    #[error("analysis assurance artifact failed: {0}")]
    Artifact(String),
    #[error("analysis assurance serialization failed: {0}")]
    Serialization(String),
}

impl AnalysisAssuranceReceipt {
    pub fn validate(&self) -> Result<(), AnalysisAssuranceError> {
        if self.schema_version != ANALYSIS_ASSURANCE_SCHEMA_VERSION
            || self.contract_version != ANALYSIS_ASSURANCE_CONTRACT_VERSION
            || self.feature_id != ANALYSIS_ASSURANCE_FEATURE_ID
            || self.boundary != ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.question_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.result.boundary != ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY
            || self.result.question_id != self.question_id
            || self.result.artifact.boundary != ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY
        {
            return Err(AnalysisAssuranceError::Invalid(
                "analysis assurance identity, result, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.result.candidate_order,
            &self.result.blocked_order,
            &self.result.class_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.checks,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AnalysisAssuranceError::Invalid(
                    "analysis assurance ordering is not canonical".into(),
                ));
            }
        }
        if self.result.admitted_order.len()
            != self
                .result
                .admitted_order
                .iter()
                .collect::<BTreeSet<_>>()
                .len()
            || self
                .result
                .admitted_order
                .iter()
                .any(|id| !self.result.candidate_order.contains(id))
        {
            return Err(AnalysisAssuranceError::Invalid(
                "analysis assurance admitted candidates are not covered by the candidate order"
                    .into(),
            ));
        }
        for values in [
            &self.result.result_order,
            &self.result.model_order,
            &self.result.evidence_order,
            &self.result.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AnalysisAssuranceError::Invalid(
                    "analysis assurance digest ordering is not canonical".into(),
                ));
            }
        }
        self.result
            .artifact
            .validate_metadata()
            .map_err(|error| AnalysisAssuranceError::Artifact(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, AnalysisAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| AnalysisAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| AnalysisAssuranceError::Serialization(error.to_string()))
    }
}

pub fn analysis_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: ANALYSIS_ASSURANCE_SCHEMA_VERSION.into(),
        capability_id: ANALYSIS_ASSURANCE_FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "api".into(),
        consumers: [
            "integration engineer".into(),
            "analysis platform operator".into(),
            "independent validation partner".into(),
        ]
        .into(),
        behavior: "assures federated statistical, causal, and ML analysis candidates and emits a qualified result only when evidence, comparability, provenance, replay, policy, authority, and locality gates close".into(),
        value: "makes API-level analysis claims independently auditable without moving raw preclinical data or converting missing evidence into confidence".into(),
        inputs: vec![TypedPort {
            name: "analysis_assurance_request".into(),
            schema: "AnalysisAssuranceRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "analysis_assurance_receipt".into(),
            schema: "AnalysisAssuranceReceipt@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::FederationExport].into(),
        permissions: [
            "read:analysis-evidence".into(),
            "evaluate:analysis-candidates".into(),
            "exchange:digest-only-analysis-assurance".into(),
        ]
        .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification/1.3/".into()),
            },
            EvidenceReference {
                source_id: "ga4gh-drs-1.3".into(),
                state: EvidenceState::Supported,
                locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "analysis release steward".into(),
            reason: "approve a qualified federated analytical result and its digest-only exchange".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_analysis(
    request: &AnalysisAssuranceRequest,
) -> Result<AnalysisAssuranceReceipt, AnalysisAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let required_classes = request
        .required_analysis_classes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidate_order = BTreeSet::new();
    let mut admitted = Vec::<(String, u16)>::new();
    let mut blocked = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        candidate_order.insert(candidate.candidate_id.clone());
        let cost = candidate.candidate_id.len() as u64 + candidate.analysis_class.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = candidate.comparability_digest.is_some()
            && candidate.influence_digest.is_some()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.scope == request.scope
            && candidate.estimand == request.estimand
            && candidate.quality_score >= request.minimum_quality_score;
        let gate = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && candidate.state == AnalysisEvidenceState::Supported
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            admitted.push((candidate.candidate_id.clone(), candidate.quality_score));
            classes.insert(candidate.analysis_class.clone());
            results.insert(candidate.result_digest.clone());
            models.insert(candidate.model_digest.clone());
            evidence.insert(candidate.evidence_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
        } else {
            blocked.insert(candidate.candidate_id.clone());
            if candidate.state != AnalysisEvidenceState::Supported {
                negative.insert(
                    format!(
                        "candidate:{}:state-{:?}-not-qualified",
                        candidate.candidate_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.comparability_digest.is_none() {
                omissions.insert(format!(
                    "candidate:{}:cross-study-comparability-missing",
                    candidate.candidate_id
                ));
            }
            if candidate.influence_digest.is_none() {
                omissions.insert(format!(
                    "candidate:{}:influence-evidence-missing",
                    candidate.candidate_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "candidate:{}:scope-mismatch",
                    candidate.candidate_id
                ));
            }
            if candidate.estimand != request.estimand {
                omissions.insert(format!(
                    "candidate:{}:estimand-mismatch",
                    candidate.candidate_id
                ));
            }
            if candidate.quality_score < request.minimum_quality_score {
                uncertainty.insert(format!(
                    "candidate:{}:quality-below-floor",
                    candidate.candidate_id
                ));
            }
            if !candidate.omissions.is_empty() || !candidate.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "candidate:{}:protected-closure-or-evidence-incomplete",
                    candidate.candidate_id
                ));
            }
            if !candidate.negative_evidence.is_empty() {
                negative.extend(candidate.negative_evidence.iter().cloned());
            }
            if !budget_ok {
                omissions.insert(format!(
                    "candidate:{}:budget-ceiling-exceeded",
                    candidate.candidate_id
                ));
            }
        }
    }
    for required_class in required_classes {
        if !classes.contains(&required_class) {
            omissions.insert(format!(
                "analysis-class:{required_class}:required-but-not-qualified"
            ));
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
    admitted.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let admitted_order = admitted
        .iter()
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();
    let selected_candidate = admitted_order.first().cloned();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.federation_allow || !request.signed_approval {
            AnalysisDisposition::Blocked
        } else if admitted_order.is_empty() || !request.protected_closure {
            AnalysisDisposition::Unknown
        } else if blocked_order.is_empty() && omissions.is_empty() && uncertainty.is_empty() {
            AnalysisDisposition::Qualified
        } else {
            AnalysisDisposition::Partial
        };
    let candidate_order = candidate_order.into_iter().collect::<Vec<_>>();
    let class_order = classes.into_iter().collect::<Vec<_>>();
    let result_order = results.into_iter().collect::<Vec<_>>();
    let model_order = models.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let artifact_payload = json!({
        "feature_id": ANALYSIS_ASSURANCE_FEATURE_ID,
        "question_id": request.question_id,
        "estimand": request.estimand,
        "disposition": disposition,
        "selected_candidate": selected_candidate,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "result_order": result_order,
        "model_order": model_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "replay_identity": request.replay_identity,
        "benchmark_digest": request.benchmark_digest,
        "evidence_receipt_digest": request.evidence_receipt_digest,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("qualified-analysis:{}", request.question_id),
        "application/vnd.aurora.qualified-analysis+json",
        &artifact_payload,
        vec![],
        vec![],
    )
    .map_err(|error| AnalysisAssuranceError::Artifact(error.to_string()))?;
    let result_id = format!("qualified-analysis:{}", request.request_id);
    let result = QualifiedAnalysisResult {
        result_id,
        question_id: request.question_id.clone(),
        estimand: request.estimand.clone(),
        disposition,
        candidate_order,
        admitted_order,
        blocked_order,
        selected_candidate,
        class_order,
        result_order,
        model_order,
        evidence_order,
        provenance_order,
        replay_identity: request.replay_identity.clone(),
        benchmark_digest: request.benchmark_digest.clone(),
        evidence_receipt_digest: request.evidence_receipt_digest.clone(),
        artifact,
        boundary: ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY.into(),
    };
    let mut checks = vec![
        "candidate ordering, score ranking, and digest ordering are canonical".into(),
        "comparability, influence, evidence, provenance, replay, benchmark, policy, federation, authority, locality, and budget gates are explicit".into(),
        "contradicted, unknown, unmeasured, omitted, and negative analysis states remain researcher-visible".into(),
        "digest-only federated assurance never exports raw research data".into(),
    ];
    checks.sort();
    let effect_receipts = if matches!(
        disposition,
        AnalysisDisposition::Qualified | AnalysisDisposition::Partial
    ) {
        vec![format!(
            "exchange:digest-only-analysis-assurance:{}",
            request.request_id
        )]
    } else {
        vec![format!("block:unsafe-release:{}", request.request_id)]
    };
    let receipt = AnalysisAssuranceReceipt {
        schema_version: ANALYSIS_ASSURANCE_SCHEMA_VERSION.into(),
        contract_version: ANALYSIS_ASSURANCE_CONTRACT_VERSION.into(),
        feature_id: ANALYSIS_ASSURANCE_FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        question_id: request.question_id.clone(),
        disposition,
        result,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &AnalysisAssuranceRequest) -> Result<(), AnalysisAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.question_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.estimand.trim().is_empty()
        || request.required_analysis_classes.is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_ANALYSIS_CANDIDATES
        || request.budget == 0
        || request.boundary != ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY
        || request
            .required_analysis_classes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(AnalysisAssuranceError::Invalid(
            "analysis assurance identity, scope, estimand, classes, candidates, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.analysis_class.trim().is_empty()
            || candidate.site_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.estimand.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.boundary != ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY
            || candidate
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(AnalysisAssuranceError::Invalid(format!(
                "analysis candidate {} is invalid or duplicated",
                candidate.candidate_id
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
        class: &str,
        state: AnalysisEvidenceState,
        score: u16,
    ) -> AnalysisCandidate {
        AnalysisCandidate {
            candidate_id: id.into(),
            analysis_class: class.into(),
            site_id: "site:alpha".into(),
            scope: "organoid:neural".into(),
            estimand: "synaptic-density-delta".into(),
            result_digest: hash(&format!("result:{id}")),
            model_digest: hash(&format!("model:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash("comparability")),
            influence_digest: Some(hash("influence")),
            state,
            quality_score: score,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<AnalysisCandidate>) -> AnalysisAssuranceRequest {
        AnalysisAssuranceRequest {
            request_id: "analysis:assurance".into(),
            workflow_id: "workflow:analysis".into(),
            question_id: "question:organoid".into(),
            scope: "organoid:neural".into(),
            estimand: "synaptic-density-delta".into(),
            required_analysis_classes: vec!["causal".into(), "statistical".into()],
            minimum_quality_score: 70,
            candidates,
            replay_identity: hash("replay"),
            benchmark_digest: hash("benchmark"),
            evidence_receipt_digest: hash("evidence-receipt"),
            budget: 200,
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: ANALYSIS_ASSURANCE_PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_declares_byte_stable_a1_assurance() {
        let manifest = analysis_assurance_manifest();
        assert_eq!(manifest.capability_id, ANALYSIS_ASSURANCE_FEATURE_ID);
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
    }

    #[test]
    fn qualifies_supported_candidates_by_score_and_id() {
        let receipt = assure_analysis(&request(vec![
            candidate(
                "candidate:b",
                "causal",
                AnalysisEvidenceState::Supported,
                80,
            ),
            candidate(
                "candidate:a",
                "statistical",
                AnalysisEvidenceState::Supported,
                90,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Qualified);
        assert_eq!(
            receipt.result.selected_candidate.as_deref(),
            Some("candidate:a")
        );
        assert_eq!(
            receipt.result.admitted_order,
            vec!["candidate:a", "candidate:b"]
        );
    }

    #[test]
    fn contradiction_and_missing_influence_are_retained() {
        let mut missing = candidate(
            "candidate:a",
            "causal",
            AnalysisEvidenceState::Supported,
            90,
        );
        missing.influence_digest = None;
        let receipt = assure_analysis(&request(vec![
            candidate(
                "candidate:b",
                "statistical",
                AnalysisEvidenceState::Contradicted,
                90,
            ),
            missing,
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Unknown);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("influence")));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }

    #[test]
    fn policy_denial_is_fail_closed() {
        let mut input = request(vec![candidate(
            "candidate:a",
            "causal",
            AnalysisEvidenceState::Supported,
            90,
        )]);
        input.policy_allow = false;
        let receipt = assure_analysis(&input).unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Blocked);
        assert!(receipt.effect_receipts[0].starts_with("block:unsafe-release:"));
    }

    #[test]
    fn protected_closure_and_required_class_are_explicit() {
        let mut input = request(vec![candidate(
            "candidate:a",
            "causal",
            AnalysisEvidenceState::Supported,
            90,
        )]);
        input.protected_closure = false;
        let receipt = assure_analysis(&input).unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("protected-closure")));
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("statistical")));
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = assure_analysis(&request(vec![
            candidate(
                "candidate:a",
                "causal",
                AnalysisEvidenceState::Supported,
                90,
            ),
            candidate(
                "candidate:a",
                "statistical",
                AnalysisEvidenceState::Supported,
                80,
            ),
        ]));
        assert!(result.is_err());
    }
}
