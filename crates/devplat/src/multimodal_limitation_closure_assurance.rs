//! Multimodal multi-study Devplat limitation-closure assurance harness.
//!
//! Atlas feature: `AFA-devplat-P26-F26`.  The fabric turns caller-declared limitations and closure
//! evidence into an auditable workflow receipt.  It does not claim that a limitation is resolved
//! merely because a plan exists, and it never executes a workflow or makes a clinical decision.

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

pub const FEATURE_ID: &str = "AFA-devplat-P26-F26";
pub const CONTRACT_VERSION: &str = "devplat-multimodal-limitation-closure-assurance/1.0";
pub const INPUT_SCHEMA: &str = "DevplatLimitationCase2@1";
pub const OUTPUT_SCHEMA: &str = "DevplatClosureReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.devplat-closure-receipt-7+json";
pub const MAX_LIMITATIONS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limitation6 {
    pub limitation_id: String,
    pub workflow_id: String,
    pub site_id: String,
    pub claim_scope: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub closure_criteria_order: Vec<String>,
    pub satisfied_criteria_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_attestation: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevplatLimitationCase2 {
    pub schema_version: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_limitation_order: Vec<String>,
    pub required_workflow_order: Vec<String>,
    pub required_site_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_limitation_count: u32,
    pub minimum_workflow_count: u32,
    pub minimum_site_count: u32,
    pub max_limitations: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub limitations: Vec<Limitation6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitationClosureDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevplatClosureReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: LimitationClosureDisposition,
    pub ranked_limitation_order: Vec<String>,
    pub selected_limitation_order: Vec<String>,
    pub unresolved_limitation_order: Vec<String>,
    pub blocked_limitation_order: Vec<String>,
    pub missing_limitation_order: Vec<String>,
    pub workflow_order: Vec<String>,
    pub selected_workflow_order: Vec<String>,
    pub unresolved_workflow_order: Vec<String>,
    pub blocked_workflow_order: Vec<String>,
    pub missing_workflow_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub closure_criteria_order: Vec<String>,
    pub satisfied_criteria_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub closure_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DevplatClosureError {
    #[error("invalid limitation-closure workflow request or receipt: {0}")]
    Invalid(String),
    #[error("limitation-closure workflow artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> DevplatClosureError {
    DevplatClosureError::Invalid(message.into())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.to_string().len() == 64
}
fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), DevplatClosureError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flat = Vec::new();
    for part in parts {
        if !ordered(part) || part.iter().any(|id| !expected.contains(id)) {
            return Err(invalid(format!("{label} state is not canonical")));
        }
        flat.extend_from_slice(part);
    }
    if flat.len() != expected.len() || flat.iter().collect::<BTreeSet<_>>().len() != flat.len() {
        return Err(invalid(format!(
            "{label} states do not form a complete partition"
        )));
    }
    Ok(())
}

impl DevplatClosureReceipt7 {
    pub fn validate(&self) -> Result<(), DevplatClosureError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A1
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_limitation_order.is_empty()
            || self.workflow_order.is_empty()
            || self.site_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("limitation-closure identity, closure, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_limitation_order,
            &self.selected_limitation_order,
            &self.unresolved_limitation_order,
            &self.blocked_limitation_order,
            &self.missing_limitation_order,
            &self.workflow_order,
            &self.selected_workflow_order,
            &self.unresolved_workflow_order,
            &self.blocked_workflow_order,
            &self.missing_workflow_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.closure_criteria_order,
            &self.satisfied_criteria_order,
            &self.counterexample_order,
            &self.uncertainty_order,
            &self.omission_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !ordered(values) {
                return Err(invalid("limitation-closure ordering is not canonical"));
            }
        }
        let mut lim = self.ranked_limitation_order.clone();
        lim.extend(self.missing_limitation_order.iter().cloned());
        lim.sort();
        partition(
            &lim,
            &[
                &self.selected_limitation_order,
                &self.unresolved_limitation_order,
                &self.blocked_limitation_order,
                &self.missing_limitation_order,
            ],
            "limitation",
        )?;
        partition(
            &self.workflow_order,
            &[
                &self.selected_workflow_order,
                &self.unresolved_workflow_order,
                &self.blocked_workflow_order,
                &self.missing_workflow_order,
            ],
            "workflow",
        )?;
        partition(
            &self.site_order,
            &[
                &self.selected_site_order,
                &self.unresolved_site_order,
                &self.blocked_site_order,
                &self.missing_site_order,
            ],
            "site",
        )?;
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.closure_digest)
            || self.artifact.content_hash != self.closure_digest
        {
            return Err(invalid("limitation-closure digest is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("close:declared-limitation:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("limitation-closure effect is outside bounded gate"));
        }
        if self.disposition == LimitationClosureDisposition::Qualified
            && self.effect_receipts
                != vec![format!("close:declared-limitation:{}", self.request_id)]
        {
            return Err(invalid("qualified limitation-closure effect is invalid"));
        }
        if self.disposition != LimitationClosureDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified limitation-closure must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| DevplatClosureError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, DevplatClosureError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| DevplatClosureError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| DevplatClosureError::Artifact(e.to_string()))
            })
    }
}

