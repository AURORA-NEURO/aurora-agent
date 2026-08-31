//! Researcher/admin workbench contract (`AFA-ids-P24-F18`).
//!
//! Builds an omission-aware, digest-bound workspace view from caller-supplied
//! imaging and omics summaries. It never renders raw data, executes work, or
//! makes biological or clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P24-F18";
pub const CONTRACT_VERSION: &str = "ids-multimodal-researcher-admin-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ResearchWorkspaceState7@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveResearchWorkspace9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.interactive-research-workspace-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_PANELS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePanel8 {
    pub panel_id: String,
    pub study_id: String,
    pub modality: String,
    pub comparability_key: String,
    pub content_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: WorkspaceEvidenceState,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchWorkspaceState7 {
    pub request_id: String,
    pub workspace_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub comparability_key: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub panels: Vec<WorkspacePanel8>,
    pub selected_panel_limit: usize,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveResearchWorkspace9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InteractiveResearchWorkspace9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub comparability_key: String,
    pub disposition: String,
    pub panel_order: Vec<String>,
    pub selected_panel_order: Vec<String>,
    pub unresolved_panel_order: Vec<String>,
    pub blocked_panel_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub view_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub workspace_digest: ContentHash,
    pub artifact: InteractiveResearchWorkspace9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResearchWorkbenchError {
    #[error("invalid research workspace: {0}")]
    Invalid(String),
    #[error("research workspace failed validation: {0}")]
    Workspace(String),
}

pub fn research_workbench_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION, "owner_crate":"ids",
        "consumers":["preclinical researcher", "study administrator", "comparability reviewer", "provenance auditor"],
        "behavior":"compile a multimodal researcher/admin workspace view with explicit omissions, uncertainty, provenance, and locality",
        "value":"gives researchers an auditable cross-study workspace without hiding missing modalities, contradictory evidence, or protected data",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA, "effects":["view:research-workspace","manage:local-capability"],
        "permissions":["read:local-research-summaries","request:workspace-view"], "autonomy_tier":"A0", "boundary":PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

impl InteractiveResearchWorkspace9 {
    pub fn validate(&self) -> Result<(), ResearchWorkbenchError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.comparability_key.trim().is_empty()
            || self.panel_order.is_empty()
            || self.view_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ResearchWorkbenchError::Workspace("workspace identity, locality, panels, views, disposition, or effects are incomplete".into()));
        }
        for values in [
            &self.panel_order,
            &self.selected_panel_order,
            &self.unresolved_panel_order,
            &self.blocked_panel_order,
            &self.missing_study_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.view_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ResearchWorkbenchError::Workspace(
                    "workspace ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.panel_order.iter().cloned());
        let parts = self
            .selected_panel_order
            .iter()
            .chain(&self.unresolved_panel_order)
            .chain(&self.blocked_panel_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.panel_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(ResearchWorkbenchError::Workspace(
                "panel states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.workspace_digest)
            || self.artifact.content_hash != self.workspace_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(ResearchWorkbenchError::Workspace(
                "workspace digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("view:research-workspace:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(ResearchWorkbenchError::Workspace(
                "effect is outside governed workspace gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(r: &ResearchWorkspaceState7) -> Result<(), ResearchWorkbenchError> {
    if r.request_id.trim().is_empty()
        || r.workspace_id.trim().is_empty()
        || r.purpose.trim().is_empty()
        || r.semantic_profile.trim().is_empty()
        || r.comparability_key.trim().is_empty()
        || r.required_studies.is_empty()
        || r.required_modalities.is_empty()
        || r.panels.is_empty()
        || r.panels.len() > MAX_PANELS
        || r.selected_panel_limit == 0
        || !valid_digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(ResearchWorkbenchError::Invalid(
            "workspace identity, required closure, panel bound, replay, or locality is invalid"
                .into(),
        ));
    }
    if BTreeSet::from_iter(r.required_studies.iter().cloned()).len() != r.required_studies.len()
        || BTreeSet::from_iter(r.required_modalities.iter().cloned()).len()
            != r.required_modalities.len()
    {
        return Err(ResearchWorkbenchError::Invalid(
            "required studies or modalities are not unique".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for p in &r.panels {
        if p.panel_id.trim().is_empty()
            || p.study_id.trim().is_empty()
            || p.modality.trim().is_empty()
            || p.comparability_key.trim().is_empty()
            || !valid_digest(&p.content_digest)
            || !valid_digest(&p.provenance_digest)
            || !valid_digest(&p.replay_identity)
            || !ids.insert(p.panel_id.clone())
        {
            return Err(ResearchWorkbenchError::Invalid(
                "panel identity, comparability, digest, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn compile_research_workbench(
    r: &ResearchWorkspaceState7,
) -> Result<InteractiveResearchWorkspace9, ResearchWorkbenchError> {
    validate_request(r)?;
    let mut panels = r.panels.clone();
    panels.sort_by(|a, b| a.panel_id.cmp(&b.panel_id));
    let panel_order = panels
        .iter()
        .map(|p| p.panel_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let required_pairs = r
        .required_studies
        .iter()
        .flat_map(|s| {
            r.required_modalities
                .iter()
                .map(move |m| format!("{s}:{m}"))
        })
        .collect::<BTreeSet<_>>();
    let mut present = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for p in &panels {
        let id = p.panel_id.clone();
        if !r.required_studies.contains(&p.study_id) || !r.required_modalities.contains(&p.modality)
        {
            omissions.insert(format!("{id}:outside-required-closure"));
            continue;
        }
        present.insert(format!("{}:{}", p.study_id, p.modality));
        if !p.local || !p.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-locality"));
        } else if p.replay_identity != r.replay_identity {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:replay-identity"));
        } else if p.comparability_key != r.comparability_key {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:comparability"));
        } else {
            match p.evidence_state {
                WorkspaceEvidenceState::Contradicted => {
                    blocked.insert(id.clone());
                    negative.insert(format!("{id}:contradicted"));
                }
                WorkspaceEvidenceState::Unknown | WorkspaceEvidenceState::Unmeasured => {
                    unresolved.insert(id.clone());
                    uncertainty.insert(format!("{id}:evidence-state"));
                }
                WorkspaceEvidenceState::Proven | WorkspaceEvidenceState::Supported => {
                    provenance.insert(p.provenance_digest.clone());
                    if selected.len() < r.selected_panel_limit {
                        selected.insert(id);
                    } else {
                        omissions.insert(format!("{id}:selection-limit"));
                    }
                }
            }
        }
    }
    for pair in required_pairs.difference(&present) {
        if let Some((s, m)) = pair.split_once(':') {
            if !r.required_studies.contains(&s.to_string()) {
                omissions.insert(format!("{pair}:study"));
            } else if !r.required_modalities.contains(&m.to_string()) {
                omissions.insert(format!("{pair}:modality"));
            } else if r.required_studies.iter().any(|x| x == s) {
                omissions.insert(format!("{pair}:missing"));
            }
        }
    }
    let missing_studies = r
        .required_studies
        .iter()
        .filter(|s| !present.iter().any(|p| p.starts_with(&format!("{s}:"))))
        .map(|s| s.clone())
        .collect::<BTreeSet<_>>();
    let missing_modalities = r
        .required_modalities
        .iter()
        .filter(|m| !present.iter().any(|p| p.ends_with(&format!(":{m}"))))
        .map(|m| m.clone())
        .collect::<BTreeSet<_>>();
    if !missing_studies.is_empty() {
        uncertainty.insert("closure:study".into());
    }
    if !missing_modalities.is_empty() {
        uncertainty.insert("closure:modality".into());
    }
    let global = !r.policy_allow
        || !r.protected_closure
        || !r.signed_approval
        || !r.raw_data_local
        || !r.aggregate_only;
    if global {
        blocked.extend(panel_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let so = selected.iter().cloned().collect::<Vec<_>>();
    let uo = unresolved.iter().cloned().collect::<Vec<_>>();
    let bo = blocked.iter().cloned().collect::<Vec<_>>();
    let ms = missing_studies.iter().cloned().collect::<Vec<_>>();
    let mm = missing_modalities.iter().cloned().collect::<Vec<_>>();
    let disposition = if global || so.is_empty() && uo.is_empty() && bo.is_empty() {
        "blocked"
    } else if !uo.is_empty()
        || !bo.is_empty()
        || !ms.is_empty()
        || !mm.is_empty()
        || !omissions.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:workspace-not-closed".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r.request_id,"workspace_id":r.workspace_id,"purpose":r.purpose,"semantic_profile":r.semantic_profile,"comparability_key":r.comparability_key,"disposition":disposition,"panel_order":panel_order,"selected_panel_order":so,"unresolved_panel_order":uo,"blocked_panel_order":bo,"missing_study_order":ms,"missing_modality_order":mm,"omission_order":omissions.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"view_order":vec![format!("omissions:{}",r.workspace_id),format!("overview:{}",r.workspace_id),format!("provenance:{}",r.workspace_id)],"replay_identity":r.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ResearchWorkbenchError::Workspace(e.to_string()))?;
    payload["workspace_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("interactive-research-workspace-9:{}",r.workspace_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":omissions.iter().cloned().collect::<Vec<_>>(),"provenance_digests":provenance.into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("manage:local-capability:{}", r.request_id),
            format!("view:research-workspace:{}", r.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let out: InteractiveResearchWorkspace9 = serde_json::from_value(payload)
        .map_err(|e| ResearchWorkbenchError::Workspace(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn p(id: &str) -> WorkspacePanel8 {
        WorkspacePanel8 {
            panel_id: id.into(),
            study_id: "s1".into(),
            modality: "img".into(),
            comparability_key: "cmp".into(),
            content_digest: h(id),
            provenance_digest: h("p"),
            replay_identity: h("r"),
            evidence_state: WorkspaceEvidenceState::Supported,
            local: true,
            aggregate_only: true,
        }
    }
    fn r(ps: Vec<WorkspacePanel8>) -> ResearchWorkspaceState7 {
        ResearchWorkspaceState7 {
            request_id: "wb:req".into(),
            workspace_id: "wb:1".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            comparability_key: "cmp".into(),
            required_studies: vec!["s1".into()],
            required_modalities: vec!["img".into()],
            panels: ps,
            selected_panel_limit: 4,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(research_workbench_manifest()["autonomy_tier"], "A0")
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            compile_research_workbench(&r(vec![p("a")]))
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn missing_closure_is_unresolved() {
        let mut q = r(vec![p("a")]);
        q.required_modalities.push("rna".into());
        assert_eq!(
            compile_research_workbench(&q).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut x = p("a");
        x.evidence_state = WorkspaceEvidenceState::Unknown;
        assert_eq!(
            compile_research_workbench(&r(vec![x])).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn contradiction_is_blocked() {
        let mut x = p("a");
        x.evidence_state = WorkspaceEvidenceState::Contradicted;
        assert_eq!(
            compile_research_workbench(&r(vec![x])).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = r(vec![p("a")]);
        q.policy_allow = false;
        assert_eq!(
            compile_research_workbench(&q).unwrap().effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn panel_order_is_canonical() {
        let r = compile_research_workbench(&r(vec![p("z"), p("a")])).unwrap();
        assert_eq!(r.panel_order, vec!["a", "z"])
    }
}
