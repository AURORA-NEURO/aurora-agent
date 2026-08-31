//! Federated continual evidence-surveillance research copilot.
//!
//! Atlas feature: `AFA-safety-P01-F11`.
//!
//! This is an admission and evidence-state product surface. It ranks caller-supplied research
//! observations, retains stale/unknown/contradictory/negative evidence, and emits a digest-only
//! research receipt. It does not retrieve sources, infer biology, move raw data, or make clinical
//! decisions.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ProvenanceLink, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-safety-P01-F11";
pub const CONTRACT_VERSION: &str =
    "safety-federated-continual-evidence-surveillance-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceWatchRequest5@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObservation {
    pub evidence_id: String,
    pub source_id: String,
    pub title: String,
    pub scope: String,
    pub evidence_state: EvidenceState,
    pub relevance_milli: u16,
    pub freshness_epoch: u64,
    pub content_digest: ContentHash,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
    pub local_data: bool,
    pub permitted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWatchRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_evidence_order: Vec<String>,
    pub required_scope_order: Vec<String>,
    pub minimum_freshness_epoch: u64,
    pub observations: Vec<EvidenceObservation>,
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
pub struct QualifiedEvidenceSet {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub evidence_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_evidence_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub missing_scope_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub evidence_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence surveillance request: {0}")]
    Invalid(String),
    #[error("evidence surveillance artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> EvidenceSurveillanceError {
    EvidenceSurveillanceError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

pub fn evidence_surveillance_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "safety".into(),
        consumers: ["preclinical evidence lead".into(), "research program librarian".into(), "federation safety steward".into()].into(),
        behavior: "ranks and admits typed federated evidence observations with freshness, semantic, provenance, replay, policy, and safety witnesses".into(),
        value: "keeps evidence surveillance auditable and prevents stale or unsupported observations from being promoted into research context".into(),
        inputs: vec![TypedPort{name:"evidence_watch_request".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs: vec![TypedPort{name:"qualified_evidence_set".into(),schema:OUTPUT_SCHEMA.into(),required:true}],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions:["exchange:evidence-surveillance".into()].into(), determinism:Determinism::ByteStable,
        evidence: vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"ro-crate".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())}],
        authority_requirements: vec![AuthorityRequirement{role:"federation safety steward".into(),reason:"evidence aggregate exchange requires institutional approval".into()}], autonomy_tier:AutonomyTier::A2,
        surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(), boundary:PRECLINICAL_BOUNDARY.into()
    }
}