pub fn devplat_multimodal_limitation_closure_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"devplat".into(),consumers:["research program lead".into(),"routing operator".into(),"independent evaluator".into()].into(),behavior:"compiles typed limitation and closure attestations into a deterministic workflow receipt without claiming unresolved constraints are closed".into(),value:"makes limitations, counterexamples, omission, provenance, replay, policy, and release gates auditable before a high-throughput research route proceeds".into(),inputs:vec![TypedPort{name:"limitation_closure_workflow_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"limitation_closure_workflow_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["close:declared-limitation".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:vec![AuthorityRequirement{role:"research program lead".into(),reason:"limitation closure changes release posture and requires explicit authority".into()}],autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

fn validate_request(q: &DevplatLimitationCase2) -> Result<(), DevplatClosureError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.requester.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.semantic_profile.trim().is_empty()
        || q.required_limitation_order.is_empty()
        || q.required_workflow_order.is_empty()
        || q.required_site_order.is_empty()
        || !ordered(&q.required_limitation_order)
        || !ordered(&q.required_workflow_order)
        || !ordered(&q.required_site_order)
        || !ordered(&q.adversarial_event_order)
        || q.minimum_limitation_count == 0
        || q.minimum_workflow_count == 0
        || q.minimum_site_count == 0
        || q.max_limitations == 0
        || q.max_limitations as usize > MAX_LIMITATIONS
        || !digest_valid(&q.replay_identity)
        || !q.policy_allow
        || !q.protected_closure
        || !q.signed_approval
        || !q.raw_data_local
        || !q.aggregate_only
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.limitations.is_empty()
        || q.limitations.len() > MAX_LIMITATIONS
    {
        return Err(invalid("limitation-closure identity, closure, policy, capacity, replay, locality, or bounds are invalid"));
    }
    let mut seen = BTreeSet::new();
    for x in &q.limitations {
        if x.limitation_id.trim().is_empty()
            || x.workflow_id.trim().is_empty()
            || x.site_id.trim().is_empty()
            || x.claim_scope.trim().is_empty()
            || x.semantic_profile != q.semantic_profile
            || !digest_valid(&x.evidence_digest)
            || !digest_valid(&x.provenance_digest)
            || !digest_valid(&x.replay_identity)
            || !ordered(&x.closure_criteria_order)
            || !ordered(&x.satisfied_criteria_order)
            || !ordered(&x.counterexample_order)
            || !ordered(&x.uncertainty_order)
            || !ordered(&x.omission_order)
            || !seen.insert(x.limitation_id.clone())
        {
            return Err(invalid(
                "limitation identity, profile, digest, criteria, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn assure_devplat_multimodal_limitation_closure(
    q: &DevplatLimitationCase2,
) -> Result<DevplatClosureReceipt7, DevplatClosureError> {
    validate_request(q)?;
    let mut rows = q.limitations.clone();
    let rank = |s: EvidenceState| match s {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    };
    rows.sort_by(|a, b| {
        (rank(a.evidence_state), a.stale, a.limitation_id.as_str()).cmp(&(
            rank(b.evidence_state),
            b.stale,
            b.limitation_id.as_str(),
        ))
    });
    let ranked = rows
        .iter()
        .map(|x| x.limitation_id.clone())
        .collect::<Vec<_>>();
    let required = q
        .required_limitation_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut criteria = BTreeSet::new();
    let mut satisfied = BTreeSet::new();
    let mut counterexamples = BTreeSet::new();
    for row in &rows {
        criteria.extend(row.closure_criteria_order.iter().cloned());
        satisfied.extend(row.satisfied_criteria_order.iter().cloned());
        counterexamples.extend(row.counterexample_order.iter().cloned());
        uncertainty.extend(row.uncertainty_order.iter().cloned());
        omission.extend(row.omission_order.iter().cloned());
        if row.negative_result {
            negative.insert(row.limitation_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.limitation_id.clone());
        }
        let hard = !row.policy_allowed
            || !row.protected_closure
            || !row.signed_attestation
            || !row.raw_data_local
            || !row.aggregate_only
            || row.revoked;
        let unresolved_state = !row
            .closure_criteria_order
            .iter()
            .all(|id| row.satisfied_criteria_order.contains(id));
        let soft = row.stale
            || row.replay_identity != q.replay_identity
            || !row.uncertainty_order.is_empty()
            || !row.omission_order.is_empty()
            || unresolved_state
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.limitation_id.clone());
        } else if soft {
            unresolved.insert(row.limitation_id.clone());
        } else {
            selected.insert(row.limitation_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omission.insert(format!("missing required limitation: {id}"));
    }
    let mut workflows = q
        .required_workflow_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    workflows.extend(rows.iter().map(|x| x.workflow_id.clone()));
    let mut sites = q
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    sites.extend(rows.iter().map(|x| x.site_id.clone()));
    fn groups(
        field: &str,
        u: &BTreeSet<String>,
        rows: &[Limitation6],
        s: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        u.iter()
            .filter(|v| {
                rows.iter().any(|x| {
                    s.contains(&x.limitation_id)
                        && match field {
                            "workflow" => &x.workflow_id,
                            "site" => &x.site_id,
                            _ => &x.limitation_id,
                        } == *v
                })
            })
            .cloned()
            .collect()
    }
    let sw = groups("workflow", &workflows, &rows, &selected);
    let uw = groups("workflow", &workflows, &rows, &unresolved);
    let bw = groups("workflow", &workflows, &rows, &blocked);
    let mw = workflows
        .difference(&sw)
        .filter(|id| !uw.contains(*id) && !bw.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let ss = groups("site", &sites, &rows, &selected);
    let us = groups("site", &sites, &rows, &unresolved);
    let bs = groups("site", &sites, &rows, &blocked);
    let ms = sites
        .difference(&ss)
        .filter(|id| !us.contains(*id) && !bs.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let global = q.policy_allow
        && q.protected_closure
        && q.signed_approval
        && q.raw_data_local
        && q.aggregate_only
        && q.adversarial_event_order.is_empty();
    let admitted = selected.len() + unresolved.len();
    let gate = !global
        || !blocked.is_empty()
        || !missing.is_empty()
        || !bw.is_empty()
        || !mw.is_empty()
        || !bs.is_empty()
        || !ms.is_empty()
        || admitted < q.minimum_limitation_count as usize
        || sw.len() + uw.len() < q.minimum_workflow_count as usize
        || ss.len() + us.len() < q.minimum_site_count as usize
        || selected.len() > q.max_limitations as usize;
    let disposition = if gate {
        LimitationClosureDisposition::Blocked
    } else if !unresolved.is_empty() || !uw.is_empty() || !uw.is_empty() {
        LimitationClosureDisposition::Unresolved
    } else {
        LimitationClosureDisposition::Qualified
    };
    let effects = if disposition == LimitationClosureDisposition::Qualified {
        vec![format!("close:declared-limitation:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons=vec![match disposition{LimitationClosureDisposition::Qualified=>"all limitation criteria, policy, replay, provenance, and locality gates passed".into(),LimitationClosureDisposition::Unresolved=>"criteria, counterexamples, stale, uncertain, omitted, unknown, speculative, or replay-mismatched limitations remain unresolved".into(),LimitationClosureDisposition::Blocked=>"limitation, closure, authorization, policy, coverage, or adversarial gates blocked workflow release".into()}];
    let provenance = ContentHash::of_bytes(
        rows.iter()
            .map(|x| x.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"requester":q.requester,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"disposition":disposition,"ranked_limitation_order":ranked,"selected_limitation_order":selected,"unresolved_limitation_order":unresolved,"blocked_limitation_order":blocked,"missing_limitation_order":missing,"workflow_order":workflows,"selected_workflow_order":sw,"unresolved_workflow_order":uw,"blocked_workflow_order":bw,"missing_workflow_order":mw,"site_order":sites,"selected_site_order":ss,"unresolved_site_order":us,"blocked_site_order":bs,"missing_site_order":ms,"closure_criteria_order":criteria,"satisfied_criteria_order":satisfied,"counterexample_order":counterexamples,"uncertainty_order":uncertainty,"omission_order":omission,"negative_evidence_order":negative,"contradiction_order":contradiction,"adversarial_event_order":q.adversarial_event_order,"replay_identity":q.replay_identity,"provenance_digest":provenance,"reasons":reasons,"effect_receipts":effects,"raw_data_local":q.raw_data_local,"aggregate_only":q.aggregate_only,"autonomy_tier":AutonomyTier::A1,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("limitation-closure:{}", q.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| DevplatClosureError::Artifact(e.to_string()))?;
    let digest = artifact.content_hash.clone();
    let receipt = DevplatClosureReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        requester: q.requester.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        disposition,
        ranked_limitation_order: ranked,
        selected_limitation_order: selected.into_iter().collect(),
        unresolved_limitation_order: unresolved.into_iter().collect(),
        blocked_limitation_order: blocked.into_iter().collect(),
        missing_limitation_order: missing.into_iter().collect(),
        workflow_order: workflows.into_iter().collect(),
        selected_workflow_order: sw.into_iter().collect(),
        unresolved_workflow_order: uw.into_iter().collect(),
        blocked_workflow_order: bw.into_iter().collect(),
        missing_workflow_order: mw.into_iter().collect(),
        site_order: sites.into_iter().collect(),
        selected_site_order: ss.into_iter().collect(),
        unresolved_site_order: us.into_iter().collect(),
        blocked_site_order: bs.into_iter().collect(),
        missing_site_order: ms.into_iter().collect(),
        closure_criteria_order: criteria.into_iter().collect(),
        satisfied_criteria_order: satisfied.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: q.adversarial_event_order.clone(),
        replay_identity: q.replay_identity.clone(),
        provenance_digest: provenance,
        reasons,
        closure_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        autonomy_tier: AutonomyTier::A1,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn lim(id: &str, state: EvidenceState) -> Limitation6 {
        Limitation6 {
            limitation_id: id.into(),
            workflow_id: format!("workflow:{id}"),
            site_id: format!("site:{id}"),
            claim_scope: "preclinical".into(),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            evidence_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("replay"),
            closure_criteria_order: vec!["criterion:replay".into()],
            satisfied_criteria_order: vec!["criterion:replay".into()],
            counterexample_order: Vec::new(),
            policy_allowed: true,
            protected_closure: true,
            signed_attestation: true,
            raw_data_local: true,
            aggregate_only: true,
            stale: false,
            revoked: false,
            negative_result: false,
            uncertainty_order: Vec::new(),
            omission_order: Vec::new(),
        }
    }
    fn q(items: Vec<Limitation6>) -> DevplatLimitationCase2 {
        DevplatLimitationCase2 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "closure:1".into(),
            requester: "lead".into(),
            purpose: "release".into(),
            semantic_profile: "imaging-omics".into(),
            required_limitation_order: vec!["lim:1".into()],
            required_workflow_order: vec!["workflow:lim:1".into()],
            required_site_order: vec!["site:lim:1".into()],
            replay_identity: h("replay"),
            minimum_limitation_count: 1,
            minimum_workflow_count: 1,
            minimum_site_count: 1,
            max_limitations: 8,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            limitations: items,
        }
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_devplat_multimodal_limitation_closure(&q(vec![lim("lim:1", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            LimitationClosureDisposition::Qualified
        )
    }
    #[test]
    fn unknown() {
        assert_eq!(
            assure_devplat_multimodal_limitation_closure(&q(vec![lim("lim:1", EvidenceState::Unknown)]))
                .unwrap()
                .disposition,
            LimitationClosureDisposition::Unresolved
        )
    }
    #[test]
    fn contradiction() {
        assert_eq!(
            assure_devplat_multimodal_limitation_closure(&q(vec![lim(
                "lim:1",
                EvidenceState::Contradicted
            )]))
            .unwrap()
            .disposition,
            LimitationClosureDisposition::Blocked
        )
    }
    #[test]
    fn missing() {
        assert_eq!(
            assure_devplat_multimodal_limitation_closure(&q(vec![lim("other", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            LimitationClosureDisposition::Blocked
        )
    }
    #[test]
    fn negative() {
        let mut x = lim("lim:1", EvidenceState::Supported);
        x.negative_result = true;
        assert_eq!(
            assure_devplat_multimodal_limitation_closure(&q(vec![x]))
                .unwrap()
                .negative_evidence_order,
            vec!["lim:1"]
        )
    }
    #[test]
    fn manifest() {
        devplat_multimodal_limitation_closure_manifest().validate().unwrap()
    }
}
