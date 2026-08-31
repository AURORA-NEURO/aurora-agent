//! IDS contract-frontier assurance (`AFA-ids-P25-F27`).
//!
//! Admits prospective capability manifests only after deterministic identity,
//! evidence, effect, replay, policy, and locality checks. No endpoint is
//! loaded and no research data is moved.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P25-F27";
pub const CONTRACT_VERSION: &str =
    "ids-prospective-high-throughput-contract-frontier-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "IdsContractInput8@1";
pub const OUTPUT_SCHEMA: &str = "IdsCapabilityManifest9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-capability-manifest-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_INPUTS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsContractInput8 {
    pub contract_id: String,
    pub contract_family: String,
    pub version: String,
    pub interface_digest: ContentHash,
    pub manifest_digest: ContentHash,
    pub required_effects: Vec<String>,
    pub permissions: Vec<String>,
    pub migration_loss: Vec<String>,
    pub evidence_state: ContractEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsContractFrontierRequest7 {
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_contract_family: String,
    pub required_effects: Vec<String>,
    pub inputs: Vec<IdsContractInput8>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCapabilityManifest9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCapabilityManifest9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_contract_family: String,
    pub disposition: String,
    pub contract_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub migrated_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_effect_order: Vec<String>,
    pub incompatible_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub manifest_digest: ContentHash,
    pub artifact: IdsCapabilityManifest9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContractFrontierError {
    #[error("invalid contract frontier request: {0}")]
    Invalid(String),
    #[error("capability manifest failed validation: {0}")]
    Manifest(String),
}

pub fn contract_frontier_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["contract steward","SDK integrator","federation operator","release auditor"],"behavior":"assure prospective IDS capability manifests with effect, schema, evidence, replay, and policy gates","value":"prevents semantic drift, unsafe effects, and incomplete contracts from entering high-throughput research federation","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["exchange:capability-manifests","manage:local-capability"],"permissions":["read:local-contract-manifests","request:contract-frontier-assurance"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}
fn valid_digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
impl IdsCapabilityManifest9 {
    pub fn validate(&self) -> Result<(), ContractFrontierError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.required_contract_family.trim().is_empty()
            || self.contract_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ContractFrontierError::Manifest(
                "manifest identity, locality, contracts, effects, or disposition is incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.contract_order,
            &self.accepted_order,
            &self.migrated_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_effect_order,
            &self.incompatible_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(ContractFrontierError::Manifest(
                    "manifest ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.contract_order.iter().cloned());
        let parts = self
            .accepted_order
            .iter()
            .chain(&self.migrated_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.incompatible_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.contract_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(ContractFrontierError::Manifest(
                "contract states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.manifest_digest)
            || self.artifact.content_hash != self.manifest_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(ContractFrontierError::Manifest(
                "manifest digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:capability-manifests:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(ContractFrontierError::Manifest(
                "effect is outside governed contract frontier".into(),
            ));
        }
        Ok(())
    }
}
fn validate_request(r: &IdsContractFrontierRequest7) -> Result<(), ContractFrontierError> {
    if r.request_id.trim().is_empty()
        || r.purpose.trim().is_empty()
        || r.semantic_profile.trim().is_empty()
        || r.required_contract_family.trim().is_empty()
        || r.required_effects.is_empty()
        || r.inputs.is_empty()
        || r.inputs.len() > MAX_INPUTS
        || !valid_digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
    {
        return Err(ContractFrontierError::Invalid(
            "frontier identity, effects, input bound, replay, or locality is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for i in &r.inputs {
        if i.contract_id.trim().is_empty()
            || i.contract_family.trim().is_empty()
            || i.version.trim().is_empty()
            || !valid_digest(&i.interface_digest)
            || !valid_digest(&i.manifest_digest)
            || i.required_effects.is_empty()
            || i.permissions.is_empty()
            || !valid_digest(&i.provenance_digest)
            || !valid_digest(&i.replay_identity)
            || !ids.insert(i.contract_id.clone())
        {
            return Err(ContractFrontierError::Invalid("contract identity, version, effects, permissions, digest, or uniqueness is invalid".into()));
        }
    }
    Ok(())
}
pub fn assure_contract_frontier(
    r: &IdsContractFrontierRequest7,
) -> Result<IdsCapabilityManifest9, ContractFrontierError> {
    validate_request(r)?;
    let mut inputs = r.inputs.clone();
    inputs.sort_by(|a, b| a.contract_id.cmp(&b.contract_id));
    let order = inputs
        .iter()
        .map(|i| i.contract_id.clone())
        .collect::<Vec<_>>();
    let required = r.required_effects.iter().cloned().collect::<BTreeSet<_>>();
    let mut accepted = BTreeSet::new();
    let migrated: BTreeSet<String> = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut missing_effect = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut prov = BTreeSet::new();
    for i in &inputs {
        let id = i.contract_id.clone();
        if i.contract_family != r.required_contract_family {
            incompatible.insert(id.clone());
            omissions.insert(format!("{id}:contract-family"));
        } else if !i.local || !i.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-locality"));
        } else if i.replay_identity != r.replay_identity {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:replay-identity"));
        } else if i.evidence_state == ContractEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted"));
        } else if !matches!(
            i.evidence_state,
            ContractEvidenceState::Proven | ContractEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
        } else {
            let offered = i.required_effects.iter().cloned().collect::<BTreeSet<_>>();
            for e in required.difference(&offered) {
                missing_effect.insert(format!("{id}:{e}"));
            }
            if required.is_subset(&offered) {
                accepted.insert(id.clone());
                effects.extend(offered);
                prov.insert(i.provenance_digest.clone());
            } else {
                unresolved.insert(id.clone());
                omissions.insert(format!("{id}:missing-effect"));
            }
        }
    }
    let global = !r.policy_allow
        || !r.protected_closure
        || !r.signed_approval
        || !r.federation_approved
        || !r.raw_data_local
        || !r.aggregate_only;
    if global {
        blocked.extend(order.iter().cloned());
        accepted.clear();
        unresolved.clear();
        incompatible.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let ao = accepted.iter().cloned().collect::<Vec<_>>();
    let mo = migrated.iter().cloned().collect::<Vec<_>>();
    let uo = unresolved.iter().cloned().collect::<Vec<_>>();
    let bo = blocked.iter().cloned().collect::<Vec<_>>();
    let io = incompatible.iter().cloned().collect::<Vec<_>>();
    let disposition = if global || ao.is_empty() && mo.is_empty() && uo.is_empty() {
        "blocked"
    } else if !uo.is_empty() || !bo.is_empty() || !io.is_empty() || !missing_effect.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:contract-frontier-not-closed".into());
        effects.insert("block:unsafe-release".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r.request_id,"purpose":r.purpose,"semantic_profile":r.semantic_profile,"required_contract_family":r.required_contract_family,"disposition":disposition,"contract_order":order,"accepted_order":ao,"migrated_order":mo,"unresolved_order":uo,"blocked_order":bo,"missing_effect_order":missing_effect.iter().cloned().collect::<Vec<_>>(),"incompatible_order":io,"omission_order":omissions.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"effect_order":effects.iter().cloned().collect::<Vec<_>>(),"replay_identity":r.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let d = ContentHash::of_value(&payload)
        .map_err(|e| ContractFrontierError::Manifest(e.to_string()))?;
    payload["manifest_digest"] = json!(d);
    payload["artifact"] = json!({"artifact_id":format!("ids-capability-manifest-9:{}",r.request_id),"content_type":CONTENT_TYPE,"content_hash":d,"semantic_loss":omissions.iter().cloned().collect::<Vec<_>>(),"provenance_digests":prov.into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("exchange:capability-manifests:{}", r.request_id),
            format!("manage:local-capability:{}", r.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let out: IdsCapabilityManifest9 = serde_json::from_value(payload)
        .map_err(|e| ContractFrontierError::Manifest(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn i(id: &str) -> IdsContractInput8 {
        IdsContractInput8 {
            contract_id: id.into(),
            contract_family: "ids".into(),
            version: "1".into(),
            interface_digest: h(id),
            manifest_digest: h("m"),
            required_effects: vec!["exchange:capability-manifests".into()],
            permissions: vec!["read".into()],
            migration_loss: vec![],
            evidence_state: ContractEvidenceState::Supported,
            provenance_digest: h("p"),
            replay_identity: h("r"),
            local: true,
            aggregate_only: true,
        }
    }
    fn r(is: Vec<IdsContractInput8>) -> IdsContractFrontierRequest7 {
        IdsContractFrontierRequest7 {
            request_id: "frontier:req".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            required_contract_family: "ids".into(),
            required_effects: vec!["exchange:capability-manifests".into()],
            inputs: is,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(contract_frontier_manifest()["autonomy_tier"], "A1")
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            assure_contract_frontier(&r(vec![i("a")]))
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn missing_effect_is_unresolved() {
        let mut q = r(vec![i("a")]);
        q.required_effects.push("manage:local-capability".into());
        assert_eq!(
            assure_contract_frontier(&q).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn family_mismatch_is_blocked() {
        let mut x = i("a");
        x.contract_family = "other".into();
        assert_eq!(
            assure_contract_frontier(&r(vec![x])).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut x = i("a");
        x.evidence_state = ContractEvidenceState::Unknown;
        assert_eq!(
            assure_contract_frontier(&r(vec![x])).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = r(vec![i("a")]);
        q.policy_allow = false;
        assert_eq!(
            assure_contract_frontier(&q).unwrap().effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn order_is_canonical() {
        let x = assure_contract_frontier(&r(vec![i("z"), i("a")])).unwrap();
        assert_eq!(x.contract_order, vec!["a", "z"])
    }
}
