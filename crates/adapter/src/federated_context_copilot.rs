//! Federated continual context-compilation research copilot.
//!
//! Atlas feature: `AFA-adapter-P03-F12`.  It admits typed, institution-local context facts into
//! a reproducible compilation request; it does not read source systems, export raw data, or
//! silently turn an omitted fact into a closed Decision Section.

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

pub const FEATURE_ID: &str = "AFA-adapter-P03-F12";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-context-compilation-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "FederatedContextQuestion5@1";
pub const OUTPUT_SCHEMA: &str = "FederatedContextReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.adapter-federated-context-receipt-7+json";
pub const MAX_FACTS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextFact6 {
    pub fact_id: String,
    pub site_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub statement_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub scope_compatible: bool,
    pub fresh: bool,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub signed_attestation: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextQuestion5 {
    pub schema_version: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_fact_order: Vec<String>,
    pub required_site_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_fact_count: u32,
    pub minimum_site_count: u32,
    pub minimum_study_count: u32,
    pub max_facts: u32,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub facts: Vec<ContextFact6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContextDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContextReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: FederatedContextDisposition,
    pub ranked_fact_order: Vec<String>,
    pub selected_fact_order: Vec<String>,
    pub unresolved_fact_order: Vec<String>,
    pub blocked_fact_order: Vec<String>,
    pub missing_fact_order: Vec<String>,
    pub site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub blocked_site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub study_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub unresolved_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub context_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContextError {
    #[error("invalid federated context question or receipt: {0}")]
    Invalid(String),
    #[error("federated context artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> FederatedContextError {
    FederatedContextError::Invalid(message.into())
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
) -> Result<(), FederatedContextError> {
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

impl FederatedContextReceipt7 {
    pub fn validate(&self) -> Result<(), FederatedContextError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.autonomy_tier != AutonomyTier::A2
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_fact_order.is_empty()
            || self.site_order.is_empty()
            || self.study_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("federated context identity, closure, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_fact_order,
            &self.selected_fact_order,
            &self.unresolved_fact_order,
            &self.blocked_fact_order,
            &self.missing_fact_order,
            &self.site_order,
            &self.selected_site_order,
            &self.unresolved_site_order,
            &self.blocked_site_order,
            &self.missing_site_order,
            &self.study_order,
            &self.selected_study_order,
            &self.unresolved_study_order,
            &self.blocked_study_order,
            &self.missing_study_order,
            &self.uncertainty_order,
            &self.omission_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !ordered(values) {
                return Err(invalid("federated context ordering is not canonical"));
            }
        }
        let mut facts = self.ranked_fact_order.clone();
        facts.extend(self.missing_fact_order.iter().cloned());
        facts.sort();
        partition(
            &facts,
            &[
                &self.selected_fact_order,
                &self.unresolved_fact_order,
                &self.blocked_fact_order,
                &self.missing_fact_order,
            ],
            "fact",
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
        partition(
            &self.study_order,
            &[
                &self.selected_study_order,
                &self.unresolved_study_order,
                &self.blocked_study_order,
                &self.missing_study_order,
            ],
            "study",
        )?;
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.context_digest)
            || self.artifact.content_hash != self.context_digest
        {
            return Err(invalid("federated context digest is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:context-copilot:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("federated context effect is outside bounded gate"));
        }
        if self.disposition == FederatedContextDisposition::Qualified
            && self.effect_receipts != vec![format!("invoke:context-copilot:{}", self.request_id)]
        {
            return Err(invalid("qualified federated context effect is invalid"));
        }
        if self.disposition != FederatedContextDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified federated context must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| FederatedContextError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedContextError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| FederatedContextError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| FederatedContextError::Artifact(e.to_string()))
            })
    }
}

pub fn federated_context_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["context compiler".into(),"federation operator".into(),"research workbench".into()].into(),behavior:"qualifies federated continual typed context facts and emits an omission-aware context admission receipt without reading raw sources".into(),value:"prevents incomplete cross-site context from being promoted while exposing deterministic provenance, replay, policy, and locality evidence".into(),inputs:vec![TypedPort{name:"federated_context_question".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"federated_context_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact,Effect::FederationExport].into(),permissions:["invoke:context-copilot".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:vec![AuthorityRequirement{role:"federation operator".into(),reason:"cross-site context admission requires explicit authority and purpose".into()}],autonomy_tier:AutonomyTier::A2,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

fn validate_request(q: &FederatedContextQuestion5) -> Result<(), FederatedContextError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.requester.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.semantic_profile.trim().is_empty()
        || q.required_fact_order.is_empty()
        || q.required_site_order.is_empty()
        || q.required_study_order.is_empty()
        || !ordered(&q.required_fact_order)
        || !ordered(&q.required_site_order)
        || !ordered(&q.required_study_order)
        || !ordered(&q.adversarial_event_order)
        || q.minimum_fact_count == 0
        || q.minimum_site_count == 0
        || q.minimum_study_count == 0
        || q.max_facts == 0
        || q.max_facts as usize > MAX_FACTS
        || !digest_valid(&q.replay_identity)
        || !q.policy_allow
        || !q.federation_allow
        || !q.signed_approval
        || !q.raw_data_local
        || !q.aggregate_only
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.facts.is_empty()
        || q.facts.len() > MAX_FACTS
    {
        return Err(invalid("federated context identity, closure, policy, capacity, replay, locality, or bounds are invalid"));
    }
    let mut seen = BTreeSet::new();
    for fact in &q.facts {
        if fact.fact_id.trim().is_empty()
            || fact.site_id.trim().is_empty()
            || fact.study_id.trim().is_empty()
            || fact.semantic_profile != q.semantic_profile
            || !digest_valid(&fact.statement_digest)
            || !digest_valid(&fact.evidence_digest)
            || !digest_valid(&fact.provenance_digest)
            || !digest_valid(&fact.replay_identity)
            || !ordered(&fact.uncertainty_order)
            || !ordered(&fact.omission_order)
            || !seen.insert(fact.fact_id.clone())
        {
            return Err(invalid(
                "context fact identity, profile, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn qualify_federated_context(
    q: &FederatedContextQuestion5,
) -> Result<FederatedContextReceipt7, FederatedContextError> {
    validate_request(q)?;
    let mut rows = q.facts.clone();
    let rank = |s: EvidenceState| match s {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    };
    rows.sort_by(|a, b| {
        (rank(a.evidence_state), !a.fresh, a.fact_id.as_str()).cmp(&(
            rank(b.evidence_state),
            !b.fresh,
            b.fact_id.as_str(),
        ))
    });
    let ranked = rows.iter().map(|x| x.fact_id.clone()).collect::<Vec<_>>();
    let required = q
        .required_fact_order
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
    for row in &rows {
        uncertainty.extend(row.uncertainty_order.iter().cloned());
        omission.extend(row.omission_order.iter().cloned());
        if row.negative_result {
            negative.insert(row.fact_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.fact_id.clone());
        }
        let hard = !row.scope_compatible
            || !row.policy_allowed
            || !row.protected_closure
            || !row.raw_data_local
            || !row.aggregate_only
            || !row.signed_attestation
            || row.revoked;
        let soft = !row.fresh
            || row.replay_identity != q.replay_identity
            || !row.uncertainty_order.is_empty()
            || !row.omission_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.fact_id.clone());
        } else if soft {
            unresolved.insert(row.fact_id.clone());
        } else {
            selected.insert(row.fact_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omission.insert(format!("missing required context fact: {id}"));
    }
    let mut sites = q
        .required_site_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    sites.extend(rows.iter().map(|x| x.site_id.clone()));
    let mut studies = q
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    studies.extend(rows.iter().map(|x| x.study_id.clone()));
    fn groups(
        field: &str,
        u: &BTreeSet<String>,
        rows: &[ContextFact6],
        s: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        u.iter()
            .filter(|v| {
                rows.iter().any(|x| {
                    s.contains(&x.fact_id)
                        && match field {
                            "site" => &x.site_id,
                            "study" => &x.study_id,
                            _ => &x.fact_id,
                        } == *v
                })
            })
            .cloned()
            .collect()
    }
    let ss = groups("site", &sites, &rows, &selected);
    let us = groups("site", &sites, &rows, &unresolved);
    let bs = groups("site", &sites, &rows, &blocked);
    let ms = sites
        .difference(&ss)
        .filter(|id| !us.contains(*id) && !bs.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let st = groups("study", &studies, &rows, &selected);
    let ut = groups("study", &studies, &rows, &unresolved);
    let bt = groups("study", &studies, &rows, &blocked);
    let mt = studies
        .difference(&st)
        .filter(|id| !ut.contains(*id) && !bt.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let global = q.policy_allow
        && q.federation_allow
        && q.signed_approval
        && q.raw_data_local
        && q.aggregate_only
        && q.adversarial_event_order.is_empty();
    let admitted = selected.len() + unresolved.len();
    let gate = !global
        || !blocked.is_empty()
        || !missing.is_empty()
        || !bs.is_empty()
        || !ms.is_empty()
        || !bt.is_empty()
        || !mt.is_empty()
        || admitted < q.minimum_fact_count as usize
        || ss.len() + us.len() < q.minimum_site_count as usize
        || st.len() + ut.len() < q.minimum_study_count as usize
        || selected.len() > q.max_facts as usize;
    let disposition = if gate {
        FederatedContextDisposition::Blocked
    } else if !unresolved.is_empty() || !us.is_empty() || !ut.is_empty() {
        FederatedContextDisposition::Unresolved
    } else {
        FederatedContextDisposition::Qualified
    };
    let effects = if disposition == FederatedContextDisposition::Qualified {
        vec![format!("invoke:context-copilot:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons=vec![match disposition{FederatedContextDisposition::Qualified=>"all context scope, evidence, policy, federation, replay, provenance, and locality gates passed".into(),FederatedContextDisposition::Unresolved=>"stale, uncertain, omitted, unknown, speculative, or replay-mismatched facts remain unresolved".into(),FederatedContextDisposition::Blocked=>"context scope, closure, authorization, policy, federation, coverage, or adversarial gates blocked compilation".into()}];
    let provenance = ContentHash::of_bytes(
        rows.iter()
            .map(|x| x.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"requester":q.requester,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"disposition":disposition,"ranked_fact_order":ranked,"selected_fact_order":selected,"unresolved_fact_order":unresolved,"blocked_fact_order":blocked,"missing_fact_order":missing,"site_order":sites,"selected_site_order":ss,"unresolved_site_order":us,"blocked_site_order":bs,"missing_site_order":ms,"study_order":studies,"selected_study_order":st,"unresolved_study_order":ut,"blocked_study_order":bt,"missing_study_order":mt,"uncertainty_order":uncertainty,"omission_order":omission,"negative_evidence_order":negative,"contradiction_order":contradiction,"adversarial_event_order":q.adversarial_event_order,"replay_identity":q.replay_identity,"provenance_digest":provenance,"reasons":reasons,"effect_receipts":effects,"raw_data_local":q.raw_data_local,"aggregate_only":q.aggregate_only,"autonomy_tier":AutonomyTier::A2,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("federated-context:{}", q.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| FederatedContextError::Artifact(e.to_string()))?;
    let digest = artifact.content_hash.clone();
    let receipt = FederatedContextReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        requester: q.requester.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        disposition,
        ranked_fact_order: ranked,
        selected_fact_order: selected.into_iter().collect(),
        unresolved_fact_order: unresolved.into_iter().collect(),
        blocked_fact_order: blocked.into_iter().collect(),
        missing_fact_order: missing.into_iter().collect(),
        site_order: sites.into_iter().collect(),
        selected_site_order: ss.into_iter().collect(),
        unresolved_site_order: us.into_iter().collect(),
        blocked_site_order: bs.into_iter().collect(),
        missing_site_order: ms.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        selected_study_order: st.into_iter().collect(),
        unresolved_study_order: ut.into_iter().collect(),
        blocked_study_order: bt.into_iter().collect(),
        missing_study_order: mt.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: q.adversarial_event_order.clone(),
        replay_identity: q.replay_identity.clone(),
        provenance_digest: provenance,
        reasons,
        context_digest: digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        autonomy_tier: AutonomyTier::A2,
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
    fn fact(id: &str, state: EvidenceState) -> ContextFact6 {
        ContextFact6 {
            fact_id: id.into(),
            site_id: format!("site:{id}"),
            study_id: format!("study:{id}"),
            semantic_profile: "imaging-omics".into(),
            statement_digest: h(id),
            evidence_digest: h(&format!("e:{id}")),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("replay"),
            evidence_state: state,
            scope_compatible: true,
            fresh: true,
            policy_allowed: true,
            protected_closure: true,
            raw_data_local: true,
            aggregate_only: true,
            signed_attestation: true,
            revoked: false,
            negative_result: false,
            uncertainty_order: Vec::new(),
            omission_order: Vec::new(),
        }
    }
    fn q(items: Vec<ContextFact6>) -> FederatedContextQuestion5 {
        FederatedContextQuestion5 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "context:1".into(),
            requester: "compiler".into(),
            purpose: "compile".into(),
            semantic_profile: "imaging-omics".into(),
            required_fact_order: vec!["fact:1".into()],
            required_site_order: vec!["site:fact:1".into()],
            required_study_order: vec!["study:fact:1".into()],
            replay_identity: h("replay"),
            minimum_fact_count: 1,
            minimum_site_count: 1,
            minimum_study_count: 1,
            max_facts: 8,
            policy_allow: true,
            federation_allow: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            facts: items,
        }
    }
    #[test]
    fn qualified() {
        assert_eq!(
            qualify_federated_context(&q(vec![fact("fact:1", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            FederatedContextDisposition::Qualified
        )
    }
    #[test]
    fn unknown() {
        assert_eq!(
            qualify_federated_context(&q(vec![fact("fact:1", EvidenceState::Unknown)]))
                .unwrap()
                .disposition,
            FederatedContextDisposition::Unresolved
        )
    }
    #[test]
    fn contradiction() {
        assert_eq!(
            qualify_federated_context(&q(vec![fact("fact:1", EvidenceState::Contradicted)]))
                .unwrap()
                .disposition,
            FederatedContextDisposition::Blocked
        )
    }
    #[test]
    fn missing() {
        assert_eq!(
            qualify_federated_context(&q(vec![fact("other", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            FederatedContextDisposition::Blocked
        )
    }
    #[test]
    fn negative() {
        let mut x = fact("fact:1", EvidenceState::Supported);
        x.negative_result = true;
        assert_eq!(
            qualify_federated_context(&q(vec![x]))
                .unwrap()
                .negative_evidence_order,
            vec!["fact:1"]
        )
    }
    #[test]
    fn manifest() {
        federated_context_copilot_manifest().validate().unwrap()
    }
}
