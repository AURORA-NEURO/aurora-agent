//! Local single-study experiment-design research workbench (`AFA-ids-P09-F17`).
//!
//! Produces a deterministic, power-aware design frontier from caller-supplied design summaries.
//! It never enrolls subjects, schedules animals, controls instruments, or makes clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P09-F17";
pub const CONTRACT_VERSION: &str =
    "ids-local-single-study-experiment-design-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentDesignRequest4@1";
pub const OUTPUT_SCHEMA: &str = "DesignFrontier8@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.design-frontier-8+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesignEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignRequest4 {
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub minimum_power_milli: i64,
    pub required_controls: Vec<String>,
    pub checkpoint: u64,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignCandidate4 {
    pub design_id: String,
    pub estimand: String,
    pub study_id: String,
    pub origin: String,
    pub control_ids: Vec<String>,
    pub sample_size: usize,
    pub power_milli: i64,
    pub effect_milli: i64,
    pub semantic_profile: String,
    pub design_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: DesignEvidenceState,
    pub signed: bool,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignFrontier8Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignFrontier8 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_control_order: Vec<String>,
    pub underpowered_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub power_scores_milli: Vec<i64>,
    pub sample_sizes: Vec<usize>,
    pub replay_identity: ContentHash,
    pub frontier_digest: ContentHash,
    pub artifact: DesignFrontier8Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentDesignError {
    #[error("invalid experiment design request: {0}")]
    Invalid(String),
    #[error("experiment design artifact failed: {0}")]
    Artifact(String),
}

pub fn experiment_design_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["experimental neuroscientist","biostatistician","research workbench operator"],"behavior":"ranks typed power-aware preclinical design summaries into a deterministic design frontier","value":"makes design controls, power shortfalls, omissions, and uncertainty explicit before any laboratory action","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["manage:local-capability"],"permissions":["read:local-research-artifacts"],"autonomy_tier":"A0","boundary":PRECLINICAL_BOUNDARY})
}