impl QualifiedEvidenceSet {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
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
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
            || self.evidence_order.is_empty()
            || self.ranked_order.len() != self.evidence_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "evidence set identity, locality, ranking, disposition, or effects are incomplete",
            ));
        }
        for values in [
            &self.evidence_order,
            &self.admitted_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_evidence_order,
            &self.stale_order,
            &self.missing_scope_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("evidence set ordering is not canonical"));
            }
        }
        let all = self.evidence_order.iter().collect::<BTreeSet<_>>();
        let partitions = self
            .admitted_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if partitions.iter().any(|id| !all.contains(id))
            || partitions.len() != all.len()
            || partitions.iter().collect::<BTreeSet<_>>().len() != partitions.len()
            || self
                .missing_evidence_order
                .iter()
                .any(|id| all.contains(id))
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != all
        {
            return Err(invalid("evidence states do not partition observations"));
        }
        for value in [
            &self.replay_identity,
            &self.evidence_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("evidence set digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.artifact.content_type != "application/vnd.aurora.qualified-evidence-set+json" {
            return Err(invalid("evidence set artifact type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("exchange:evidence-surveillance:")
            {
                return Err(invalid("qualified evidence exchange effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid(
                "non-qualified evidence exchange must block release",
            ));
        }
        Ok(())
    }
}

pub fn assure_evidence_surveillance(
    request: &EvidenceWatchRequest,
) -> Result<QualifiedEvidenceSet, EvidenceSurveillanceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| {
        b.relevance_milli
            .cmp(&a.relevance_milli)
            .then(b.freshness_epoch.cmp(&a.freshness_epoch))
            .then(a.evidence_id.cmp(&b.evidence_id))
    });
    let ranked_order = observations
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut evidence_order = ranked_order.clone();
    evidence_order.sort();
    let required = request
        .required_evidence_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let map = observations
        .iter()
        .map(|item| (item.evidence_id.clone(), item))
        .collect::<std::collections::BTreeMap<_, _>>();
    let missing_evidence_order = required
        .iter()
        .filter(|id| !map.contains_key(*id))
        .cloned()
        .collect::<Vec<_>>();
    let scopes = request
        .required_scope_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut missing_scope = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for item in &observations {
        let id = &item.evidence_id;
        if item.negative_result {
            negative.insert(format!("{id}:negative-result"));
        }
        omissions.extend(item.omissions.iter().map(|x| format!("{id}:{x}")));
        uncertainty.extend(item.uncertainty.iter().map(|x| format!("{id}:{x}")));
        if item.freshness_epoch < request.minimum_freshness_epoch {
            stale.insert(id.clone());
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:stale-evidence"));
            continue;
        }
        if item.evidence_state == EvidenceState::Contradicted {
            blocked.insert(id.clone());
            contradiction.insert(format!("{id}:contradicted-evidence"));
            continue;
        }
        if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-unresolved"));
            continue;
        }
        let complete = item.title.trim() != ""
            && item.source_id.trim() != ""
            && item.semantic_profile == request.semantic_profile
            && item.replay_identity == request.replay_identity
            && item.provenance_digest.is_some()
            && digest(&item.content_digest)
            && scopes.contains(&item.scope)
            && item.omissions.is_empty()
            && item.uncertainty.is_empty()
            && item.local_data
            && item.permitted
            && item.relevance_milli >= 600;
        if complete
            && matches!(
                item.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            admitted.insert(id.clone());
        } else {
            unresolved.insert(id.clone());
            if item.provenance_digest.is_none() {
                omissions.insert(format!("{id}:provenance-missing"));
            }
            if !scopes.contains(&item.scope) {
                missing_scope.insert(item.scope.clone());
                omissions.insert(format!("{id}:required-scope-missing"));
            }
            if item.relevance_milli < 600 {
                uncertainty.insert(format!("{id}:relevance-threshold-not-met"));
            }
            if !item.local_data || !item.permitted {
                blocked.insert(id.clone());
                unresolved.remove(id);
                omissions.insert(format!("{id}:locality-or-permission-denied"));
            }
        }
    }
    for id in &missing_evidence_order {
        omissions.insert(format!("{id}:required-evidence-missing"));
    }
    for scope in &missing_scope {
        omissions.insert(format!("required-scope-missing:{scope}"));
    }
    negative.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("adversarial:{event}")),
    );
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_approved
        || !request.raw_data_local
        || !request.aggregate_only
        || request.budget > request.max_budget
        || !request.adversarial_events.is_empty();
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
    let disposition = if global_block {
        "blocked"
    } else if missing_evidence_order.is_empty()
        && missing_scope.is_empty()
        && !admitted.is_empty()
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        "qualified"
    } else {
        "unresolved"
    };
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let stale_order = stale.into_iter().collect::<Vec<_>>();
    let missing_scope_order = missing_scope.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let contradiction_order = contradiction.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let adversarial_event_order = request.adversarial_events.clone();
    let effect_receipts = if disposition == "qualified" {
        vec![format!(
            "exchange:evidence-surveillance:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"federation_id":request.federation_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"evidence_order":evidence_order,"ranked_order":ranked_order,"admitted_order":admitted_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_evidence_order":missing_evidence_order,"stale_order":stale_order,"missing_scope_order":missing_scope_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"contradiction_order":contradiction_order,"negative_evidence_order":negative_evidence_order,"adversarial_event_order":adversarial_event_order,"replay_identity":request.replay_identity,"effect_receipts":effect_receipts,"raw_data_local":request.raw_data_local,"aggregate_only":request.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let evidence_digest = ContentHash::of_value(&payload)
        .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("safety-evidence:{}", request.request_id),
        "application/vnd.aurora.qualified-evidence-set+json",
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: format!("federation:{}", request.federation_id),
            relation: "derived-from-local-evidence-manifest".into(),
            digest: request.replay_identity.clone(),
        }],
    )
    .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let set = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        evidence_order,
        ranked_order,
        admitted_order,
        unresolved_order,
        blocked_order,
        missing_evidence_order,
        stale_order,
        missing_scope_order,
        omission_order,
        uncertainty_order,
        contradiction_order,
        negative_evidence_order,
        adversarial_event_order,
        replay_identity: request.replay_identity.clone(),
        evidence_digest,
        artifact,
        effect_receipts,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    set.validate()?;
    Ok(set)
}

