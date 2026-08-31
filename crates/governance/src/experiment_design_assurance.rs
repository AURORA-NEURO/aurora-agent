//! Federated continual experiment-design assurance gateway.
//!
//! Atlas feature: `AFA-governance-P09-F27`.
//! The gateway checks caller-supplied preclinical design attestations, power and factor closure,
//! and governance gates. It never schedules animals, consumes material, executes instruments, or
//! produces a clinical recommendation.

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

pub const FEATURE_ID: &str = "AFA-governance-P09-F27";
pub const CONTRACT_VERSION: &str =
    "governance-prospective-high-throughput-experiment-design-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective5@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign7@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignArm {
    pub arm_id: String,
    pub label: String,
    pub factor_order: Vec<String>,
    pub planned_n: u32,
    pub power_milli: u16,
    pub variance_milli: u32,
    pub evidence_state: EvidenceState,
    pub design_digest: ContentHash,
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
pub struct ExperimentObjective {
    pub request_id: String,
    pub federation_id: String,
    pub source_institution: String,
    pub target_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_arm_order: Vec<String>,
    pub required_factor_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub arms: Vec<DesignArm>,
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
pub struct ExperimentDesignAssurance {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub source_institution: String,
    pub target_institution: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub arm_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_arm_order: Vec<String>,
    pub missing_factor_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub design_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExperimentDesignError {
    #[error("invalid experiment design request: {0}")]
    Invalid(String),
    #[error("experiment design artifact failed: {0}")]
    Artifact(String),
}
fn invalid(v: impl Into<String>) -> ExperimentDesignError {
    ExperimentDesignError::Invalid(v.into())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64
}
pub fn experiment_design_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"governance".into(),consumers:["preclinical design scientist".into(),"power-analysis reviewer".into(),"federation governance steward".into()].into(),behavior:"assures typed federated experiment designs with factor, power, variance, baseline, provenance, replay, policy, and locality gates".into(),value:"prevents underpowered or incompletely evidenced designs from entering an approved research workflow while retaining omissions and negative evidence".into(),inputs:vec![TypedPort{name:"experiment_objective".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"executable_experiment_design".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::FederationExport,Effect::WriteLocalArtifact].into(),permissions:["approve:experiment-design".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:vec![AuthorityRequirement{role:"experiment design steward".into(),reason:"federated design approval requires institutional authorization".into()}],autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Cli,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Protocol,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
impl ExperimentDesignAssurance {
    pub fn validate(&self) -> Result<(), ExperimentDesignError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.source_institution == self.target_institution
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "unresolved" | "blocked"
            )
            || self.arm_order.is_empty()
            || self.ranked_order.len() != self.arm_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid("experiment assurance identity, locality, arms, disposition, or effects are incomplete"));
        }
        for v in [
            &self.arm_order,
            &self.qualified_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_arm_order,
            &self.missing_factor_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.contradiction_order,
            &self.negative_evidence_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(v) {
                return Err(invalid("experiment assurance ordering is not canonical"));
            }
        }
        let ids = self.arm_order.iter().collect::<BTreeSet<_>>();
        let p = self
            .qualified_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if p.len() != ids.len()
            || p.iter().any(|x| !ids.contains(x))
            || p.iter().collect::<BTreeSet<_>>().len() != p.len()
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != ids
        {
            return Err(invalid("experiment arm states do not partition arms"));
        }
        for d in [
            &self.baseline_digest,
            &self.replay_identity,
            &self.design_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(d) {
                return Err(invalid("experiment assurance digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?;
        if self.artifact.content_type != "application/vnd.aurora.executable-experiment-design+json"
        {
            return Err(invalid("experiment design artifact type is invalid"));
        }
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("approve:experiment-design:")
            {
                return Err(invalid("qualified experiment design effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid(
                "non-qualified experiment design must block release",
            ));
        }
        Ok(())
    }
}
pub fn assure_experiment_design(
    q: &ExperimentObjective,
) -> Result<ExperimentDesignAssurance, ExperimentDesignError> {
    validate(q)?;
    let mut arms = q.arms.clone();
    arms.sort_by(|a, b| {
        b.power_milli
            .cmp(&a.power_milli)
            .then(a.arm_id.cmp(&b.arm_id))
    });
    let ranked = arms.iter().map(|x| x.arm_id.clone()).collect::<Vec<_>>();
    let mut order = ranked.clone();
    order.sort();
    let req = q
        .required_arm_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = req
        .iter()
        .filter(|x| !arms.iter().any(|a| &a.arm_id == *x))
        .cloned()
        .collect::<Vec<_>>();
    let factors = q
        .required_factor_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for arm in &arms {
        let id = &arm.arm_id;
        if arm.negative_result {
            negative.insert(format!("{id}:negative-result"));
        }
        omissions.extend(arm.omissions.iter().map(|x| format!("{id}:{x}")));
        uncertainty.extend(arm.uncertainty.iter().map(|x| format!("{id}:{x}")));
        if arm.evidence_state == EvidenceState::Contradicted {
            blocked.insert(id.clone());
            contradiction.insert(format!("{id}:contradicted-evidence"));
            continue;
        }
        if matches!(
            arm.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-unresolved"));
            continue;
        }
        let have = arm.factor_order.iter().cloned().collect::<BTreeSet<_>>();
        let ok = !arm.label.trim().is_empty()
            && arm.planned_n > 0
            && arm.power_milli >= 800
            && arm.variance_milli > 0
            && factors.is_subset(&have)
            && arm.semantic_profile == q.semantic_profile
            && arm.replay_identity == q.replay_identity
            && arm.provenance_digest.is_some()
            && digest(&arm.design_digest)
            && arm.omissions.is_empty()
            && arm.uncertainty.is_empty()
            && arm.local_data
            && arm.permitted;
        if ok
            && matches!(
                arm.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
        {
            qualified.insert(id.clone());
        } else {
            unresolved.insert(id.clone());
            if arm.planned_n == 0 {
                omissions.insert(format!("{id}:planned-sample-missing"));
            }
            if arm.power_milli < 800 {
                uncertainty.insert(format!("{id}:power-threshold-not-met"));
            }
            if arm.variance_milli == 0 {
                omissions.insert(format!("{id}:variance-missing"));
            }
            if !factors.is_subset(&have) {
                omissions.insert(format!("{id}:factor-closure-incomplete"));
            }
            if arm.provenance_digest.is_none() {
                omissions.insert(format!("{id}:provenance-missing"));
            }
            if !arm.local_data || !arm.permitted {
                blocked.insert(id.clone());
                unresolved.remove(id);
                omissions.insert(format!("{id}:locality-or-permission-denied"));
            }
        }
    }
    for id in &missing {
        omissions.insert(format!("{id}:required-arm-missing"));
    }
    let missing_factor = q
        .required_factor_order
        .iter()
        .filter(|f| !arms.iter().any(|a| a.factor_order.contains(f)))
        .cloned()
        .collect::<Vec<_>>();
    for f in &missing_factor {
        omissions.insert(format!("required-factor-missing:{f}"));
    }
    negative.extend(
        q.adversarial_events
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let global = !q.policy_allow
        || !q.protected_closure
        || !q.signed_approval
        || !q.federation_approved
        || !q.raw_data_local
        || !q.aggregate_only
        || q.budget > q.max_budget
        || !q.adversarial_events.is_empty();
    if !q.policy_allow {
        uncertainty.insert("request:policy-denied".into());
    }
    if !q.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !q.signed_approval || !q.federation_approved {
        uncertainty.insert("request:institutional-approval-incomplete".into());
    }
    if q.budget > q.max_budget {
        omissions.insert("request:budget-ceiling-exceeded".into());
    }
    let disposition = if global {
        "blocked"
    } else if missing.is_empty()
        && missing_factor.is_empty()
        && !qualified.is_empty()
        && unresolved.is_empty()
        && blocked.is_empty()
    {
        "qualified"
    } else {
        "unresolved"
    };
    let qualified = qualified.into_iter().collect::<Vec<_>>();
    let unresolved = unresolved.into_iter().collect::<Vec<_>>();
    let blocked = blocked.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let contradiction = contradiction.into_iter().collect::<Vec<_>>();
    let negative = negative.into_iter().collect::<Vec<_>>();
    let effects = if disposition == "qualified" {
        vec![format!("approve:experiment-design:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":q.request_id,"federation_id":q.federation_id,"source_institution":q.source_institution,"target_institution":q.target_institution,"purpose":q.purpose,"semantic_profile":q.semantic_profile,"disposition":disposition,"arm_order":order,"ranked_order":ranked,"qualified_order":qualified,"unresolved_order":unresolved,"blocked_order":blocked,"missing_arm_order":missing,"missing_factor_order":missing_factor,"omission_order":omissions,"uncertainty_order":uncertainty,"contradiction_order":contradiction,"negative_evidence_order":negative,"adversarial_event_order":q.adversarial_events,"baseline_digest":q.baseline_digest,"replay_identity":q.replay_identity,"effect_receipts":effects,"raw_data_local":q.raw_data_local,"aggregate_only":q.aggregate_only,"boundary":PRECLINICAL_BOUNDARY});
    let design_digest = ContentHash::of_value(&payload)
        .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("governance-experiment:{}", q.request_id),
        "application/vnd.aurora.executable-experiment-design+json",
        &payload,
        Vec::new(),
        vec![ProvenanceLink {
            source_id: format!("federation:{}", q.federation_id),
            relation: "derived-from-local-design-manifest".into(),
            digest: q.replay_identity.clone(),
        }],
    )
    .map_err(|e| ExperimentDesignError::Artifact(e.to_string()))?;
    let out = ExperimentDesignAssurance {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: q.request_id.clone(),
        federation_id: q.federation_id.clone(),
        source_institution: q.source_institution.clone(),
        target_institution: q.target_institution.clone(),
        purpose: q.purpose.clone(),
        semantic_profile: q.semantic_profile.clone(),
        disposition: disposition.into(),
        arm_order: order,
        ranked_order: ranked,
        qualified_order: qualified,
        unresolved_order: unresolved,
        blocked_order: blocked,
        missing_arm_order: missing,
        missing_factor_order: missing_factor,
        omission_order: omissions,
        uncertainty_order: uncertainty,
        contradiction_order: contradiction,
        negative_evidence_order: negative,
        adversarial_event_order: q.adversarial_events.clone(),
        baseline_digest: q.baseline_digest.clone(),
        replay_identity: q.replay_identity.clone(),
        design_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: q.raw_data_local,
        aggregate_only: q.aggregate_only,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}
fn validate(q: &ExperimentObjective) -> Result<(), ExperimentDesignError> {
    if q.request_id.trim().is_empty()
        || q.federation_id.trim().is_empty()
        || q.source_institution.trim().is_empty()
        || q.target_institution.trim().is_empty()
        || q.source_institution == q.target_institution
        || q.purpose.trim().is_empty()
        || q.semantic_profile.trim().is_empty()
        || q.required_arm_order.is_empty()
        || q.required_factor_order.is_empty()
        || q.arms.is_empty()
        || !canonical(&q.required_arm_order)
        || !canonical(&q.required_factor_order)
        || !canonical(&q.adversarial_events)
        || !digest(&q.baseline_digest)
        || !digest(&q.replay_identity)
        || q.budget == 0
        || q.max_budget == 0
        || q.boundary != PRECLINICAL_BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
    {
        return Err(invalid("experiment objective identity, requirements, digests, budget, locality, or boundary is invalid"));
    }
    let mut seen = BTreeSet::new();
    for arm in &q.arms {
        if arm.arm_id.trim().is_empty()
            || arm.label.trim().is_empty()
            || !seen.insert(arm.arm_id.clone())
            || arm.factor_order.is_empty()
            || !canonical(&arm.factor_order)
            || arm.power_milli > 1000
            || !digest(&arm.design_digest)
            || arm.provenance_digest.as_ref().is_some_and(|d| !digest(d))
            || !digest(&arm.replay_identity)
            || arm.semantic_profile.trim().is_empty()
            || !canonical(&arm.omissions)
            || !canonical(&arm.uncertainty)
        {
            return Err(invalid(format!(
                "arm {} is malformed or duplicated",
                arm.arm_id
            )));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn arm(id: &str, state: EvidenceState, p: u16) -> DesignArm {
        DesignArm {
            arm_id: id.into(),
            label: format!("arm-{id}"),
            factor_order: vec!["dose".into(), "time".into()],
            planned_n: 20,
            power_milli: p,
            variance_milli: 100,
            evidence_state: state,
            design_digest: h(&format!("design-{id}")),
            provenance_digest: Some(h(&format!("prov-{id}"))),
            replay_identity: h("replay"),
            semantic_profile: "preclinical-neural".into(),
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
            local_data: true,
            permitted: true,
        }
    }
    fn q(arms: Vec<DesignArm>) -> ExperimentObjective {
        ExperimentObjective {
            request_id: "request-1".into(),
            federation_id: "fed-1".into(),
            source_institution: "site-a".into(),
            target_institution: "site-b".into(),
            purpose: "design-approval".into(),
            semantic_profile: "preclinical-neural".into(),
            required_arm_order: vec!["arm-a".into()],
            required_factor_order: vec!["dose".into(), "time".into()],
            baseline_digest: h("baseline"),
            arms,
            replay_identity: h("replay"),
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
    fn manifest() {
        let m = experiment_design_assurance_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
        m.validate().unwrap()
    }
    #[test]
    fn qualified() {
        assert_eq!(
            assure_experiment_design(&q(vec![arm("arm-a", EvidenceState::Supported, 900)]))
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn underpowered_unresolved() {
        let x = assure_experiment_design(&q(vec![arm("arm-a", EvidenceState::Supported, 700)]))
            .unwrap();
        assert_eq!(x.disposition, "unresolved")
    }
    #[test]
    fn unknown_and_contradicted() {
        let mut x = q(vec![
            arm("arm-a", EvidenceState::Unknown, 900),
            arm("arm-b", EvidenceState::Contradicted, 900),
        ]);
        let o = assure_experiment_design(&x).unwrap();
        assert!(o.unresolved_order.contains(&"arm-a".into()));
        assert!(o.blocked_order.contains(&"arm-b".into()));
        x.required_arm_order = vec!["arm-a".into()];
    }
    #[test]
    fn adversarial_blocks() {
        let mut x = q(vec![arm("arm-a", EvidenceState::Supported, 900)]);
        x.adversarial_events = vec!["poisoned-baseline".into()];
        assert_eq!(assure_experiment_design(&x).unwrap().disposition, "blocked")
    }
    #[test]
    fn duplicate_rejected() {
        let x = q(vec![
            arm("arm-a", EvidenceState::Supported, 900),
            arm("arm-a", EvidenceState::Supported, 800),
        ]);
        assert!(matches!(
            assure_experiment_design(&x),
            Err(ExperimentDesignError::Invalid(_))
        ))
    }
    #[test]
    fn deterministic() {
        let a = assure_experiment_design(&q(vec![
            arm("arm-b", EvidenceState::Supported, 700),
            arm("arm-a", EvidenceState::Supported, 900),
        ]))
        .unwrap();
        let b = assure_experiment_design(&q(vec![
            arm("arm-a", EvidenceState::Supported, 900),
            arm("arm-b", EvidenceState::Supported, 700),
        ]))
        .unwrap();
        assert_eq!(a.ranked_order, b.ranked_order);
        assert_eq!(a.design_digest, b.design_digest)
    }
}