impl DesignFrontier8 {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ExperimentDesignError::Invalid(
                "design identity, checkpoint, locality, candidates, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.frontier_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_control_order,
            &self.underpowered_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(ExperimentDesignError::Invalid(
                    "design ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.frontier_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<BTreeSet<_>>();
        if ids != parts || ids.len() != self.candidate_order.len() {
            return Err(ExperimentDesignError::Invalid(
                "design candidate states do not partition".into(),
            ));
        }
        if self.selected_order.len() + self.frontier_order.len() != self.power_scores_milli.len()
            || self.selected_order.len() + self.frontier_order.len() != self.sample_sizes.len()
            || self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.frontier_digest
        {
            return Err(ExperimentDesignError::Artifact(
                "design artifact, score cardinality, or digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| !e.starts_with("manage:local-capability:") && e != "block:unsafe-release")
        {
            return Err(ExperimentDesignError::Invalid(
                "effect is outside design gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ExperimentDesignError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?,
        )
        .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))
    }
}

pub fn design_experiment(
    request: &ExperimentDesignRequest4,
    candidates: &[DesignCandidate4],
) -> Result<DesignFrontier8, ExperimentDesignError> {
    validate_request(request, candidates)?;
    let mut rows = candidates.to_vec();
    rows.sort_by(|a, b| {
        b.power_milli
            .cmp(&a.power_milli)
            .then(b.effect_milli.cmp(&a.effect_milli))
            .then(a.design_id.cmp(&b.design_id))
    });
    let candidate_order = rows
        .iter()
        .map(|x| x.design_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut frontier = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_controls = BTreeSet::new();
    let mut underpowered = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for d in &rows {
        if d.negative_result {
            negative.insert(format!("{}:negative-result", d.design_id));
        }
        for r in &d.omission_reasons {
            omissions.insert(format!("{}:{}", d.design_id, r));
        }
        let missing = request
            .required_controls
            .iter()
            .filter(|x| !d.control_ids.contains(x))
            .count();
        if missing > 0 {
            missing_controls.insert(format!("{}:missing:{}", d.design_id, missing));
        }
        if d.power_milli < request.minimum_power_milli {
            underpowered.insert(d.design_id.clone());
        }
        let mut reasons = Vec::new();
        if d.study_id != request.study_id {
            reasons.push("study-mismatch");
        }
        if d.semantic_profile != request.semantic_profile {
            reasons.push("semantic-profile-mismatch");
        }
        if missing > 0 {
            reasons.push("control-closure-incomplete");
        }
        if d.power_milli < request.minimum_power_milli {
            reasons.push("power-threshold-failed");
        }
        if d.replay_identity != request.replay_identity {
            reasons.push("replay-identity-mismatch");
        }
        if !d.signed || !d.permitted {
            reasons.push("authorization-missing");
        }
        if !d.raw_data_local || !d.aggregate_only {
            reasons.push("locality-or-aggregate-only-failed");
        }
        if d.evidence_state == DesignEvidenceState::Contradicted {
            blocked.insert(d.design_id.clone());
            negative.insert(format!("{}:contradicted", d.design_id));
        } else if !matches!(
            d.evidence_state,
            DesignEvidenceState::Proven | DesignEvidenceState::Supported
        ) || !reasons.is_empty()
        {
            unresolved.insert(d.design_id.clone());
            uncertainty.insert(format!("{}:unresolved", d.design_id));
        } else if selected.is_empty() {
            selected.insert(d.design_id.clone());
        } else {
            frontier.insert(d.design_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    let disposition = if global || !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if global {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        frontier.clear();
        unresolved.clear();
    }
    if disposition != "qualified" {
        omissions.insert("request:design-gates-incomplete".into());
    }
    let rank = rows
        .iter()
        .map(|d| (d.design_id.clone(), (d.power_milli, d.sample_size)))
        .collect::<BTreeMap<_, _>>();
    let ranked_ids = selected
        .iter()
        .chain(&frontier)
        .cloned()
        .collect::<Vec<_>>();
    let powers = ranked_ids.iter().map(|id| rank[id].0).collect::<Vec<_>>();
    let sizes = ranked_ids.iter().map(|id| rank[id].1).collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"study_id":request.study_id,"requester":request.requester,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"checkpoint":request.checkpoint,"disposition":disposition,"candidate_order":candidate_order,"selected_order":selected,"frontier_order":frontier,"unresolved_order":unresolved,"blocked_order":blocked,"missing_control_order":missing_controls,"underpowered_order":underpowered,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"power_scores_milli":powers,"sample_sizes":sizes,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let frontier_digest = ContentHash::of_value(&payload)
        .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?;
    let artifact = DesignFrontier8Artifact {
        artifact_id: format!("design-frontier-8:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: frontier_digest.clone(),
        semantic_loss: Vec::new(),
        provenance_digests: rows
            .iter()
            .map(|x| x.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effects = if disposition == "qualified" {
        vec![format!("manage:local-capability:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = DesignFrontier8 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        frontier_order: payload["frontier_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        missing_control_order: payload["missing_control_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        underpowered_order: payload["underpowered_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_str().unwrap().into())
            .collect(),
        power_scores_milli: powers,
        sample_sizes: sizes,
        replay_identity: request.replay_identity.clone(),
        frontier_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: request.raw_data_local,
        aggregate_only: request.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ExperimentDesignRequest4,
    candidates: &[DesignCandidate4],
) -> Result<(), ExperimentDesignError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_controls.is_empty()
        || request.minimum_power_milli < 0
        || request.minimum_power_milli > 1000
        || request.checkpoint == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || candidates.is_empty()
    {
        return Err(ExperimentDesignError::Invalid("design identity, controls, power, checkpoint, budget, replay, locality, candidates, or boundary is invalid".into()));
    }
    let mut ids = BTreeSet::new();
    for d in candidates {
        if d.design_id.trim().is_empty()
            || d.estimand.trim().is_empty()
            || d.study_id.trim().is_empty()
            || d.origin.trim().is_empty()
            || d.design_digest.as_str().len() != 64
            || d.provenance_digest.as_str().len() != 64
            || d.replay_identity.as_str().len() != 64
            || !ids.insert(d.design_id.clone())
        {
            return Err(ExperimentDesignError::Invalid(
                "design identity, uniqueness, origin, estimand, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> ContentHash {
        ContentHash::of_bytes(s.as_bytes())
    }
    fn req() -> ExperimentDesignRequest4 {
        ExperimentDesignRequest4 {
            request_id: "request:design".into(),
            study_id: "study:1".into(),
            requester: "experimentalist".into(),
            purpose: "power-aware-design".into(),
            semantic_profile: "neuro:v1".into(),
            minimum_power_milli: 800,
            required_controls: vec!["vehicle".into()],
            checkpoint: 1,
            budget_units: 10,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn design(id: &str, state: DesignEvidenceState) -> DesignCandidate4 {
        DesignCandidate4 {
            design_id: id.into(),
            estimand: "effect".into(),
            study_id: "study:1".into(),
            origin: "site-a".into(),
            control_ids: vec!["vehicle".into()],
            sample_size: 12,
            power_milli: 900,
            effect_milli: 100,
            semantic_profile: "neuro:v1".into(),
            design_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("r"),
            evidence_state: state,
            signed: true,
            permitted: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_reasons: Vec::new(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(experiment_design_manifest()["autonomy_tier"], "A0");
    }
    #[test]
    fn qualified_is_replayable() {
        let r = design_experiment(
            &req(),
            &[
                design("b", DesignEvidenceState::Supported),
                design("a", DesignEvidenceState::Proven),
            ],
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert!(!r.frontier_order.is_empty());
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = design_experiment(&req(), &[design("a", DesignEvidenceState::Unknown)]).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn contradiction_blocks() {
        let r =
            design_experiment(&req(), &[design("a", DesignEvidenceState::Contradicted)]).unwrap();
        assert_eq!(r.disposition, "blocked");
    }
    #[test]
    fn underpowered_is_unresolved() {
        let mut d = design("a", DesignEvidenceState::Supported);
        d.power_milli = 100;
        let r = design_experiment(&req(), &[d]).unwrap();
        assert_eq!(r.disposition, "unresolved");
    }
    #[test]
    fn duplicate_is_rejected() {
        assert!(design_experiment(
            &req(),
            &[
                design("a", DesignEvidenceState::Supported),
                design("a", DesignEvidenceState::Supported)
            ]
        )
        .is_err());
    }
}