fn validate_request(request: &EvidenceWatchRequest) -> Result<(), EvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_evidence_order.is_empty()
        || request.required_scope_order.is_empty()
        || request.observations.is_empty()
        || !canonical(&request.required_evidence_order)
        || !canonical(&request.required_scope_order)
        || !canonical(&request.adversarial_events)
        || !digest(&request.replay_identity)
        || request.budget == 0
        || request.max_budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(invalid("evidence watch identity, requirements, digest, budget, locality, or boundary is invalid"));
    }
    let mut seen = BTreeSet::new();
    for item in &request.observations {
        if item.evidence_id.trim().is_empty()
            || item.source_id.trim().is_empty()
            || item.title.trim().is_empty()
            || !seen.insert(item.evidence_id.clone())
            || item.scope.trim().is_empty()
            || item.relevance_milli > 1000
            || !digest(&item.content_digest)
            || item.provenance_digest.as_ref().is_some_and(|x| !digest(x))
            || !digest(&item.replay_identity)
            || item.semantic_profile.trim().is_empty()
            || !canonical(&item.omissions)
            || !canonical(&item.uncertainty)
        {
            return Err(invalid(format!(
                "observation {} is malformed or duplicated",
                item.evidence_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(x: &str) -> ContentHash {
        ContentHash::of_bytes(x.as_bytes())
    }
    fn obs(id: &str, state: EvidenceState, rel: u16) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: id.into(),
            source_id: format!("source-{id}"),
            title: format!("title-{id}"),
            scope: "organoid-study".into(),
            evidence_state: state,
            relevance_milli: rel,
            freshness_epoch: 10,
            content_digest: hash(&format!("content-{id}")),
            provenance_digest: Some(hash(&format!("provenance-{id}"))),
            replay_identity: hash("replay"),
            semantic_profile: "preclinical-neural-organoid".into(),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_data: true,
            permitted: true,
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> EvidenceWatchRequest {
        EvidenceWatchRequest {
            request_id: "request-1".into(),
            federation_id: "federation-1".into(),
            purpose: "evidence-watch".into(),
            semantic_profile: "preclinical-neural-organoid".into(),
            required_evidence_order: vec!["evidence-a".into()],
            required_scope_order: vec!["organoid-study".into()],
            minimum_freshness_epoch: 5,
            observations,
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
    fn manifest_is_a2() {
        let m = evidence_surveillance_copilot_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
        m.validate().unwrap();
    }
    #[test]
    fn supported_is_admitted() {
        let s = assure_evidence_surveillance(&request(vec![obs(
            "evidence-a",
            EvidenceState::Supported,
            900,
        )]))
        .unwrap();
        assert_eq!(s.disposition, "qualified");
        s.validate().unwrap();
    }
    #[test]
    fn stale_is_unresolved() {
        let mut q = request(vec![obs("evidence-a", EvidenceState::Supported, 900)]);
        q.minimum_freshness_epoch = 20;
        let s = assure_evidence_surveillance(&q).unwrap();
        assert_eq!(s.disposition, "unresolved");
        assert!(s.stale_order.contains(&"evidence-a".into()));
    }
    #[test]
    fn unknown_and_contradiction_retained() {
        let mut q = request(vec![
            obs("evidence-a", EvidenceState::Unknown, 900),
            obs("evidence-b", EvidenceState::Contradicted, 900),
        ]);
        q.required_evidence_order = vec!["evidence-a".into()];
        let s = assure_evidence_surveillance(&q).unwrap();
        assert!(s.unresolved_order.contains(&"evidence-a".into()));
        assert!(s.blocked_order.contains(&"evidence-b".into()));
    }
    #[test]
    fn adversarial_blocks() {
        let mut q = request(vec![obs("evidence-a", EvidenceState::Supported, 900)]);
        q.adversarial_events = vec!["poisoned-source".into()];
        let s = assure_evidence_surveillance(&q).unwrap();
        assert_eq!(s.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_rejected() {
        let q = request(vec![
            obs("evidence-a", EvidenceState::Supported, 900),
            obs("evidence-a", EvidenceState::Supported, 800),
        ]);
        assert!(matches!(
            assure_evidence_surveillance(&q),
            Err(EvidenceSurveillanceError::Invalid(_))
        ));
    }
    #[test]
    fn ranking_deterministic() {
        let a = assure_evidence_surveillance(&request(vec![
            obs("evidence-b", EvidenceState::Supported, 700),
            obs("evidence-a", EvidenceState::Supported, 900),
        ]))
        .unwrap();
        let b = assure_evidence_surveillance(&request(vec![
            obs("evidence-a", EvidenceState::Supported, 900),
            obs("evidence-b", EvidenceState::Supported, 700),
        ]))
        .unwrap();
        assert_eq!(a.ranked_order, b.ranked_order);
        assert_eq!(a.evidence_digest, b.evidence_digest);
    }
}
