//! Prospective high-throughput statistical, causal, and ML analysis workbench.
//!
//! Atlas feature: `AFA-prism-P13-F19`.
//!
//! PRISM owns decision-state comparison; this product adds the admission boundary around a
//! multi-study analysis portfolio. It compiles typed analysis attestations into a deterministic
//! workbench receipt, preserving negative and unresolved evidence without running a model.

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

pub const FEATURE_ID: &str = "AFA-prism-P13-F19";
pub const CONTRACT_VERSION: &str = "prism-prospective-statistical-causal-ml-analysis-workbench/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisWorkbenchRequest5@1";
pub const OUTPUT_SCHEMA: &str = "AnalysisWorkbenchReceipt7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.prism-analysis-workbench-receipt-7+json";
pub const MAX_JOBS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisJob6 {
    pub job_id: String,
    pub study_id: String,
    pub modality: String,
    pub model_id: String,
    pub estimand: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub input_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub identification_supported: bool,
    pub comparability_supported: bool,
    pub quality_supported: bool,
    pub policy_allowed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub signed_approval: bool,
    pub stale: bool,
    pub revoked: bool,
    pub negative_result: bool,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkbenchRequest5 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_job_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub required_model_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub minimum_job_count: u32,
    pub minimum_study_count: u32,
    pub minimum_model_count: u32,
    pub max_jobs: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
    pub jobs: Vec<AnalysisJob6>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisWorkbenchDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisWorkbenchReceipt7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: AnalysisWorkbenchDisposition,
    pub ranked_job_order: Vec<String>,
    pub selected_job_order: Vec<String>,
    pub unresolved_job_order: Vec<String>,
    pub blocked_job_order: Vec<String>,
    pub missing_job_order: Vec<String>,
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
    pub model_order: Vec<String>,
    pub selected_model_order: Vec<String>,
    pub unresolved_model_order: Vec<String>,
    pub blocked_model_order: Vec<String>,
    pub missing_model_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub reasons: Vec<String>,
    pub analysis_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub autonomy_tier: AutonomyTier,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnalysisWorkbenchError {
    #[error("invalid PRISM analysis-workbench request or receipt: {0}")]
    Invalid(String),
    #[error("PRISM analysis-workbench artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> AnalysisWorkbenchError {
    AnalysisWorkbenchError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn partition(
    universe: &[String],
    parts: &[&[String]],
    label: &str,
) -> Result<(), AnalysisWorkbenchError> {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    if expected.len() != universe.len() {
        return Err(invalid(format!("{label} universe contains duplicates")));
    }
    let mut flat = Vec::new();
    for part in parts {
        if !canonical(part) || part.iter().any(|id| !expected.contains(id)) {
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

impl AnalysisWorkbenchReceipt7 {
    pub fn validate(&self) -> Result<(), AnalysisWorkbenchError> {
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
            || self.ranked_job_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.model_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("analysis workbench identity, closure, locality, autonomy, or effects are incomplete"));
        }
        for values in [
            &self.ranked_job_order,
            &self.selected_job_order,
            &self.unresolved_job_order,
            &self.blocked_job_order,
            &self.missing_job_order,
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
            &self.model_order,
            &self.selected_model_order,
            &self.unresolved_model_order,
            &self.blocked_model_order,
            &self.missing_model_order,
            &self.uncertainty_order,
            &self.omission_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
        ] {
            if !canonical(values) {
                return Err(invalid("analysis workbench ordering is not canonical"));
            }
        }
        let mut job_universe = self.ranked_job_order.clone();
        job_universe.extend(self.missing_job_order.iter().cloned());
        job_universe.sort();
        partition(
            &job_universe,
            &[
                &self.selected_job_order,
                &self.unresolved_job_order,
                &self.blocked_job_order,
                &self.missing_job_order,
            ],
            "job",
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
            &self.model_order,
            &[
                &self.selected_model_order,
                &self.unresolved_model_order,
                &self.blocked_model_order,
                &self.missing_model_order,
            ],
            "model",
        )?;
        if !digest_valid(&self.replay_identity)
            || !digest_valid(&self.provenance_digest)
            || !digest_valid(&self.analysis_digest)
            || self.artifact.content_hash != self.analysis_digest
        {
            return Err(invalid("analysis workbench digest is invalid"));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("analyze:local-portfolio:") && effect != "block:unsafe-release"
        }) {
            return Err(invalid(
                "analysis workbench effect is outside the bounded gate",
            ));
        }
        if self.disposition == AnalysisWorkbenchDisposition::Qualified
            && self.effect_receipts != vec![format!("analyze:local-portfolio:{}", self.request_id)]
        {
            return Err(invalid("qualified analysis workbench effect is invalid"));
        }
        if self.disposition != AnalysisWorkbenchDisposition::Qualified
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified analysis workbench must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| AnalysisWorkbenchError::Artifact(e.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, AnalysisWorkbenchError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|e| AnalysisWorkbenchError::Artifact(e.to_string()))
            .and_then(|value| {
                ContentHash::of_value(&value)
                    .map_err(|e| AnalysisWorkbenchError::Artifact(e.to_string()))
            })
    }
}

pub fn analysis_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"prism".into(),consumers:["statistical researcher".into(),"causal inference lead".into(),"analysis workbench operator".into()].into(),behavior:"qualifies a prospective multi-study statistical, causal, and ML analysis portfolio and emits an omission-aware decision-state receipt without executing models".into(),value:"makes analysis identification, comparability, quality, provenance, replay, negative evidence, and protected locality auditable before computation".into(),inputs:vec![TypedPort{name:"analysis_workbench_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"analysis_workbench_receipt".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["analyze:declared-local-portfolio".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())},EvidenceReference{source_id:"ro-crate".into(),state:EvidenceState::Supported,locator:Some("https://www.researchobject.org/ro-crate/specification.html".into())}],authority_requirements:vec![AuthorityRequirement{role:"analysis workbench operator".into(),reason:"analysis admission can consume governed local artifacts and requires explicit researcher authority".into()}],autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::McpTool,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}

fn validate_request(q: &AnalysisWorkbenchRequest5) -> Result<(), AnalysisWorkbenchError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.researcher.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.semantic_profile.trim().is_empty()
        || q.required_job_order.is_empty()
        || q.required_study_order.is_empty()
        || q.required_modality_order.is_empty()
        || q.required_model_order.is_empty()
        || !canonical(&q.required_job_order)
        || !canonical(&q.required_study_order)
        || !canonical(&q.required_modality_order)
        || !canonical(&q.required_model_order)
        || !canonical(&q.adversarial_event_order)
        || q.minimum_job_count == 0
        || q.minimum_study_count == 0
        || q.minimum_model_count == 0
        || q.max_jobs == 0
        || !digest_valid(&q.replay_identity)
        || q.boundary != PRECLINICAL_BOUNDARY
        || q.jobs.is_empty()
        || q.jobs.len() > MAX_JOBS
    {
        return Err(invalid("analysis workbench identity, closure, capacity, replay, boundary, or bounds are invalid"));
    }
    let mut ids = BTreeSet::new();
    for job in &q.jobs {
        if job.job_id.trim().is_empty()
            || job.study_id.trim().is_empty()
            || job.modality.trim().is_empty()
            || job.model_id.trim().is_empty()
            || job.estimand.trim().is_empty()
            || job.semantic_profile != q.semantic_profile
            || !digest_valid(&job.input_digest)
            || !digest_valid(&job.provenance_digest)
            || !digest_valid(&job.replay_identity)
            || !canonical(&job.uncertainty_order)
            || !canonical(&job.omission_order)
            || !ids.insert(job.job_id.clone())
        {
            return Err(invalid(
                "analysis job identity, profile, digest, or ordering is invalid",
            ));
        }
    }
    Ok(())
}

pub fn qualify_analysis_workbench(
    q: &AnalysisWorkbenchRequest5,
) -> Result<AnalysisWorkbenchReceipt7, AnalysisWorkbenchError> {
    validate_request(q)?;
    let mut rows = q.jobs.clone();
    let rank = |s: EvidenceState| match s {
        EvidenceState::Proven => 0,
        EvidenceState::Supported => 1,
        EvidenceState::Speculative => 2,
        EvidenceState::Unknown => 3,
        EvidenceState::Contradicted => 4,
    };
    rows.sort_by(|a, b| {
        (rank(a.evidence_state), a.stale, a.job_id.as_str()).cmp(&(
            rank(b.evidence_state),
            b.stale,
            b.job_id.as_str(),
        ))
    });
    let ranked = rows.iter().map(|x| x.job_id.clone()).collect::<Vec<_>>();
    let required = q
        .required_job_order
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
            negative.insert(row.job_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.job_id.clone());
        }
        let hard = !row.identification_supported
            || !row.comparability_supported
            || !row.quality_supported
            || !row.policy_allowed
            || !row.raw_data_local
            || !row.aggregate_only
            || !row.signed_approval
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
            blocked.insert(row.job_id.clone());
        } else if soft {
            unresolved.insert(row.job_id.clone());
        } else {
            selected.insert(row.job_id.clone());
        }
    }
    let missing = required
        .difference(&ranked.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &missing {
        omission.insert(format!("missing required analysis job: {id}"));
    }
    let mut studies = q
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    studies.extend(rows.iter().map(|x| x.study_id.clone()));
    let mut modalities = q
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    modalities.extend(rows.iter().map(|x| x.modality.clone()));
    let mut models = q
        .required_model_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    models.extend(rows.iter().map(|x| x.model_id.clone()));
    fn groups(
        key: &str,
        universe: &BTreeSet<String>,
        rows: &[AnalysisJob6],
        selected: &BTreeSet<String>,
        unresolved: &BTreeSet<String>,
        blocked: &BTreeSet<String>,
    ) -> (
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
    ) {
        let choose = |id: &String, set: &BTreeSet<String>| {
            rows.iter().any(|x| {
                set.contains(&x.job_id)
                    && match key {
                        "study" => &x.study_id,
                        "modality" => &x.modality,
                        "model" => &x.model_id,
                        _ => &x.job_id,
                    } == id
            })
        };
        let a = universe
            .iter()
            .filter(|id| choose(id, selected))
            .cloned()
            .collect::<BTreeSet<_>>();
        let b = universe
            .iter()
            .filter(|id| !a.contains(*id) && choose(id, unresolved))
            .cloned()
            .collect::<BTreeSet<_>>();
        let c = universe
            .iter()
            .filter(|id| !a.contains(*id) && !b.contains(*id) && choose(id, blocked))
            .cloned()
            .collect::<BTreeSet<_>>();
        let d = universe
            .difference(&a)
            .filter(|id| !b.contains(*id) && !c.contains(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        (a, b, c, d)
    }
    let (ss, us, bs, ms) = groups("study", &studies, &rows, &selected, &unresolved, &blocked);
    let (sm, um, bm, mm) = groups(
        "modality",
        &modalities,
        &rows,
        &selected,
        &unresolved,
        &blocked,
    );
    let (sx, ux, bx, mx) = groups("model", &models, &rows, &selected, &unresolved, &blocked);
    let globally_open = q.policy_allow
        && q.protected_closure
        && q.signed_approval
        && q.raw_data_local
        && q.aggregate_only
        && q.adversarial_event_order.is_empty();
    let admitted_or_unresolved = selected.len() + unresolved.len();
    let disp = if !globally_open
        || !blocked.is_empty()
        || !missing.is_empty()
        || !bs.is_empty()
        || !ms.is_empty()
        || !bm.is_empty()
        || !mm.is_empty()
        || !bx.is_empty()
        || !mx.is_empty()
        || admitted_or_unresolved < q.minimum_job_count as usize
        || ss.len() + us.len() < q.minimum_study_count as usize
        || sx.len() + ux.len() < q.minimum_model_count as usize
        || selected.len() > q.max_jobs as usize
    {
        AnalysisWorkbenchDisposition::Blocked
    } else if !unresolved.is_empty() || !us.is_empty() || !um.is_empty() || !ux.is_empty() {
        AnalysisWorkbenchDisposition::Unresolved
    } else {
        AnalysisWorkbenchDisposition::Qualified
    };
    let effects = if disp == AnalysisWorkbenchDisposition::Qualified {
        vec![format!("analyze:local-portfolio:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let reasons=vec![match disp{AnalysisWorkbenchDisposition::Qualified=>"all analysis identification, comparability, quality, policy, replay, provenance, and locality gates passed".into(),AnalysisWorkbenchDisposition::Unresolved=>"stale, uncertain, omitted, unknown, or replay-mismatched analysis remains unresolved".into(),AnalysisWorkbenchDisposition::Blocked=>"identification, comparability, quality, policy, coverage, authorization, or adversarial gates blocked analysis".into()}];
    let provenance = ContentHash::of_bytes(
        rows.iter()
            .map(|x| x.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"researcher":q.researcher,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"disposition":disp,"ranked_job_order":ranked,"selected_job_order":selected,"unresolved_job_order":unresolved,"blocked_job_order":blocked,"missing_job_order":missing,"study_order":studies,"selected_study_order":ss,"unresolved_study_order":us,"blocked_study_order":bs,"missing_study_order":ms,"modality_order":modalities,"selected_modality_order":sm,"unresolved_modality_order":um,"blocked_modality_order":bm,"missing_modality_order":mm,"model_order":models,"selected_model_order":sx,"unresolved_model_order":ux,"blocked_model_order":bx,"missing_model_order":mx,"uncertainty_order":uncertainty,"omission_order":omission,"negative_evidence_order":negative,"contradiction_order":contradiction,"adversarial_event_order":q.adversarial_event_order,"replay_identity":q.replay_identity,"provenance_digest":provenance,"reasons":reasons,"effect_receipts":effects,"raw_data_local":q.raw_data_local,"aggregate_only":q.aggregate_only,"autonomy_tier":AutonomyTier::A1,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("analysis-workbench:{}", q.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| AnalysisWorkbenchError::Artifact(e.to_string()))?;
    let digest = artifact.content_hash.clone();
    let receipt = AnalysisWorkbenchReceipt7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        researcher: q.researcher.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        disposition: disp,
        ranked_job_order: ranked,
        selected_job_order: selected.into_iter().collect(),
        unresolved_job_order: unresolved.into_iter().collect(),
        blocked_job_order: blocked.into_iter().collect(),
        missing_job_order: missing.into_iter().collect(),
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
        model_order: models.into_iter().collect(),
        selected_model_order: sx.into_iter().collect(),
        unresolved_model_order: ux.into_iter().collect(),
        blocked_model_order: bx.into_iter().collect(),
        missing_model_order: mx.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: q.adversarial_event_order.clone(),
        replay_identity: q.replay_identity.clone(),
        provenance_digest: provenance,
        reasons,
        analysis_digest: digest,
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
    fn job(id: &str, state: EvidenceState) -> AnalysisJob6 {
        AnalysisJob6 {
            job_id: id.into(),
            study_id: format!("study:{id}"),
            modality: "imaging".into(),
            model_id: format!("model:{id}"),
            estimand: "effect".into(),
            semantic_profile: "imaging-omics".into(),
            evidence_state: state,
            input_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("replay"),
            identification_supported: true,
            comparability_supported: true,
            quality_supported: true,
            policy_allowed: true,
            raw_data_local: true,
            aggregate_only: true,
            signed_approval: true,
            stale: false,
            revoked: false,
            negative_result: false,
            uncertainty_order: Vec::new(),
            omission_order: Vec::new(),
        }
    }
    fn q(items: Vec<AnalysisJob6>) -> AnalysisWorkbenchRequest5 {
        AnalysisWorkbenchRequest5 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            request_id: "analysis:1".into(),
            researcher: "operator".into(),
            purpose: "analysis".into(),
            semantic_profile: "imaging-omics".into(),
            required_job_order: vec!["job:1".into()],
            required_study_order: vec!["study:job:1".into()],
            required_modality_order: vec!["imaging".into()],
            required_model_order: vec!["model:job:1".into()],
            replay_identity: h("replay"),
            minimum_job_count: 1,
            minimum_study_count: 1,
            minimum_model_count: 1,
            max_jobs: 8,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            jobs: items,
        }
    }
    #[test]
    fn qualified_is_deterministic() {
        assert_eq!(
            qualify_analysis_workbench(&q(vec![job("job:1", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            AnalysisWorkbenchDisposition::Qualified
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        assert_eq!(
            qualify_analysis_workbench(&q(vec![job("job:1", EvidenceState::Unknown)]))
                .unwrap()
                .disposition,
            AnalysisWorkbenchDisposition::Unresolved
        )
    }
    #[test]
    fn contradiction_is_blocked() {
        assert_eq!(
            qualify_analysis_workbench(&q(vec![job("job:1", EvidenceState::Contradicted)]))
                .unwrap()
                .disposition,
            AnalysisWorkbenchDisposition::Blocked
        )
    }
    #[test]
    fn missing_job_is_blocked() {
        assert_eq!(
            qualify_analysis_workbench(&q(vec![job("other", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            AnalysisWorkbenchDisposition::Blocked
        )
    }
    #[test]
    fn negative_is_retained() {
        let mut x = job("job:1", EvidenceState::Supported);
        x.negative_result = true;
        assert_eq!(
            qualify_analysis_workbench(&q(vec![x]))
                .unwrap()
                .negative_evidence_order,
            vec!["job:1"]
        )
    }
    #[test]
    fn manifest_is_valid() {
        analysis_workbench_manifest().validate().unwrap()
    }
}
