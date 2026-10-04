//! Fiber P32 frontier: certify evidence fibration and protected closure before compilation.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;
pub const FEATURE_ID: &str = "AFA-fiber-P32-F01";
pub const CONTRACT_VERSION: &str = "fiber-local-fibration-integrity/2.0";
pub const OUTPUT_SCHEMA_VERSION: &str = "2.0.0";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.fiber.fibration-integrity-card-2+json";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FiberRegion4 {
    pub region_id: String,
    pub section_id: String,
    pub factor_order: Vec<String>,
    pub evidence_digest: ContentHash,
    pub scope_id: String,
    pub policy_epoch: u64,
    pub local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FibrationIntegrityRequest4 {
    pub request_id: String,
    pub purpose: String,
    pub regions: Vec<FiberRegion4>,
    pub required_region_order: Vec<String>,
    pub required_factor_order: Vec<String>,
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
pub struct FibrationIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    /// Evidence identity keyed by region so provenance cannot become detached from its source.
    pub region_digests: BTreeMap<String, ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FibrationIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub mode: String,
    pub scale: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub region_order: Vec<String>,
    /// The section identity for every region in `region_order`.
    pub section_by_region: BTreeMap<String, String>,
    pub accepted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub factor_order: Vec<String>,
    pub scope_order: Vec<String>,
    pub epoch_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub closure_digest: ContentHash,
    pub artifact: FibrationIntegrityArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FibrationIntegrityError {
    #[error("invalid fibration-integrity request: {0}")]
    Invalid(String),
    #[error("fibration-integrity card failed validation: {0}")]
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
pub fn manifest(id: &str, version: &str, scale: &str, mode: &str) -> serde_json::Value {
    json!({"schema_version":"1.0.0","capability_id":id,"version":version,"owner_crate":"fiber","consumers":["query compiler","protected-closure verifier","decision-section compiler","release auditor"],"behavior":format!("certify evidence fibration and protected closure at {scale} ({mode})"),"value":"prevents scope leakage and unsupported factor joins while making deferred evidence explicit","input_schema":"FibrationIntegrityRequest4@1","output_schema":"FibrationIntegrityCard7@2","content_binding":"canonical card body excluding closure_digest and artifact.content_hash","effects":["emit:fibration-card","retain:semantic-loss","block:unsafe-compilation"],"permissions":["read:local-fiber-regions"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}

fn content_digest(value: &Value) -> Result<ContentHash, FibrationIntegrityError> {
    let mut body = value.clone();
    let object = body.as_object_mut().ok_or_else(|| {
        FibrationIntegrityError::Output("fibration card is not a JSON object".into())
    })?;
    object.remove("closure_digest");
    if let Some(artifact) = object.get_mut("artifact").and_then(Value::as_object_mut) {
        artifact.remove("content_hash");
    }
    ContentHash::of_value(&body).map_err(|error| FibrationIntegrityError::Output(error.to_string()))
}

impl FibrationIntegrityCard7 {
    pub fn validate(&self) -> Result<(), FibrationIntegrityError> {
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
            || self.region_order.is_empty()
            || !ordered_nonempty(&self.region_order)
            || !ordered_nonempty(&self.accepted_order)
            || !ordered_nonempty(&self.rejected_order)
            || !ordered_nonempty(&self.unknown_order)
            || !ordered_nonempty(&self.omitted_order)
            || !ordered_nonempty(&self.factor_order)
            || !ordered_nonempty(&self.scope_order)
            || !ordered_nonempty(&self.epoch_order)
            || !ordered_nonempty(&self.negative_evidence_order)
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
            return Err(FibrationIntegrityError::Output(
                "fibration identity, ordering, locality, digest, or artifact is invalid".into(),
            ));
        }
        let ids = BTreeSet::from_iter(self.region_order.iter().cloned());
        if self.section_by_region.len() != ids.len()
            || self
                .section_by_region
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                != ids
            || self
                .section_by_region
                .values()
                .any(|section| !nonempty(section))
            || self
                .artifact
                .region_digests
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                != ids
            || self
                .artifact
                .region_digests
                .values()
                .any(|value| !digest(value))
            || self.artifact.artifact_id != format!("fiber-fibration:{}", self.request_id)
        {
            return Err(FibrationIntegrityError::Output(
                "region-to-section mapping or artifact identity is invalid".into(),
            ));
        }
        let states = self
            .accepted_order
            .iter()
            .chain(&self.rejected_order)
            .chain(&self.unknown_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.region_order.len()
            || states.len() != ids.len()
            || BTreeSet::from_iter(states) != ids
        {
            return Err(FibrationIntegrityError::Output(
                "region states do not partition".into(),
            ));
        }
        let expected_effect = if self.disposition == "qualified" {
            format!("emit:fibration-card:{}", self.request_id)
        } else {
            "block:unsafe-compilation".to_string()
        };
        if self.effect_receipts != [expected_effect]
            || !["qualified", "partial", "unknown", "blocked"].contains(&self.disposition.as_str())
            || (self.disposition == "blocked"
                && (self.omitted_order != self.region_order
                    || !self.accepted_order.is_empty()
                    || !self.rejected_order.is_empty()
                    || !self.unknown_order.is_empty()))
            || (self.disposition == "qualified"
                && (self.accepted_order != self.region_order
                    || !self.rejected_order.is_empty()
                    || !self.unknown_order.is_empty()
                    || !self.omitted_order.is_empty()))
            || (self.disposition == "partial" && self.accepted_order == self.region_order)
        {
            return Err(FibrationIntegrityError::Output(
                "disposition, region states, or effect receipt is inconsistent".into(),
            ));
        }
        let value = serde_json::to_value(self)
            .map_err(|error| FibrationIntegrityError::Output(error.to_string()))?;
        if content_digest(&value)? != self.closure_digest {
            return Err(FibrationIntegrityError::Output(
                "closure digest does not bind the card body".into(),
            ));
        }
        Ok(())
    }
}
pub fn certify(
    request: &FibrationIntegrityRequest4,
    id: &str,
    version: &str,
    scale: &str,
    mode: &str,
) -> Result<FibrationIntegrityCard7, FibrationIntegrityError> {
    if !nonempty(&request.request_id)
        || !nonempty(&request.purpose)
        || request.regions.is_empty()
        || request.required_region_order.is_empty()
        || request.required_factor_order.is_empty()
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !nonempty(id)
        || !nonempty(version)
        || !nonempty(scale)
        || !nonempty(mode)
        || !ordered_nonempty(&request.required_region_order)
        || !ordered_nonempty(&request.required_factor_order)
        || !ordered_nonempty(&request.adversarial_events)
    {
        return Err(FibrationIntegrityError::Invalid(
            "fibration identity, requirements, digest, ordering, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut rows = request.regions.clone();
    rows.sort_by(|a, b| a.region_id.cmp(&b.region_id));
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    let mut section_by_region = BTreeMap::new();
    let mut accepted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut factors = BTreeSet::new();
    let mut scopes = BTreeSet::new();
    let mut epochs = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut digests = BTreeMap::new();
    for r in &rows {
        if !seen.insert(r.region_id.clone())
            || !nonempty(&r.region_id)
            || !nonempty(&r.section_id)
            || r.factor_order.is_empty()
            || !ordered_nonempty(&r.factor_order)
            || !digest(&r.evidence_digest)
            || !nonempty(&r.scope_id)
            || !r.local
            || !r.aggregate_only
        {
            return Err(FibrationIntegrityError::Invalid(
                "region identity, factor ordering, evidence, or locality is invalid".into(),
            ));
        }
        order.push(r.region_id.clone());
        section_by_region.insert(r.region_id.clone(), r.section_id.clone());
        factors.extend(r.factor_order.iter().cloned());
        scopes.insert(r.scope_id.clone());
        epochs.insert(format!("{}:{}", r.scope_id, r.policy_epoch));
        digests.insert(r.region_id.clone(), r.evidence_digest.clone());
        if r.negative_result {
            negative.insert(format!("{}:negative-result", r.region_id));
        }
        if r.policy_epoch == 0 {
            unknown.insert(r.region_id.clone());
        } else if !request.required_region_order.contains(&r.region_id)
            || !request
                .required_factor_order
                .iter()
                .all(|f| r.factor_order.contains(f))
        {
            rejected.insert(r.region_id.clone());
        } else if r.evidence_digest == request.replay_identity {
            omitted.insert(r.region_id.clone());
        } else {
            accepted.insert(r.region_id.clone());
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
        accepted.clear();
        rejected.clear();
        unknown.clear();
    }
    let missing = !request
        .required_region_order
        .iter()
        .all(|r| seen.contains(r))
        || !request
            .required_factor_order
            .iter()
            .all(|f| factors.contains(f));
    let disposition = if global {
        "blocked"
    } else if missing {
        "unknown"
    } else if !rejected.is_empty() || !unknown.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let mut payload = json!({"schema_version":OUTPUT_SCHEMA_VERSION,"contract_version":version,"feature_id":id,"mode":mode,"scale":scale,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"region_order":order,"section_by_region":section_by_region,"accepted_order":accepted.into_iter().collect::<Vec<_>>(),"rejected_order":rejected.into_iter().collect::<Vec<_>>(),"unknown_order":unknown.into_iter().collect::<Vec<_>>(),"omitted_order":omitted.into_iter().collect::<Vec<_>>(),"factor_order":factors.into_iter().collect::<Vec<_>>(),"scope_order":scopes.into_iter().collect::<Vec<_>>(),"epoch_order":epochs.into_iter().collect::<Vec<_>>(),"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":BOUNDARY});
    payload["artifact"] = json!({"artifact_id":format!("fiber-fibration:{}",request.request_id),"content_type":CONTENT_TYPE,"semantic_loss":payload["omitted_order"],"region_digests":digests,"boundary":BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("emit:fibration-card:{}", request.request_id)]
    } else {
        vec!["block:unsafe-compilation".to_string()]
    });
    let hash = content_digest(&payload)?;
    payload["closure_digest"] = json!(hash);
    payload["artifact"]["content_hash"] = json!(hash);
    let out: FibrationIntegrityCard7 = serde_json::from_value(payload)
        .map_err(|e| FibrationIntegrityError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn region(region_id: &str, section_id: &str) -> FiberRegion4 {
        FiberRegion4 {
            region_id: region_id.into(),
            section_id: section_id.into(),
            factor_order: vec!["factor-a".into()],
            evidence_digest: hash(&format!("evidence:{region_id}")),
            scope_id: "study:local".into(),
            policy_epoch: 1,
            local: true,
            aggregate_only: true,
            negative_result: false,
        }
    }

    fn request(regions: Vec<FiberRegion4>) -> FibrationIntegrityRequest4 {
        FibrationIntegrityRequest4 {
            request_id: "request:1".into(),
            purpose: "verify protected region mapping".into(),
            regions,
            required_region_order: vec!["region-a".into()],
            required_factor_order: vec!["factor-a".into()],
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

    fn certify_request(
        request: &FibrationIntegrityRequest4,
    ) -> Result<FibrationIntegrityCard7, FibrationIntegrityError> {
        certify(request, FEATURE_ID, CONTRACT_VERSION, "study", "local")
    }

    #[test]
    fn manifest_publishes_the_versioned_content_binding() {
        let manifest = manifest(FEATURE_ID, CONTRACT_VERSION, "study", "local");

        assert_eq!(manifest["output_schema"], "FibrationIntegrityCard7@2");
        assert_eq!(
            manifest["content_binding"],
            "canonical card body excluding closure_digest and artifact.content_hash"
        );
    }

    #[test]
    fn qualified_card_binds_section_mapping_and_artifact_provenance() {
        let card = certify_request(&request(vec![region("region-a", "section-a")])).unwrap();

        assert_eq!(card.schema_version, OUTPUT_SCHEMA_VERSION);
        assert_eq!(card.disposition, "qualified");
        assert_eq!(
            card.section_by_region.get("region-a"),
            Some(&"section-a".to_string())
        );
        card.validate().unwrap();

        let mut changed_section = card.clone();
        changed_section
            .section_by_region
            .insert("region-a".into(), "section-forged".into());
        assert!(changed_section.validate().is_err());

        let mut changed_provenance = card;
        changed_provenance
            .artifact
            .region_digests
            .insert("region-a".into(), hash("forged-evidence"));
        assert!(changed_provenance.validate().is_err());
    }

    #[test]
    fn effect_receipts_are_bound_to_the_disposition() {
        let mut card = certify_request(&request(vec![region("region-a", "section-a")])).unwrap();
        card.effect_receipts[0] = "emit:fibration-card:forged-request".into();
        assert!(card.validate().is_err());
    }

    #[test]
    fn policy_denial_blocks_and_accounts_for_every_region() {
        let mut request = request(vec![region("region-a", "section-a")]);
        request.policy_allowed = false;

        let card = certify_request(&request).unwrap();
        assert_eq!(card.disposition, "blocked");
        assert!(card.accepted_order.is_empty());
        assert_eq!(card.omitted_order, card.region_order);
        assert_eq!(card.effect_receipts, ["block:unsafe-compilation"]);
        card.validate().unwrap();
    }

    #[test]
    fn missing_required_region_is_unknown_and_extra_region_is_rejected() {
        let card = certify_request(&request(vec![region("region-b", "section-b")])).unwrap();

        assert_eq!(card.disposition, "unknown");
        assert_eq!(card.rejected_order, ["region-b"]);
        assert_eq!(card.accepted_order, Vec::<String>::new());
        card.validate().unwrap();
    }

    #[test]
    fn input_region_order_does_not_change_the_canonical_card() {
        let first = certify_request(&request(vec![
            region("region-a", "section-a"),
            region("region-b", "section-b"),
        ]))
        .unwrap();
        let second = certify_request(&request(vec![
            region("region-b", "section-b"),
            region("region-a", "section-a"),
        ]))
        .unwrap();

        assert_eq!(first.region_order, second.region_order);
        assert_eq!(first.section_by_region, second.section_by_region);
        assert_eq!(first.closure_digest, second.closure_digest);
    }

    #[test]
    fn empty_required_identifiers_are_refused() {
        let mut request = request(vec![region("region-a", "section-a")]);
        request.required_factor_order = vec![String::new()];

        assert!(matches!(
            certify_request(&request),
            Err(FibrationIntegrityError::Invalid(_))
        ));
    }
}
