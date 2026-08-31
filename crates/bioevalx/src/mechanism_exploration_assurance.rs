//! Federated continual mechanism-exploration assurance.
//!
//! Atlas feature: `AFA-bioevalx-P08-F28`.
//!
//! This module is a release-gate product surface rather than a mechanism scorer.  It
//! deterministically ranks typed mechanism candidates, keeps unsupported and contradictory
//! evidence visible, and refuses to qualify a portfolio when protected closure, provenance,
//! federation, or adversarial gates are incomplete.  Raw experimental payloads never leave the
//! institution; only digests and an auditable receipt are produced.

use bioprism_foundation::research::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ProvenanceLink, ResearchSurface, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioevalx-P08-F28";
pub const CONTRACT_VERSION: &str =
    "bioevalx-federated-continual-mechanism-exploration-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "MechanismPortfolio5@1";
pub const OUTPUT_SCHEMA: &str = "MechanismAssuranceReport8@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub candidate_id: String,
    pub mechanism_label: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub evidence_state: EvidenceState,
    pub support_score_milli: u16,
    pub novelty_score_milli: u16,
    pub artifact_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub baseline_digest: Option<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_data: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolioRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub candidates: Vec<MechanismCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub budget: u64,
    pub max_budget: u64,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismAssuranceReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub competing_explanation_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub checkpoint_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MechanismAssuranceError {
    #[error("invalid mechanism assurance request: {0}")]
    Invalid(String),
    #[error("mechanism assurance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> MechanismAssuranceError {
    MechanismAssuranceError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

pub fn mechanism_exploration_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "bioevalx".into(),
        consumers: [
            "imaging core scientist".into(),
            "mechanistic biology program lead".into(),
            "federated evaluation board".into(),
        ]
        .into(),
        behavior: "ranks and release-gates federated continual mechanism portfolios with typed evidence, provenance, replay, competing-explanation, locality, and adversarial witnesses".into(),
        value: "prevents unsupported mechanistic explanations from becoming qualified research conclusions while preserving contradiction, omission, negative, and unresolved evidence".into(),
        inputs: vec![TypedPort {
            name: "mechanism_portfolio".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "mechanism_assurance_report".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact]
            .into(),
        permissions: ["verify:bioevalx-mechanism-assurance".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference {
                source_id: "w3c-prov-o".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.w3.org/TR/prov-o/".into()),
            },
            EvidenceReference {
                source_id: "ro-crate".into(),
                state: EvidenceState::Supported,
                locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()),
            },
        ],
        authority_requirements: vec![AuthorityRequirement {
            role: "federated research release steward".into(),
            reason: "approval is required before an aggregate mechanism report crosses an institution boundary".into(),
        }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Protocol,
            ResearchSurface::Policy,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl MechanismAssuranceReport {
    pub fn validate(&self) -> Result<(), MechanismAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || !matches!(self.disposition.as_str(), "qualified" | "unresolved" | "blocked")
            || self.candidate_order.is_empty()
            || self.ranked_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
            || self.checkpoint_order.len() < 5
        {
            return Err(invalid("mechanism report identity, locality, partition, checkpoints, disposition, or effects are incomplete"));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.competing_explanation_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("mechanism report ordering is not canonical"));
            }
        }
        if self.checkpoint_order
            != [
                "admit-typed-portfolio",
                "check-evidence-and-baseline",
                "check-provenance-and-replay",
                "check-policy-and-federation",
                "retain-omission-and-negative-receipt",
            ]
        {
            return Err(invalid("mechanism report checkpoints are not canonical"));
        }
        let candidate_set = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let partitions = self
            .qualified_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if partitions.iter().any(|id| !candidate_set.contains(id))
            || partitions.len() != candidate_set.len()
            || partitions.iter().collect::<BTreeSet<_>>().len() != partitions.len()
            || self
                .missing_candidate_order
                .iter()
                .any(|id| candidate_set.contains(id))
        {
            return Err(invalid("mechanism candidate states do not partition observed and missing candidates"));
        }
        if self
            .ranked_order
            .iter()
            .collect::<BTreeSet<_>>()
            != candidate_set
        {
            return Err(invalid("mechanism ranking is not a candidate permutation"));
        }
        for value in [
            &self.replay_identity,
            &self.portfolio_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("mechanism report digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismAssuranceError::Artifact(error.to_string()))?;
        if self.artifact.content_type
            != "application/vnd.aurora.bioevalx-mechanism-assurance-report+json"
        {
            return Err(invalid("mechanism report artifact content type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("verify:bioevalx-mechanism-assurance:")
        {
                return Err(invalid("qualified mechanism report effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified mechanism report must block release"));
        }
        Ok(())
    }
}

pub fn assure_mechanism_portfolio(
    request: &MechanismPortfolioRequest,
) -> Result<MechanismAssuranceReport, MechanismAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_score_milli
            .cmp(&left.support_score_milli)
            .then(right.novelty_score_milli.cmp(&left.novelty_score_milli))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();

    let required_candidates = request
        .required_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let candidate_map = candidates
        .iter()
        .map(|candidate| (candidate.candidate_id.clone(), candidate))
        .collect::<std::collections::BTreeMap<_, _>>();
    let missing_candidate_order = required_candidates
        .iter()
        .filter(|id| !candidate_map.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut competing = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let required_studies = request
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();

    for candidate in &candidates {
        if candidate.negative_result {
            negative.insert(format!("{}:negative-result", candidate.candidate_id));
        }
        for omission in &candidate.omissions {
            omissions.insert(format!("{}:{omission}", candidate.candidate_id));
        }
        for item in &candidate.uncertainty {
            uncertainty.insert(format!("{}:{item}", candidate.candidate_id));
        }
        if candidate.evidence_state == EvidenceState::Contradicted {
            blocked.insert(candidate.candidate_id.clone());
            competing.insert(format!("{}:contradicted-evidence", candidate.candidate_id));
            continue;
        }
        if matches!(candidate.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative)
        {
            unresolved.insert(candidate.candidate_id.clone());
            uncertainty.insert(format!("{}:evidence-unresolved", candidate.candidate_id));
            continue;
        }
        let studies = candidate.study_order.iter().cloned().collect::<BTreeSet<_>>();
        let modalities = candidate.modality_order.iter().cloned().collect::<BTreeSet<_>>();
        let complete = candidate.local_data
            && candidate.permitted
            && candidate.provenance_digest.is_some()
            && candidate.baseline_digest.is_some()
            && candidate.replay_identity == request.replay_identity
            && candidate.semantic_profile == request.semantic_profile
            && required_studies.is_subset(&studies)
            && required_modalities.is_subset(&modalities)
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.support_score_milli >= 600;
        if complete && matches!(candidate.evidence_state, EvidenceState::Proven | EvidenceState::Supported)
        {
            qualified.insert(candidate.candidate_id.clone());
        } else {
            unresolved.insert(candidate.candidate_id.clone());
            if candidate.provenance_digest.is_none() || candidate.baseline_digest.is_none() {
                omissions.insert(format!("{}:typed-provenance-or-baseline-missing", candidate.candidate_id));
            }
            if !required_studies.is_subset(&studies) {
                omissions.insert(format!("{}:required-study-coverage-incomplete", candidate.candidate_id));
            }
            if !required_modalities.is_subset(&modalities) {
                omissions.insert(format!("{}:required-modality-coverage-incomplete", candidate.candidate_id));
            }
            if candidate.support_score_milli < 600 {
                uncertainty.insert(format!("{}:support-threshold-not-met", candidate.candidate_id));
            }
            if !candidate.local_data || !candidate.permitted {
                blocked.insert(candidate.candidate_id.clone());
                unresolved.remove(&candidate.candidate_id);
                omissions.insert(format!("{}:locality-or-permission-denied", candidate.candidate_id));
            }
        }
    }
    let missing_study_order = request
        .required_study_order
        .iter()
        .filter(|study| !candidates.iter().any(|candidate| candidate.study_order.contains(study)))
        .cloned()
        .collect::<Vec<_>>();
    let missing_modality_order = request
        .required_modality_order
        .iter()
        .filter(|modality| !candidates.iter().any(|candidate| candidate.modality_order.contains(modality)))
        .cloned()
        .collect::<Vec<_>>();
    for id in &missing_candidate_order {
        omissions.insert(format!("{id}:required-candidate-missing"));
    }
    for study in &missing_study_order {
        omissions.insert(format!("required-study-missing:{study}"));
    }
    for modality in &missing_modality_order {
        omissions.insert(format!("required-modality-missing:{modality}"));
    }
    for event in &request.adversarial_events {
        negative.insert(format!("adversarial:{event}"));
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.aggregate_only
        || !request.raw_data_local
        || !request.adversarial_events.is_empty()
        || request.budget > request.max_budget;
    let disposition = if global_block {
        "blocked"
    } else if missing_candidate_order.is_empty()
        && missing_study_order.is_empty()
        && missing_modality_order.is_empty()
        && !qualified.is_empty()
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        "qualified"
    } else {
        "unresolved"
    };
    if !request.policy_allow {
        uncertainty.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval || !request.federation_approved {
        uncertainty.insert("request:institutional-approval-incomplete".into());
    }
    if request.budget > request.max_budget {
        omissions.insert("request:budget-ceiling-exceeded".into());
    }
    let qualified_order = qualified.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let competing_explanation_order = competing.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let adversarial_event_order = request.adversarial_events.clone();
    let checkpoint_order = vec![
        "admit-typed-portfolio".into(),
        "check-evidence-and-baseline".into(),
        "check-provenance-and-replay".into(),
        "check-policy-and-federation".into(),
        "retain-omission-and-negative-receipt".into(),
    ];
    let effect_receipts = if disposition == "qualified" {
        vec![format!("verify:bioevalx-mechanism-assurance:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "ranked_order": ranked_order,
        "qualified_order": qualified_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_candidate_order": missing_candidate_order,
        "missing_study_order": missing_study_order,
        "missing_modality_order": missing_modality_order,
        "omission_order": omission_order,
        "uncertainty_order": uncertainty_order,
        "competing_explanation_order": competing_explanation_order,
        "negative_evidence_order": negative_evidence_order,
        "adversarial_event_order": adversarial_event_order,
        "checkpoint_order": checkpoint_order,
        "replay_identity": request.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "aggregate_only": request.aggregate_only,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let portfolio_digest = ContentHash::of_value(&payload)
        .map_err(|error| MechanismAssuranceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("bioevalx-mechanism-assurance:{}", request.request_id),
        "application/vnd.aurora.bioevalx-mechanism-assurance-report+json",
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: format!("federation:{}", request.federation_id),
            relation: "derived-from-local-aggregate-manifest".into(),
            digest: request.replay_identity.clone(),
        }],
    )
    .map_err(|error| MechanismAssuranceError::Artifact(error.to_string()))?;
    let report = MechanismAssuranceReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        candidate_order,
        ranked_order,
        qualified_order,
        unresolved_order,
        blocked_order,
        missing_candidate_order,
        missing_study_order,
        missing_modality_order,
        omission_order,
        uncertainty_order,
        competing_explanation_order,
        negative_evidence_order,
        adversarial_event_order,
        checkpoint_order,
        replay_identity: request.replay_identity.clone(),
        portfolio_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

fn validate_request(request: &MechanismPortfolioRequest) -> Result<(), MechanismAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.candidates.is_empty()
        || request.required_candidate_order.is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || !canonical(&request.required_candidate_order)
        || !canonical(&request.required_study_order)
        || !canonical(&request.required_modality_order)
        || !digest(&request.replay_identity)
        || request.budget == 0
        || request.max_budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !canonical(&request.adversarial_events)
    {
        return Err(invalid("mechanism portfolio identity, requirements, digest, budget, locality, or boundary is invalid"));
    }
    let mut seen = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_label.trim().is_empty()
            || !seen.insert(candidate.candidate_id.clone())
            || candidate.study_order.is_empty()
            || candidate.modality_order.is_empty()
            || !canonical(&candidate.study_order)
            || !canonical(&candidate.modality_order)
            || candidate.support_score_milli > 1000
            || candidate.novelty_score_milli > 1000
            || !digest(&candidate.artifact_digest)
            || candidate
                .provenance_digest
                .as_ref()
                .is_some_and(|value| !digest(value))
            || !digest(&candidate.replay_identity)
            || candidate.semantic_profile.trim().is_empty()
            || candidate
                .baseline_digest
                .as_ref()
                .is_some_and(|value| !digest(value))
            || !canonical(&candidate.omissions)
            || !canonical(&candidate.uncertainty)
        {
            return Err(invalid(format!("candidate {} is malformed or duplicated", candidate.candidate_id)));
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

    fn candidate(id: &str, state: EvidenceState, support: u16) -> MechanismCandidate {
        MechanismCandidate {
            candidate_id: id.into(),
            mechanism_label: format!("mechanism-{id}"),
            study_order: vec!["study-a".into(), "study-b".into()],
            modality_order: vec!["imaging".into(), "omics".into()],
            evidence_state: state,
            support_score_milli: support,
            novelty_score_milli: 800,
            artifact_digest: hash(&format!("artifact-{id}")),
            provenance_digest: Some(hash(&format!("provenance-{id}"))),
            replay_identity: hash("replay"),
            semantic_profile: "preclinical-neural-organoid".into(),
            baseline_digest: Some(hash("baseline")),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_data: true,
            permitted: true,
        }
    }

    fn request(candidates: Vec<MechanismCandidate>) -> MechanismPortfolioRequest {
        MechanismPortfolioRequest {
            request_id: "request-1".into(),
            federation_id: "consortium-1".into(),
            purpose: "mechanism-release".into(),
            semantic_profile: "preclinical-neural-organoid".into(),
            required_candidate_order: vec!["candidate-a".into()],
            required_study_order: vec!["study-a".into(), "study-b".into()],
            required_modality_order: vec!["imaging".into(), "omics".into()],
            candidates,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            budget: 4,
            max_budget: 8,
            adversarial_events: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2_and_typed() {
        let manifest = mechanism_exploration_assurance_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        manifest.validate().unwrap();
    }

    #[test]
    fn qualified_portfolio_is_ranked_and_replay_bound() {
        let report = assure_mechanism_portfolio(&request(vec![candidate(
            "candidate-a",
            EvidenceState::Supported,
            900,
        )]))
        .unwrap();
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.ranked_order, vec!["candidate-a"]);
        assert!(report.effect_receipts[0].starts_with("verify:bioevalx-mechanism-assurance:"));
        report.validate().unwrap();
    }

    #[test]
    fn missing_requirements_remain_unresolved() {
        let mut req = request(vec![candidate("candidate-a", EvidenceState::Supported, 900)]);
        req.required_modality_order.push("spatial".into());
        req.required_modality_order.sort();
        let report = assure_mechanism_portfolio(&req).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.omission_order.iter().any(|item| item.contains("required-modality")));
    }

    #[test]
    fn unknown_and_contradicted_evidence_is_never_a_pass() {
        let mut req = request(vec![
            candidate("candidate-a", EvidenceState::Unknown, 990),
            candidate("candidate-b", EvidenceState::Contradicted, 990),
        ]);
        req.required_candidate_order = vec!["candidate-a".into()];
        let report = assure_mechanism_portfolio(&req).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert!(report.unresolved_order.contains(&"candidate-a".into()));
        assert!(report.blocked_order.contains(&"candidate-b".into()));
    }

    #[test]
    fn policy_or_adversarial_event_blocks_release() {
        let mut req = request(vec![candidate("candidate-a", EvidenceState::Supported, 900)]);
        req.adversarial_events = vec!["poisoned-artifact".into()];
        let report = assure_mechanism_portfolio(&req).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn duplicate_candidate_is_rejected() {
        let req = request(vec![
            candidate("candidate-a", EvidenceState::Supported, 900),
            candidate("candidate-a", EvidenceState::Supported, 800),
        ]);
        assert!(matches!(
            assure_mechanism_portfolio(&req),
            Err(MechanismAssuranceError::Invalid(_))
        ));
    }

    #[test]
    fn ranking_is_deterministic_across_input_order() {
        let first = assure_mechanism_portfolio(&request(vec![
            candidate("candidate-b", EvidenceState::Supported, 700),
            candidate("candidate-a", EvidenceState::Supported, 900),
        ]))
        .unwrap();
        let second = assure_mechanism_portfolio(&request(vec![
            candidate("candidate-a", EvidenceState::Supported, 900),
            candidate("candidate-b", EvidenceState::Supported, 700),
        ]))
        .unwrap();
        assert_eq!(first.ranked_order, second.ranked_order);
        assert_eq!(first.portfolio_digest, second.portfolio_digest);
    }
}
