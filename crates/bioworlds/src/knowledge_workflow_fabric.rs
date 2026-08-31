//! Multimodal multi-study knowledge-representation workflow fabric.
//!
//! Atlas feature: `AFA-bioworlds-P04-F14`.  This is a typed admission fabric: it compiles
//! institution-local observation attestations into a deterministic knowledge-workflow receipt,
//! retaining uncertainty, omissions, negative results, and contradictions.  It never reads raw
//! observations, moves protected data, or makes clinical decisions.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioworlds-P04-F14";
pub const CONTRACT_VERSION: &str =
    "bioworlds-multimodal-knowledge-representation-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "KnowledgeWorkflowRequest5@1";
pub const OUTPUT_SCHEMA: &str = "KnowledgeWorkflowReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.bioworlds-knowledge-workflow-receipt-7+json";
pub const MAX_OBSERVATIONS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeObservation6 {
    pub observation_id: String,
    pub study_id: String,
    pub modality: String,
    pub concept_id: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub value_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub schema_compatible: bool,
    pub qc_passed: bool,
    pub policy_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub signed_attestation: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeWorkflowRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_observation_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub required_concept_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_observation_count: u32,
    pub minimum_study_count: u32,
    pub minimum_modality_count: u32,
    pub minimum_concept_count: u32,
    pub max_observations: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub observations: Vec<KnowledgeObservation6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeWorkflowDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnowledgeWorkflowReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: KnowledgeWorkflowDisposition,
    pub ranked_observation_order: Vec<String>,
    pub selected_observation_order: Vec<String>,
    pub unresolved_observation_order: Vec<String>,
    pub blocked_observation_order: Vec<String>,
    pub missing_observation_order: Vec<String>,
    pub study_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub unresolved_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub unresolved_modality_order: Vec<String>,
    pub blocked_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub concept_order: Vec<String>,
    pub selected_concept_order: Vec<String>,
    pub unresolved_concept_order: Vec<String>,
    pub blocked_concept_order: Vec<String>,
    pub missing_concept_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub knowledge_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum KnowledgeWorkflowError {
    #[error("invalid multimodal knowledge workflow request or receipt: {0}")]
    Invalid(String),
    #[error("multimodal knowledge workflow artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> KnowledgeWorkflowError {
    KnowledgeWorkflowError::Invalid(message.into())
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
) -> Result<(), KnowledgeWorkflowError> {
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

impl KnowledgeWorkflowReceipt7 {
    pub fn validate(&self) -> Result<(), KnowledgeWorkflowError> {
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
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.ranked_observation_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.concept_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("knowledge workflow identity, closure, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_observation_order,
            &self.selected_observation_order,
            &self.unresolved_observation_order,
            &self.blocked_observation_order,
            &self.missing_observation_order,
            &self.study_order,
            &self.selected_study_order,
            &self.unresolved_study_order,
            &self.blocked_study_order,
            &self.missing_study_order,
            &self.modality_order,
            &self.selected_modality_order,
            &self.unresolved_modality_order,
            &self.blocked_modality_order,
            &self.missing_modality_order,
            &self.concept_order,
            &self.selected_concept_order,
            &self.unresolved_concept_order,
            &self.blocked_concept_order,
            &self.missing_concept_order,
            &self.uncertainty_order,
            &self.omission_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !ordered(values) {
                return Err(invalid("knowledge workflow ordering is not canonical"));
            }
        }
        let mut observation_universe = self.ranked_observation_order.clone();
        observation_universe.extend(self.missing_observation_order.iter().cloned());
        observation_universe.sort();
        partition(
            &observation_universe,
            &[
                &self.selected_observation_order,
                &self.unresolved_observation_order,
                &self.blocked_observation_order,
                &self.missing_observation_order,
            ],
            "observation",
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
        partition(
            &self.modality_order,
            &[
                &self.selected_modality_order,
                &self.unresolved_modality_order,
                &self.blocked_modality_order,
                &self.missing_modality_order,
            ],
            "modality",
        )?;
        partition(
            &self.concept_order,
            &[
                &self.selected_concept_order,
                &self.unresolved_concept_order,
                &self.blocked_concept_order,
                &self.missing_concept_order,
            ],
            "concept",
        )?;
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.knowledge_digest)
            || self.artifact.content_hash != self.knowledge_digest
        {
            return Err(invalid("knowledge workflow digest is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("represent:local-knowledge:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid("knowledge workflow effect is outside bounded gate"));
        }
        if self.disposition == KnowledgeWorkflowDisposition::Qualified
            && self.effect_receipts
                != vec![format!("represent:local-knowledge:{}", self.request_id)]
        {
            return Err(invalid("qualified knowledge workflow effect is invalid"));
        }
        if self.disposition != KnowledgeWorkflowDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified knowledge workflow must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| KnowledgeWorkflowError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, KnowledgeWorkflowError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| KnowledgeWorkflowError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| KnowledgeWorkflowError::Artifact(e.to_string()))
            })
    }
}

