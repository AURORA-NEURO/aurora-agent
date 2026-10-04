//! Policy P32 frontier: validate bounded, expiring, and revocable autonomy grants.
use bioprism_foundation::AutonomyTier;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
pub const CONTRACT_VERSION: &str = "policy-local-grant-integrity/2.0";
pub const OUTPUT_SCHEMA_VERSION: &str = "2.0.0";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.policy.grant-integrity-card-2+json";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutonomyGrant4 {
    pub grant_id: String,
    pub actor: String,
    pub action: String,
    pub scope: String,
    pub autonomy_tier: String,
    pub expiration_epoch: u64,
    pub policy_epoch: u64,
    pub revoked: bool,
    pub evidence_digest: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantIntegrityRequest4 {
    pub request_id: String,
    pub purpose: String,
    pub grants: Vec<AutonomyGrant4>,
    pub required_grant_order: Vec<String>,
    pub required_action_order: Vec<String>,
    pub current_epoch: u64,
    pub maximum_tier: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub replay_identity: ContentHash,
    pub adversarial_events: Vec<String>,
    pub action_budget: u32,
    pub action_count: u32,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    /// Evidence identity keyed by grant so provenance remains attributable.
    pub evidence_digests: BTreeMap<String, ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrantIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub grant_order: Vec<String>,
    pub required_grant_order: Vec<String>,
    pub required_action_order: Vec<String>,
    pub current_epoch: u64,
    pub maximum_tier: String,
    pub actor_by_grant: BTreeMap<String, String>,
    pub action_by_grant: BTreeMap<String, String>,
    pub scope_by_grant: BTreeMap<String, String>,
    pub tier_by_grant: BTreeMap<String, String>,
    pub expiration_epoch_by_grant: BTreeMap<String, u64>,
    pub policy_epoch_by_grant: BTreeMap<String, u64>,
    pub revoked_by_grant: BTreeMap<String, bool>,
    pub allowed_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub action_order: Vec<String>,
    pub scope_order: Vec<String>,
    pub tier_order: Vec<String>,
    pub expiration_order: Vec<String>,
    pub revocation_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: GrantIntegrityArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GrantIntegrityError {
    #[error("invalid grant-integrity request: {0}")]
    Invalid(String),
    #[error("grant-integrity card failed validation: {0}")]
    Output(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn ordered_nonempty(v: &[String]) -> bool {
    ordered(v) && v.iter().all(|value| nonempty(value))
}
fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn nonempty(v: &str) -> bool {
    !v.trim().is_empty()
}
fn autonomy_tier(value: &str) -> Option<AutonomyTier> {
    match value.trim().to_ascii_lowercase().as_str() {
        "a0" => Some(AutonomyTier::A0),
        "a1" => Some(AutonomyTier::A1),
        "a2" => Some(AutonomyTier::A2),
        "a3" => Some(AutonomyTier::A3),
        "a4" => Some(AutonomyTier::A4),
        _ => None,
    }
}
pub fn manifest(id: &str, version: &str, scale: &str, mode: &str) -> serde_json::Value {
    json!({"schema_version":"1.0.0","capability_id":id,"version":version,"owner_crate":"policy","consumers":["autonomy broker","instrument gateway","federation admission","release auditor"],"behavior":format!("qualify expiring autonomy grants at {scale} ({mode})"),"value":"prevents revoked, expired, over-scoped, or over-tier grants from authorizing research effects","input_schema":"GrantIntegrityRequest4@1","output_schema":"GrantIntegrityCard7@2","content_binding":"canonical card body excluding closure_digest and artifact.content_hash","effects":["emit:grant-card","retain:revocation","block:unauthorized-effect"],"permissions":["read:local-policy"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn content_digest(value: &Value) -> Result<ContentHash, GrantIntegrityError> {
    let mut body = value.clone();
    let object = body.as_object_mut().ok_or_else(|| {
        GrantIntegrityError::Output("grant integrity card is not a JSON object".into())
    })?;
    object.remove("closure_digest");
    if let Some(artifact) = object.get_mut("artifact").and_then(Value::as_object_mut) {
        artifact.remove("content_hash");
    }
    ContentHash::of_value(&body).map_err(|error| GrantIntegrityError::Output(error.to_string()))
}
impl GrantIntegrityCard7 {
    pub fn validate(&self) -> Result<(), GrantIntegrityError> {
        if self.schema_version != OUTPUT_SCHEMA_VERSION
            || !nonempty(&self.contract_version)
            || !nonempty(&self.feature_id)
            || !nonempty(&self.mode)
            || !nonempty(&self.scale)
            || !nonempty(&self.request_id)
            || !nonempty(&self.purpose)
            || self.boundary != BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.grant_order.is_empty()
            || !ordered_nonempty(&self.grant_order)
            || !ordered_nonempty(&self.required_grant_order)
            || !ordered_nonempty(&self.required_action_order)
            || !ordered_nonempty(&self.allowed_order)
            || !ordered_nonempty(&self.denied_order)
            || !ordered_nonempty(&self.unknown_order)
            || !ordered_nonempty(&self.omitted_order)
            || !ordered_nonempty(&self.action_order)
            || !ordered_nonempty(&self.scope_order)
            || !ordered_nonempty(&self.tier_order)
            || !ordered_nonempty(&self.expiration_order)
            || !ordered_nonempty(&self.revocation_order)
            || autonomy_tier(&self.maximum_tier).is_none()
            || !digest(&self.replay_identity)
            || !digest(&self.closure_digest)
            || !nonempty(&self.artifact.artifact_id)
            || self.artifact.content_type != CONTENT_TYPE
            || !digest(&self.artifact.content_hash)
            || self.artifact.content_hash != self.closure_digest
            || self.artifact.semantic_loss != self.omitted_order
            || self.artifact.boundary != BOUNDARY
            || self.effect_receipts.len() != 1
        {
            return Err(GrantIntegrityError::Output(
                "grant identity, ordering, locality, digest, or artifact is invalid".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.grant_order.iter().cloned());
        let mapping_keys = |keys: BTreeSet<String>| keys == ids;
        if ids.len() != self.grant_order.len()
            || !mapping_keys(self.actor_by_grant.keys().cloned().collect())
            || !mapping_keys(self.action_by_grant.keys().cloned().collect())
            || !mapping_keys(self.scope_by_grant.keys().cloned().collect())
            || !mapping_keys(self.tier_by_grant.keys().cloned().collect())
            || !mapping_keys(self.expiration_epoch_by_grant.keys().cloned().collect())
            || !mapping_keys(self.policy_epoch_by_grant.keys().cloned().collect())
            || !mapping_keys(self.revoked_by_grant.keys().cloned().collect())
            || !mapping_keys(self.artifact.evidence_digests.keys().cloned().collect())
            || self.actor_by_grant.values().any(|value| !nonempty(value))
            || self.action_by_grant.values().any(|value| !nonempty(value))
            || self.scope_by_grant.values().any(|value| !nonempty(value))
            || self
                .tier_by_grant
                .values()
                .any(|value| autonomy_tier(value).is_none())
            || self
                .artifact
                .evidence_digests
                .values()
                .any(|value| !digest(value))
            || self.artifact.artifact_id != format!("policy-grant:{}", self.request_id)
        {
            return Err(GrantIntegrityError::Output(
                "grant provenance mapping or artifact identity is invalid".into(),
            ));
        }
        if BTreeSet::from_iter(self.action_by_grant.values().cloned())
            != BTreeSet::from_iter(self.action_order.iter().cloned())
            || BTreeSet::from_iter(self.scope_by_grant.values().cloned())
                != BTreeSet::from_iter(self.scope_order.iter().cloned())
            || BTreeSet::from_iter(self.tier_by_grant.values().cloned())
                != BTreeSet::from_iter(self.tier_order.iter().cloned())
            || BTreeSet::from_iter(
                self.expiration_epoch_by_grant
                    .iter()
                    .map(|(grant, epoch)| format!("{grant}:{epoch}")),
            ) != BTreeSet::from_iter(self.expiration_order.iter().cloned())
            || self.revocation_order
                != self
                    .revoked_by_grant
                    .iter()
                    .filter(|(_, revoked)| **revoked)
                    .map(|(grant, _)| format!("{grant}:revoked"))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
        {
            return Err(GrantIntegrityError::Output(
                "grant summaries do not match their per-grant mappings".into(),
            ));
        }
        let states = self
            .allowed_order
            .iter()
            .chain(&self.denied_order)
            .chain(&self.unknown_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.grant_order.len()
            || states.len() != ids.len()
            || BTreeSet::from_iter(states) != ids
        {
            return Err(GrantIntegrityError::Output(
                "grant states do not partition".into(),
            ));
        }
        let maximum_tier = autonomy_tier(&self.maximum_tier).ok_or_else(|| {
            GrantIntegrityError::Output("maximum autonomy tier is invalid".into())
        })?;
        let has_unknown = |grant: &str| {
            self.policy_epoch_by_grant[grant] == 0
                || self.artifact.evidence_digests[grant] == self.replay_identity
        };
        let denied_by_policy = |grant: &str| {
            self.revoked_by_grant[grant]
                || self.expiration_epoch_by_grant[grant] <= self.current_epoch
                || autonomy_tier(&self.tier_by_grant[grant]).is_some_and(|tier| tier > maximum_tier)
                || !self
                    .required_action_order
                    .contains(&self.action_by_grant[grant])
        };
        if self.allowed_order.iter().any(|grant| {
            !self.required_grant_order.contains(grant)
                || has_unknown(grant)
                || denied_by_policy(grant)
        }) || self.unknown_order.iter().any(|grant| !has_unknown(grant))
            || self
                .denied_order
                .iter()
                .any(|grant| has_unknown(grant) || !denied_by_policy(grant))
            || (self.disposition != "blocked"
                && self.omitted_order.iter().any(|grant| {
                    self.required_grant_order.contains(grant)
                        || has_unknown(grant)
                        || denied_by_policy(grant)
                }))
        {
            return Err(GrantIntegrityError::Output(
                "grant states do not match expiry, tier, evidence, action, or revocation policy"
                    .into(),
            ));
        }
        let missing = !self
            .required_grant_order
            .iter()
            .all(|grant| ids.contains(grant))
            || !self
                .required_action_order
                .iter()
                .all(|action| self.action_order.contains(action));
        let expected_effect = if self.disposition == "qualified" {
            format!("emit:grant-card:{}", self.request_id)
        } else {
            "block:unauthorized-effect".to_string()
        };
        if self.effect_receipts != [expected_effect]
            || !["qualified", "partial", "unknown", "blocked"].contains(&self.disposition.as_str())
            || (self.disposition == "blocked"
                && (self.omitted_order != self.grant_order
                    || !self.allowed_order.is_empty()
                    || !self.denied_order.is_empty()
                    || !self.unknown_order.is_empty()))
            || (self.disposition == "qualified"
                && (missing
                    || self.allowed_order != self.grant_order
                    || !self.denied_order.is_empty()
                    || !self.unknown_order.is_empty()
                    || !self.omitted_order.is_empty()))
            || (self.disposition == "unknown" && !missing)
            || (self.disposition == "partial"
                && (missing || self.allowed_order == self.grant_order))
        {
            return Err(GrantIntegrityError::Output(
                "disposition, grant states, or effect receipt is inconsistent".into(),
            ));
        }
        let value = serde_json::to_value(self)
            .map_err(|error| GrantIntegrityError::Output(error.to_string()))?;
        if content_digest(&value)? != self.closure_digest {
            return Err(GrantIntegrityError::Output(
                "closure digest does not bind the grant card body".into(),
            ));
        }
        Ok(())
    }
}
pub fn qualify(
    request: &GrantIntegrityRequest4,
    id: &str,
    version: &str,
    scale: &str,
    mode: &str,
) -> Result<GrantIntegrityCard7, GrantIntegrityError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.purpose)
        || !nonempty(id)
        || !nonempty(version)
        || !nonempty(scale)
        || !nonempty(mode)
        || request.grants.is_empty()
        || request.required_grant_order.is_empty()
        || request.required_action_order.is_empty()
        || autonomy_tier(&request.maximum_tier).is_none()
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !ordered_nonempty(&request.required_grant_order)
        || !ordered_nonempty(&request.required_action_order)
        || !ordered_nonempty(&request.adversarial_events)
    {
        return Err(GrantIntegrityError::Invalid("grant identity, requirements, tier, digest, ordering, locality, or boundary is invalid".into()));
    }
    let maximum_tier = autonomy_tier(&request.maximum_tier)
        .ok_or_else(|| GrantIntegrityError::Invalid("maximum autonomy tier is invalid".into()))?;
    let mut rows = request.grants.clone();
    rows.sort_by(|a, b| a.grant_id.cmp(&b.grant_id));
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut allowed = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut actions = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    let mut tiers = BTreeSet::new();
    let mut expirations = BTreeSet::new();
    let mut revocations = BTreeSet::new();
    let mut actors = BTreeMap::new();
    let mut actions_by_grant = BTreeMap::new();
    let mut scopes_by_grant = BTreeMap::new();
    let mut tiers_by_grant = BTreeMap::new();
    let mut expiration_by_grant = BTreeMap::new();
    let mut policy_epoch_by_grant = BTreeMap::new();
    let mut revoked_by_grant = BTreeMap::new();
    let mut digests = BTreeMap::new();
    for g in &rows {
        if !seen.insert(g.grant_id.clone())
            || !nonempty(&g.grant_id)
            || !nonempty(&g.actor)
            || !nonempty(&g.action)
            || !nonempty(&g.scope)
            || !nonempty(&g.autonomy_tier)
            || autonomy_tier(&g.autonomy_tier).is_none()
            || !digest(&g.evidence_digest)
            || !g.local
            || !g.aggregate_only
        {
            return Err(GrantIntegrityError::Invalid(
                "grant identity, actor, scope, tier, evidence, or locality is invalid".into(),
            ));
        }
        order.push(g.grant_id.clone());
        actions.insert(g.action.clone());
        scopes.insert(g.scope.clone());
        tiers.insert(g.autonomy_tier.clone());
        expirations.insert(format!("{}:{}", g.grant_id, g.expiration_epoch));
        actors.insert(g.grant_id.clone(), g.actor.clone());
        actions_by_grant.insert(g.grant_id.clone(), g.action.clone());
        scopes_by_grant.insert(g.grant_id.clone(), g.scope.clone());
        tiers_by_grant.insert(g.grant_id.clone(), g.autonomy_tier.clone());
        expiration_by_grant.insert(g.grant_id.clone(), g.expiration_epoch);
        policy_epoch_by_grant.insert(g.grant_id.clone(), g.policy_epoch);
        revoked_by_grant.insert(g.grant_id.clone(), g.revoked);
        digests.insert(g.grant_id.clone(), g.evidence_digest.clone());
        if g.revoked {
            revocations.insert(format!("{}:revoked", g.grant_id));
        }
        if g.policy_epoch == 0 || g.evidence_digest == request.replay_identity {
            unknown.insert(g.grant_id.clone());
        } else if g.revoked
            || g.expiration_epoch <= request.current_epoch
            || autonomy_tier(&g.autonomy_tier).is_some_and(|tier| tier > maximum_tier)
            || !request.required_action_order.contains(&g.action)
        {
            denied.insert(g.grant_id.clone());
        } else if !request.required_grant_order.contains(&g.grant_id) {
            omitted.insert(g.grant_id.clone());
        } else {
            allowed.insert(g.grant_id.clone());
        }
    }
    let global = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || request.action_count > request.action_budget;
    if global {
        omitted.extend(order.iter().cloned());
        allowed.clear();
        denied.clear();
        unknown.clear();
    }
    let missing = !request
        .required_grant_order
        .iter()
        .all(|g| seen.contains(g))
        || !request
            .required_action_order
            .iter()
            .all(|a| actions.contains(a));
    let disposition = if global {
        "blocked"
    } else if missing {
        "unknown"
    } else if !denied.is_empty() || !unknown.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let mut payload = json!({"schema_version":OUTPUT_SCHEMA_VERSION,"contract_version":version,"feature_id":id,"mode":mode,"scale":scale,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"grant_order":order,"required_grant_order":request.required_grant_order,"required_action_order":request.required_action_order,"current_epoch":request.current_epoch,"maximum_tier":request.maximum_tier,"actor_by_grant":actors,"action_by_grant":actions_by_grant,"scope_by_grant":scopes_by_grant,"tier_by_grant":tiers_by_grant,"expiration_epoch_by_grant":expiration_by_grant,"policy_epoch_by_grant":policy_epoch_by_grant,"revoked_by_grant":revoked_by_grant,"allowed_order":allowed.into_iter().collect::<Vec<_>>(),"denied_order":denied.into_iter().collect::<Vec<_>>(),"unknown_order":unknown.into_iter().collect::<Vec<_>>(),"omitted_order":omitted.into_iter().collect::<Vec<_>>(),"action_order":actions.into_iter().collect::<Vec<_>>(),"scope_order":scopes.into_iter().collect::<Vec<_>>(),"tier_order":tiers.into_iter().collect::<Vec<_>>(),"expiration_order":expirations.into_iter().collect::<Vec<_>>(),"revocation_order":revocations.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    payload["artifact"] = json!({"artifact_id":format!("policy-grant:{}",request.request_id),"content_type":CONTENT_TYPE,"semantic_loss":payload["omitted_order"],"evidence_digests":digests,"boundary":BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("emit:grant-card:{}", request.request_id)]
    } else {
        vec!["block:unauthorized-effect".to_string()]
    });
    let hash = content_digest(&payload)?;
    payload["closure_digest"] = json!(hash);
    payload["artifact"]["content_hash"] = json!(hash);
    let out: GrantIntegrityCard7 =
        serde_json::from_value(payload).map_err(|e| GrantIntegrityError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn grant(grant_id: &str, action: &str, tier: &str) -> AutonomyGrant4 {
        AutonomyGrant4 {
            grant_id: grant_id.into(),
            actor: "agent:local".into(),
            action: action.into(),
            scope: "study:local".into(),
            autonomy_tier: tier.into(),
            expiration_epoch: 20,
            policy_epoch: 1,
            revoked: false,
            evidence_digest: hash(&format!("evidence:{grant_id}")),
            local: true,
            aggregate_only: true,
        }
    }

    fn request(grants: Vec<AutonomyGrant4>) -> GrantIntegrityRequest4 {
        GrantIntegrityRequest4 {
            request_id: "request:1".into(),
            purpose: "authorize a bounded local research effect".into(),
            grants,
            required_grant_order: vec!["grant-a".into()],
            required_action_order: vec!["compute".into()],
            current_epoch: 10,
            maximum_tier: "A3".into(),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            replay_identity: hash("replay"),
            adversarial_events: Vec::new(),
            action_budget: 4,
            action_count: 1,
            boundary: BOUNDARY.into(),
        }
    }

    fn qualify_request(
        request: &GrantIntegrityRequest4,
    ) -> Result<GrantIntegrityCard7, GrantIntegrityError> {
        qualify(
            request,
            "AFA-policy-P32-F01",
            CONTRACT_VERSION,
            "study",
            "local",
        )
    }

    #[test]
    fn manifest_publishes_the_versioned_content_binding() {
        let manifest = manifest("feature", CONTRACT_VERSION, "study", "local");

        assert_eq!(manifest["output_schema"], "GrantIntegrityCard7@2");
        assert_eq!(
            manifest["content_binding"],
            "canonical card body excluding closure_digest and artifact.content_hash"
        );
    }

    #[test]
    fn qualified_card_binds_all_grant_authority_links() {
        let card = qualify_request(&request(vec![grant("grant-a", "compute", "A2")])).unwrap();

        assert_eq!(card.schema_version, OUTPUT_SCHEMA_VERSION);
        assert_eq!(card.disposition, "qualified");
        assert_eq!(card.actor_by_grant["grant-a"], "agent:local");
        assert_eq!(card.action_by_grant["grant-a"], "compute");
        assert_eq!(card.scope_by_grant["grant-a"], "study:local");
        assert_eq!(card.expiration_epoch_by_grant["grant-a"], 20);
        assert_eq!(
            card.artifact.evidence_digests["grant-a"],
            hash("evidence:grant-a")
        );
        card.validate().unwrap();

        let mut forged_actor = card.clone();
        forged_actor
            .actor_by_grant
            .insert("grant-a".into(), "agent:forged".into());
        assert!(forged_actor.validate().is_err());

        let mut forged_evidence = card;
        forged_evidence
            .artifact
            .evidence_digests
            .insert("grant-a".into(), hash("evidence:forged"));
        assert!(forged_evidence.validate().is_err());
    }

    #[test]
    fn autonomy_tiers_use_semantic_order_and_accept_wire_case_variants() {
        let mut request = request(vec![grant("grant-a", "compute", "a2")]);
        let permitted = qualify_request(&request).unwrap();
        assert_eq!(permitted.allowed_order, ["grant-a"]);

        request.maximum_tier = "A1".into();
        let denied = qualify_request(&request).unwrap();
        assert_eq!(denied.denied_order, ["grant-a"]);
        assert_eq!(denied.disposition, "partial");
    }

    #[test]
    fn grants_expire_at_the_boundary_epoch() {
        let mut request = request(vec![grant("grant-a", "compute", "A2")]);
        request.current_epoch = 20;

        let card = qualify_request(&request).unwrap();
        assert_eq!(card.denied_order, ["grant-a"]);
        assert!(card.allowed_order.is_empty());
    }

    #[test]
    fn revoked_grants_are_denied_and_named_in_the_receipt() {
        let mut grant = grant("grant-a", "compute", "A2");
        grant.revoked = true;

        let card = qualify_request(&request(vec![grant])).unwrap();
        assert_eq!(card.denied_order, ["grant-a"]);
        assert_eq!(card.revocation_order, ["grant-a:revoked"]);
        card.validate().unwrap();
    }

    #[test]
    fn missing_required_grants_remain_unknown() {
        let mut request = request(vec![grant("grant-a", "compute", "A2")]);
        request.required_grant_order = vec!["grant-a".into(), "grant-b".into()];

        let card = qualify_request(&request).unwrap();
        assert_eq!(card.disposition, "unknown");
        assert_eq!(card.allowed_order, ["grant-a"]);
        card.validate().unwrap();
    }

    #[test]
    fn policy_denial_blocks_and_accounts_for_every_grant() {
        let mut request = request(vec![grant("grant-a", "compute", "A2")]);
        request.policy_allowed = false;

        let card = qualify_request(&request).unwrap();
        assert_eq!(card.disposition, "blocked");
        assert_eq!(card.omitted_order, card.grant_order);
        assert!(card.allowed_order.is_empty());
        card.validate().unwrap();
    }

    #[test]
    fn unsupported_autonomy_tiers_are_refused() {
        let request = request(vec![grant("grant-a", "compute", "A5")]);

        assert!(matches!(
            qualify_request(&request),
            Err(GrantIntegrityError::Invalid(_))
        ));
    }
}
