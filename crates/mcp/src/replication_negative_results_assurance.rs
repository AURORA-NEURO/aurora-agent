//! Prospective high-throughput replication and negative-result assurance for `AFA-mcp-P15-F27`.
//!
//! The harness turns caller-supplied, aggregate-only replication observations into a deterministic
//! `ReplicationRecord7`. It is a release gate, not a statistics engine: null, negative,
//! contradictory, missing, and unmeasured results remain explicit and prevent an unsafe claim.

use bioprism_foundation::{TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::{ContentHash, QualityEvidenceState};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-mcp-P15-F27";
pub const CONTRACT_VERSION: &str = "mcp-prospective-high-throughput-replication-negative-results-assurance/1.0";
pub const INPUT_SCHEMA: &str = "ClaimAndProtocol3@1";
pub const OUTPUT_SCHEMA: &str = "ReplicationRecord7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.mcp-replication-record-7+json";
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationOutcome { Positive, Null, Negative, Inconclusive }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationObservation3 {
    pub observation_id: String,
    pub study_id: String,
    pub site_id: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub outcome: ReplicationOutcome,
    pub effect_milli: i64,
    pub evidence_state: QualityEvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed: bool,
    pub comparable: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimAndProtocol3 {
    pub request_id: String,
    pub claim_id: String,
    pub claim_text: String,
    pub protocol_id: String,
    pub semantic_profile: String,
    pub expected_direction: String,
    pub minimum_replicates: usize,
    pub batch_limit: usize,
    pub protocol_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub observations: Vec<ReplicationObservation3>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationRecord7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub claim_id: String,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub positive_order: Vec<String>,
    pub null_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub inconclusive_order: Vec<String>,
    pub site_order: Vec<String>,
    pub missing_site_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub effect_median_milli: i64,
    pub positive_count: usize,
    pub null_count: usize,
    pub negative_count: usize,
    pub batch_limit: usize,
    pub replay_identity: ContentHash,
    pub record_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ReplicationAssuranceError {
    #[error("invalid replication assurance request: {0}")] Invalid(String),
    #[error("replication assurance artifact failed: {0}")] Artifact(String),
}

fn canonical(values: &[String]) -> bool { values.windows(2).all(|w| w[0] < w[1]) }
fn digest(value: &ContentHash) -> bool { value.as_str().len() == 64 }

impl ReplicationRecord7 {
    pub fn validate(&self) -> Result<(), ReplicationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local || !self.aggregate_only
            || self.request_id.trim().is_empty() || self.claim_id.trim().is_empty()
            || self.observation_order.is_empty() || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        { return Err(ReplicationAssuranceError::Invalid("identity, locality, observations, or effects are incomplete".into())); }
        for values in [&self.observation_order, &self.qualified_order, &self.unresolved_order, &self.blocked_order, &self.positive_order, &self.null_order, &self.negative_order, &self.inconclusive_order, &self.site_order, &self.missing_site_order, &self.omission_order, &self.negative_evidence_order, &self.effect_receipts] {
            if !canonical(values) { return Err(ReplicationAssuranceError::Invalid("replication ordering is not canonical".into())); }
        }
        let all = BTreeSet::from_iter(self.observation_order.iter().cloned());
        let parts = self.qualified_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<BTreeSet<_>>();
        if all != parts || all.len() != self.observation_order.len() { return Err(ReplicationAssuranceError::Invalid("observation states do not partition".into())); }
        if self.artifact.content_type != CONTENT_TYPE || self.artifact.content_hash != self.record_digest { return Err(ReplicationAssuranceError::Artifact("artifact metadata or digest is inconsistent".into())); }
        if self.effect_receipts.iter().any(|e| e != "block:unsafe-release" && !e.starts_with("release:replication:")) { return Err(ReplicationAssuranceError::Invalid("effect outside assurance boundary".into())); }
        Ok(())
    }
}

pub fn replication_assurance_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"mcp","consumers":["AURORA extension developer","replication scientist","release governance board"],"behavior":"audits high-throughput replication and negative-result summaries without executing protocols or exporting raw data","value":"keeps null, negative, contradictory, and incomplete replication evidence visible before release","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["block:unsafe-release","release:replication:qualified"],"permissions":["evaluate:capability-runs"],"autonomy_tier":"A1","boundary":PRECLINICAL_BOUNDARY})
}

pub fn assure_replication(request: ClaimAndProtocol3) -> Result<ReplicationRecord7, ReplicationAssuranceError> {
    if request.request_id.trim().is_empty() || request.claim_id.trim().is_empty() || request.claim_text.trim().is_empty() || request.protocol_id.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.expected_direction.trim().is_empty() || request.minimum_replicates == 0 || request.batch_limit == 0 || request.observations.is_empty() || request.observations.len() > request.batch_limit || request.observations.len() > MAX_OBSERVATIONS || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || !request.aggregate_only || !digest(&request.protocol_digest) || !digest(&request.baseline_digest) || !digest(&request.replay_identity) {
        return Err(ReplicationAssuranceError::Invalid("claim, batch, digest, locality, or boundary constraints are invalid".into()));
    }
    let mut rows = request.observations.clone();
    rows.sort_by(|a,b| a.study_id.cmp(&b.study_id).then(a.site_id.cmp(&b.site_id)).then(a.observation_id.cmp(&b.observation_id)));
    let mut ids = BTreeSet::new();
    if rows.iter().any(|r| r.observation_id.trim().is_empty() || !ids.insert(r.observation_id.clone()) || r.study_id.trim().is_empty() || r.site_id.trim().is_empty() || !digest(&r.artifact_digest) || !digest(&r.provenance_digest) || !digest(&r.replay_identity)) { return Err(ReplicationAssuranceError::Invalid("observation identity or digest is invalid".into())); }
    let observation_order = rows.iter().map(|r| r.observation_id.clone()).collect::<Vec<_>>();
    let site_order = rows.iter().map(|r| r.site_id.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>();
    let mut qualified = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new();
    let mut positive = BTreeSet::new(); let mut nulls = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut inconclusive = BTreeSet::new(); let mut omissions = BTreeSet::new(); let mut negative_evidence = BTreeSet::new(); let mut effects = Vec::new(); let mut effect_values = Vec::new();
    for row in &rows {
        for reason in &row.omission_reasons { omissions.insert(format!("{}:{}", row.observation_id, reason)); }
        match row.outcome { ReplicationOutcome::Positive => { positive.insert(row.observation_id.clone()); effect_values.push(row.effect_milli); }, ReplicationOutcome::Null => { nulls.insert(row.observation_id.clone()); negative_evidence.insert(format!("{}:null", row.observation_id)); }, ReplicationOutcome::Negative => { negative.insert(row.observation_id.clone()); negative_evidence.insert(format!("{}:negative", row.observation_id)); }, ReplicationOutcome::Inconclusive => { inconclusive.insert(row.observation_id.clone()); } }
        let compatible = row.protocol_id == request.protocol_id && row.semantic_profile == request.semantic_profile && row.replay_identity == request.replay_identity && row.signed && row.comparable && row.raw_data_local && row.aggregate_only;
        if row.evidence_state == QualityEvidenceState::Contradicted { blocked.insert(row.observation_id.clone()); negative_evidence.insert(format!("{}:contradicted", row.observation_id)); }
        else if !compatible || !matches!(row.evidence_state, QualityEvidenceState::Proven | QualityEvidenceState::Supported) { unresolved.insert(row.observation_id.clone()); }
        else { qualified.insert(row.observation_id.clone()); }
    }
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !request.aggregate_only;
    if !request.policy_allow { omissions.insert("request:policy-denied".into()); }
    if !request.protected_closure { omissions.insert("request:protected-closure-incomplete".into()); }
    if !request.signed_approval { omissions.insert("request:signed-approval-missing".into()); }
    let disposition = if global_block || !blocked.is_empty() { "blocked" } else if qualified.len() < request.minimum_replicates || !negative.is_empty() || !nulls.is_empty() || !inconclusive.is_empty() { "unresolved" } else { "qualified" };
    if disposition != "qualified" { omissions.insert("request:replication-gates-incomplete".into()); }
    if global_block { blocked.extend(observation_order.iter().cloned()); qualified.clear(); unresolved.clear(); }
    let effect_median_milli = if effect_values.is_empty() { 0 } else { effect_values.sort(); effect_values[effect_values.len()/2] };
    let qualified_order = qualified.into_iter().collect::<Vec<_>>(); let unresolved_order = unresolved.into_iter().collect::<Vec<_>>(); let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"claim_id":request.claim_id,"disposition":disposition,"observation_order":observation_order,"qualified_order":qualified_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"positive_order":positive,"null_order":nulls,"negative_order":negative,"inconclusive_order":inconclusive,"site_order":site_order,"omission_order":omissions,"negative_evidence_order":negative_evidence,"effect_median_milli":effect_median_milli,"batch_limit":request.batch_limit,"replay_identity":request.replay_identity,"boundary":PRECLINICAL_BOUNDARY});
    let record_digest = ContentHash::of_value(&payload).map_err(|e| ReplicationAssuranceError::Artifact(e.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(format!("replication-record-7:{}", request.request_id), CONTENT_TYPE, &payload, Vec::new(), Vec::new()).map_err(|e| ReplicationAssuranceError::Artifact(e.to_string()))?;
    effects.push(if disposition == "qualified" { format!("release:replication:qualified:{}", request.request_id) } else { "block:unsafe-release".into() });
    let mut receipt = ReplicationRecord7 { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id, claim_id: request.claim_id, disposition: disposition.into(), observation_order: payload["observation_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), qualified_order: payload["qualified_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), unresolved_order: payload["unresolved_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), blocked_order: payload["blocked_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), positive_order: positive.into_iter().collect(), null_order: nulls.into_iter().collect(), negative_order: negative.into_iter().collect(), inconclusive_order: inconclusive.into_iter().collect(), site_order: site_order.clone(), missing_site_order: site_order.into_iter().filter(|s| !rows.iter().any(|r| r.site_id == *s && receipt_placeholder(&qualified_order, &r.observation_id))).collect(), omission_order: omissions.into_iter().collect(), negative_evidence_order: negative_evidence.into_iter().collect(), effect_median_milli, positive_count: 0, null_count: 0, negative_count: 0, batch_limit: request.batch_limit, replay_identity: request.replay_identity, record_digest, artifact, effect_receipts: effects, raw_data_local: request.raw_data_local, aggregate_only: request.aggregate_only, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.positive_count = receipt.positive_order.len(); receipt.null_count = receipt.null_order.len(); receipt.negative_count = receipt.negative_order.len(); receipt.validate()?; Ok(receipt)
}

fn receipt_placeholder(qualified: &[String], id: &str) -> bool { qualified.iter().any(|q| q == id) }

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(v: &str) -> ContentHash { ContentHash::of_bytes(v.as_bytes()) }
    fn request() -> ClaimAndProtocol3 { ClaimAndProtocol3 { request_id:"request:replication".into(), claim_id:"claim:1".into(), claim_text:"effect is reproducible".into(), protocol_id:"protocol:1".into(), semantic_profile:"neuro:v1".into(), expected_direction:"positive".into(), minimum_replicates:1, batch_limit:4, protocol_digest:hash("protocol"), baseline_digest:hash("baseline"), replay_identity:hash("replay"), policy_allow:true, protected_closure:true, signed_approval:true, raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into(), observations:vec![obs("a",ReplicationOutcome::Positive,QualityEvidenceState::Supported)] } }
    fn obs(id:&str, outcome:ReplicationOutcome, state:QualityEvidenceState)->ReplicationObservation3 { ReplicationObservation3 { observation_id:id.into(), study_id:format!("study:{id}"), site_id:format!("site:{id}"), protocol_id:"protocol:1".into(), semantic_profile:"neuro:v1".into(), outcome, effect_milli:100, evidence_state:state, artifact_digest:hash(id), provenance_digest:hash(&format!("p:{id}")), replay_identity:hash("replay"), signed:true, comparable:true, raw_data_local:true, aggregate_only:true, negative_result:false, omission_reasons:vec![] } }
    #[test] fn manifest_is_versioned(){ assert_eq!(replication_assurance_manifest()["capability_id"], FEATURE_ID); }
    #[test] fn qualified_batch_is_released(){ let r=assure_replication(request()).unwrap(); assert_eq!(r.disposition,"qualified"); assert!(r.effect_receipts[0].starts_with("release:replication:")); }
    #[test] fn null_is_unresolved(){ let mut q=request(); q.observations[0].outcome=ReplicationOutcome::Null; let r=assure_replication(q).unwrap(); assert_eq!(r.disposition,"unresolved"); assert!(r.negative_evidence_order.iter().any(|v| v.ends_with(":null"))); }
    #[test] fn policy_denial_blocks(){ let mut q=request(); q.policy_allow=false; let r=assure_replication(q).unwrap(); assert_eq!(r.disposition,"blocked"); assert_eq!(r.effect_receipts,vec!["block:unsafe-release"]); }
    #[test] fn contradiction_blocks(){ let mut q=request(); q.observations[0].evidence_state=QualityEvidenceState::Contradicted; let r=assure_replication(q).unwrap(); assert_eq!(r.disposition,"blocked"); }
    #[test] fn deterministic_digest(){ let a=assure_replication(request()).unwrap(); let mut q=request(); q.observations.reverse(); let b=assure_replication(q).unwrap(); assert_eq!(a.record_digest,b.record_digest); }
}