pub fn knowledge_workflow_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioworlds".into(), consumers: ["knowledge engineer".into(), "multimodal researcher".into(), "workflow operator".into()].into(), behavior: "compiles multimodal multi-study observation attestations into a deterministic omission-aware knowledge-representation workflow receipt without reading raw observations".into(), value: "makes cross-study semantic compatibility, quality, provenance, replay, negative evidence, and protected locality auditable before a knowledge world is released".into(), inputs: vec![TypedPort { name: "knowledge_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "knowledge_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["represent:declared-local-observations".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "anndata".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }], authority_requirements: vec![bioprism_foundation::AuthorityRequirement { role: "workflow operator".into(), reason: "knowledge representation admission uses governed local attestations and requires explicit researcher authority".into() }], autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(q: &KnowledgeWorkflowRequest5) -> Result<(), KnowledgeWorkflowError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.researcher.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.semantic_profile.trim().is_empty()
        || q.required_observation_order.is_empty()
        || q.required_study_order.is_empty()
        || q.required_modality_order.is_empty()
        || q.required_concept_order.is_empty()
        || !ordered(&q.required_observation_order)
        || !ordered(&q.required_study_order)
        || !ordered(&q.required_modality_order)
        || !ordered(&q.required_concept_order)
        || !ordered(&q.adversarial_event_order)
        || q.minimum_observation_count == 0
        || q.minimum_study_count == 0
        || q.minimum_modality_count == 0
        || q.minimum_concept_count == 0
        || q.max_observations == 0
        || q.max_observations as usize > MAX_OBSERVATIONS
        || !digest_valid(&q.replay_identity)
        || q.boundary != PRECLINICAL_BOUNDARY
        || !q.policy_allow
        || !q.protected_closure
        || !q.signed_approval
        || !q.raw_data_local
        || !q.aggregate_only
        || q.observations.is_empty()
        || q.observations.len() > MAX_OBSERVATIONS
    {
        return Err(invalid("knowledge workflow identity, closure, policy, capacity, replay, locality, boundary, or bounds are invalid"));
    }
    let mut seen = BTreeSet::new();
    for observation in &q.observations {
        if observation.observation_id.trim().is_empty()
            || observation.study_id.trim().is_empty()
            || observation.modality.trim().is_empty()
            || observation.concept_id.trim().is_empty()
            || observation.semantic_profile != q.semantic_profile
            || !digest_valid(&observation.value_digest)
            || !digest_valid(&observation.provenance_digest)
            || !digest_valid(&observation.replay_identity)
            || !ordered(&observation.uncertainty_order)
            || !ordered(&observation.omission_order)
            || !seen.insert(observation.observation_id.clone())
        {
            return Err(invalid(
                "observation identity, profile, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn compile_knowledge_workflow(
    q: &KnowledgeWorkflowRequest5,
) -> Result<KnowledgeWorkflowReceipt7, KnowledgeWorkflowError> {
    validate_request(q)?;
    let mut rows = q.observations.clone();
    let rank = |state: EvidenceState| match state {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    };
    rows.sort_by(|a, b| {
        (rank(a.evidence_state), a.stale, a.observation_id.as_str()).cmp(&(
            rank(b.evidence_state),
            b.stale,
            b.observation_id.as_str(),
        ))
    });
    let ranked = rows
        .iter()
        .map(|row| row.observation_id.clone())
        .collect::<Vec<_>>();
    let required = q
        .required_observation_order
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
            negative.insert(row.observation_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.observation_id.clone());
        }
        let hard = !row.schema_compatible
            || !row.qc_passed
            || !row.policy_allowed
            || !row.raw_data_local
            || !row.aggregate_only
            || !row.signed_attestation
            || row.revoked;
        let soft = row.stale
            || row.replay_identity != q.replay_identity
            || !row.uncertainty_order.is_empty()
            || !row.omission_order.is_empty()
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            );
        if hard || row.evidence_state == EvidenceState::Contradicted {
            blocked.insert(row.observation_id.clone());
        } else if soft {
            unresolved.insert(row.observation_id.clone());
        } else {
            selected.insert(row.observation_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omission.insert(format!("missing required observation: {id}"));
    }
    let mut studies = q
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    studies.extend(rows.iter().map(|row| row.study_id.clone()));
    let mut modalities = q
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    modalities.extend(rows.iter().map(|row| row.modality.clone()));
    let mut concepts = q
        .required_concept_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    concepts.extend(rows.iter().map(|row| row.concept_id.clone()));
    fn groups(
        field: &str,
        universe: &BTreeSet<String>,
        rows: &[KnowledgeObservation6],
        state: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        universe
            .iter()
            .filter(|value| {
                rows.iter().any(|row| {
                    state.contains(&row.observation_id)
                        && match field {
                            "study" => &row.study_id,
                            "modality" => &row.modality,
                            "concept" => &row.concept_id,
                            _ => &row.observation_id,
                        } == *value
                })
            })
            .cloned()
            .collect()
    }
    let ss = groups("study", &studies, &rows, &selected);
    let us = groups("study", &studies, &rows, &unresolved);
    let bs = groups("study", &studies, &rows, &blocked);
    let ms = studies
        .difference(&ss)
        .filter(|id| !us.contains(*id) && !bs.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let sm = groups("modality", &modalities, &rows, &selected);
    let um = groups("modality", &modalities, &rows, &unresolved);
    let bm = groups("modality", &modalities, &rows, &blocked);
    let mm = modalities
        .difference(&sm)
        .filter(|id| !um.contains(*id) && !bm.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let sc = groups("concept", &concepts, &rows, &selected);
    let uc = groups("concept", &concepts, &rows, &unresolved);
    let bc = groups("concept", &concepts, &rows, &blocked);
    let mc = concepts
        .difference(&sc)
        .filter(|id| !uc.contains(*id) && !bc.contains(*id))
        .cloned()
        .collect::<BTreeSet<_>>();
    let global = q.policy_allow
        && q.protected_closure
        && q.signed_approval
        && q.raw_data_local
        && q.aggregate_only
        && q.adversarial_event_order.is_empty();
    let admitted = selected.len() + unresolved.len();
    let blocked_gate = !global
        || !blocked.is_empty()
        || !missing.is_empty()
        || !bs.is_empty()
        || !ms.is_empty()
        || !bm.is_empty()
        || !mm.is_empty()
        || !bc.is_empty()
        || !mc.is_empty()
        || admitted < q.minimum_observation_count as usize
        || ss.len() + us.len() < q.minimum_study_count as usize
        || sm.len() + um.len() < q.minimum_modality_count as usize
        || sc.len() + uc.len() < q.minimum_concept_count as usize
        || selected.len() > q.max_observations as usize;
    let disposition = if blocked_gate {
        KnowledgeWorkflowDisposition::Blocked
    } else if !unresolved.is_empty() || !us.is_empty() || !um.is_empty() || !uc.is_empty() {
        KnowledgeWorkflowDisposition::Unresolved
    } else {
        KnowledgeWorkflowDisposition::Qualified
    };
    let effects = if disposition == KnowledgeWorkflowDisposition::Qualified {
        vec![format!("represent:local-knowledge:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons=vec![match disposition {KnowledgeWorkflowDisposition::Qualified=>"all semantic, quality, policy, replay, provenance, and locality gates passed".into(),KnowledgeWorkflowDisposition::Unresolved=>"stale, uncertain, omitted, unknown, speculative, or replay-mismatched observations remain unresolved".into(),KnowledgeWorkflowDisposition::Blocked=>"semantic, quality, policy, closure, authorization, coverage, or adversarial gates blocked knowledge representation".into()}];
    let provenance = ContentHash::of_bytes(
        rows.iter()
            .map(|row| row.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"researcher":q.researcher,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"disposition":disposition,"ranked_observation_order":ranked,"selected_observation_order":selected,"unresolved_observation_order":unresolved,"blocked_observation_order":blocked,"missing_observation_order":missing,"study_order":studies,"selected_study_order":ss,"unresolved_study_order":us,"blocked_study_order":bs,"missing_study_order":ms,"modality_order":modalities,"selected_modality_order":sm,"unresolved_modality_order":um,"blocked_modality_order":bm,"missing_modality_order":mm,"concept_order":concepts,"selected_concept_order":sc,"unresolved_concept_order":uc,"blocked_concept_order":bc,"missing_concept_order":mc,"uncertainty_order":uncertainty,"omission_order":omission,"negative_evidence_order":negative,"contradiction_order":contradiction,"adversarial_event_order":q.adversarial_event_order,"replay_identity":q.replay_identity,"provenance_digest":provenance,"reasons":reasons,"effect_receipts":effects,"raw_data_local":q.raw_data_local,"aggregate_only":q.aggregate_only,"autonomy_tier":AutonomyTier::A1,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("knowledge-workflow:{}", q.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| KnowledgeWorkflowError::Artifact(e.to_string()))?;
    let digest = artifact.content_hash.clone();
    let receipt = KnowledgeWorkflowReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        researcher: q.researcher.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        disposition,
        ranked_observation_order: ranked,
        selected_observation_order: selected.into_iter().collect(),
        unresolved_observation_order: unresolved.into_iter().collect(),
        blocked_observation_order: blocked.into_iter().collect(),
        missing_observation_order: missing.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        selected_study_order: ss.into_iter().collect(),
        unresolved_study_order: us.into_iter().collect(),
        blocked_study_order: bs.into_iter().collect(),
        missing_study_order: ms.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        selected_modality_order: sm.into_iter().collect(),
        unresolved_modality_order: um.into_iter().collect(),
        blocked_modality_order: bm.into_iter().collect(),
        missing_modality_order: mm.into_iter().collect(),
        concept_order: concepts.into_iter().collect(),
        selected_concept_order: sc.into_iter().collect(),
        unresolved_concept_order: uc.into_iter().collect(),
        blocked_concept_order: bc.into_iter().collect(),
        missing_concept_order: mc.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: q.adversarial_event_order.clone(),
        replay_identity: q.replay_identity.clone(),
        provenance_digest: provenance,
        reasons,
        knowledge_digest: digest,
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
    fn obs(id: &str, state: EvidenceState) -> KnowledgeObservation6 {
        KnowledgeObservation6 {
            observation_id: id.into(),
            study_id: format!("study:{id}"),
            modality: "imaging".into(),
            concept_id: format!("concept:{id}"),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            value_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("replay"),
            schema_compatible: true,
            qc_passed: true,
            policy_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            signed_attestation: true,
            stale: false,
            revoked: false,
            negative_result: false,
            uncertainty_order: Vec::new(),
            omission_order: Vec::new(),
        }
    }
    fn req(items: Vec<KnowledgeObservation6>) -> KnowledgeWorkflowRequest5 {
        KnowledgeWorkflowRequest5 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "knowledge:1".into(),
            researcher: "researcher".into(),
            purpose: "world".into(),
            semantic_profile: "imaging-omics".into(),
            required_observation_order: vec!["obs:1".into()],
            required_study_order: vec!["study:obs:1".into()],
            required_modality_order: vec!["imaging".into()],
            required_concept_order: vec!["concept:obs:1".into()],
            replay_identity: h("replay"),
            minimum_observation_count: 1,
            minimum_study_count: 1,
            minimum_modality_count: 1,
            minimum_concept_count: 1,
            max_observations: 8,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            observations: items,
        }
    }
    #[test]
    fn qualified() {
        assert_eq!(
            compile_knowledge_workflow(&req(vec![obs("obs:1", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            KnowledgeWorkflowDisposition::Qualified
        )
    }
    #[test]
    fn unknown_unresolved() {
        assert_eq!(
            compile_knowledge_workflow(&req(vec![obs("obs:1", EvidenceState::Unknown)]))
                .unwrap()
                .disposition,
            KnowledgeWorkflowDisposition::Unresolved
        )
    }
    #[test]
    fn contradiction_blocked() {
        assert_eq!(
            compile_knowledge_workflow(&req(vec![obs("obs:1", EvidenceState::Contradicted)]))
                .unwrap()
                .disposition,
            KnowledgeWorkflowDisposition::Blocked
        )
    }
    #[test]
    fn missing_blocked() {
        assert_eq!(
            compile_knowledge_workflow(&req(vec![obs("other", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            KnowledgeWorkflowDisposition::Blocked
        )
    }
    #[test]
    fn negative_retained() {
        let mut x = obs("obs:1", EvidenceState::Supported);
        x.negative_result = true;
        assert_eq!(
            compile_knowledge_workflow(&req(vec![x]))
                .unwrap()
                .negative_evidence_order,
            vec!["obs:1"]
        )
    }
    #[test]
    fn manifest() {
        knowledge_workflow_manifest().validate().unwrap()
    }
}
