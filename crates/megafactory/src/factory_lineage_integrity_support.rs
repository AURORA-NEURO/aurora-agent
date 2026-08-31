//! Megafactory P32: factory-lineage admission and replay integrity.
//!
//! A factory plan is useful only when every stage has a named downstream consumer,
//! typed ports, an explicit parent lineage, and enough evidence to replay the
//! admission decision.  This module validates that graph and emits a deterministic
//! card; it never schedules work, deploys a worker, or performs an experimental
//! effect.  Raw preclinical data remains institution-local and only aggregate
//! lineage metadata can cross a federation boundary.

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = RESEARCH_CONTRACT_SCHEMA_VERSION;
pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.megafactory.factory-lineage-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryStage4 {
    pub stage_id: String,
    pub parent_stage: String,
    pub owner_crate: String,
    pub consumer: String,
    pub behavior: String,
    pub input_schema: String,
    pub output_schema: String,
    pub effect: String,
    pub artifact_digest: String,
    pub evidence_state: String,
    pub deterministic: bool,
    pub idempotent: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryLineageRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub stages: Vec<FactoryStage4>,
    pub required_stage_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub stage_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryLineageArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FactoryLineageCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub lineage_order: Vec<String>,
    pub consumer_order: Vec<String>,
    pub contract_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub admitted_stage_count: u64,
    pub total_stage_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: FactoryLineageArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FactoryLineageIntegrityError {
    #[error("factory lineage request is invalid: {0}")]
    Invalid(String),
    #[error("factory lineage digest failed: {0}")]
    Digest(String),
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn invalid(message: impl Into<String>) -> FactoryLineageIntegrityError {
    FactoryLineageIntegrityError::Invalid(message.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "megafactory",
        "consumers": ["factory planner", "workflow compiler", "federation steward", "replay auditor"],
        "behavior": format!("qualify deterministic factory lineage at {scale} ({mode})"),
        "value": "prevents orphaned, cyclic, or unauditable factory stages from entering research execution",
        "input_schema": "FactoryLineageRequest4@1",
        "output_schema": "FactoryLineageCard7@1",
        "effects": ["emit:lineage-admission-card", "retain:rejected-and-unresolved-stages", "block:unsafe-factory-plan"],
        "permissions": ["read:local-factory-manifests", "exchange:aggregate-lineage"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": BOUNDARY,
    })
}

fn has_cycle(stage_map: &BTreeMap<String, String>) -> bool {
    fn visit(
        node: &str,
        stage_map: &BTreeMap<String, String>,
        visiting: &mut BTreeSet<String>,
        finished: &mut BTreeSet<String>,
    ) -> bool {
        if finished.contains(node) {
            return false;
        }
        if !visiting.insert(node.to_owned()) {
            return true;
        }
        if let Some(parent) = stage_map.get(node) {
            if parent != "root" && visit(parent, stage_map, visiting, finished) {
                return true;
            }
        }
        visiting.remove(node);
        finished.insert(node.to_owned());
        false
    }

    let mut visiting = BTreeSet::new();
    let mut finished = BTreeSet::new();
    stage_map
        .keys()
        .any(|node| visit(node, stage_map, &mut visiting, &mut finished))
}

fn validate_card(card: &FactoryLineageCard7) -> Result<(), FactoryLineageIntegrityError> {
    if card.schema_version != SCHEMA_VERSION
        || card.feature_id.is_empty()
        || card.request_id.is_empty()
        || card.purpose.is_empty()
        || card.boundary != BOUNDARY
        || card.artifact.boundary != BOUNDARY
        || !card.raw_data_local
        || !card.aggregate_only
        || !valid_digest(&card.replay_identity)
        || !valid_digest(&card.closure_digest)
        || card.artifact.content_type != CONTENT_TYPE
        || card.artifact.content_hash != card.closure_digest
        || card.admitted_stage_count > card.total_stage_count
    {
        return Err(invalid(
            "factory identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for values in [
        &card.stage_order,
        &card.admitted_order,
        &card.rejected_order,
        &card.unknown_order,
        &card.omitted_order,
        &card.lineage_order,
        &card.consumer_order,
        &card.contract_order,
        &card.effect_order,
        &card.effect_receipts,
    ] {
        if !canonical(values) {
            return Err(invalid("factory vectors are not canonical"));
        }
    }
    let ids = card.stage_order.iter().collect::<BTreeSet<_>>();
    let states = card
        .admitted_order
        .iter()
        .chain(&card.rejected_order)
        .chain(&card.unknown_order)
        .chain(&card.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("factory stage states do not partition stages"));
    }
    if card.admitted_stage_count != card.admitted_order.len() as u64 {
        return Err(invalid(
            "admitted stage count does not match admitted order",
        ));
    }
    Ok(())
}

pub fn qualify(
    request: &FactoryLineageRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<FactoryLineageCard7, FactoryLineageIntegrityError> {
    if request.schema_version != SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.stages.is_empty()
        || request.stage_budget == 0
        || !valid_digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !canonical(&request.required_stage_order)
        || !canonical(&request.adversarial_events)
    {
        return Err(invalid(
            "factory identity, ordering, replay, locality, boundary, or budget is invalid",
        ));
    }

    let mut stages = request.stages.clone();
    stages.sort_by(|left, right| left.stage_id.cmp(&right.stage_id));
    let mut seen = BTreeSet::new();
    let mut stage_map = BTreeMap::new();
    let mut admitted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut lineage = BTreeSet::new();
    let mut consumers = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut semantic_loss = Vec::new();

    for stage in &stages {
        if stage.stage_id.trim().is_empty()
            || stage.parent_stage.trim().is_empty()
            || stage.owner_crate.trim().is_empty()
            || stage.consumer.trim().is_empty()
            || stage.behavior.trim().is_empty()
            || stage.input_schema.trim().is_empty()
            || stage.output_schema.trim().is_empty()
            || stage.effect.trim().is_empty()
            || !valid_digest(&stage.artifact_digest)
            || stage.evidence_state.trim().is_empty()
            || !stage.local
            || !stage.aggregate_only
        {
            return Err(invalid(
                "stage identity, lineage, consumer, typed ports, effect, evidence, or locality is incomplete",
            ));
        }
        if !seen.insert(stage.stage_id.clone()) {
            return Err(invalid(format!(
                "duplicate factory stage {}",
                stage.stage_id
            )));
        }
        stage_map.insert(stage.stage_id.clone(), stage.parent_stage.clone());
        lineage.insert(format!("{}<-{}", stage.stage_id, stage.parent_stage));
        consumers.insert(stage.consumer.clone());
        contracts.insert(format!("{}→{}", stage.input_schema, stage.output_schema));
        effects.insert(stage.effect.clone());
        evidence.insert(stage.artifact_digest.clone());
        match stage.evidence_state.as_str() {
            "supported" | "proven" if stage.required && stage.deterministic && stage.idempotent => {
                admitted.insert(stage.stage_id.clone());
            }
            "contradicted" | "rejected" => {
                rejected.insert(stage.stage_id.clone());
                semantic_loss.push(stage.stage_id.clone());
            }
            "unknown" | "speculative" | "unmeasured" => {
                unknown.insert(stage.stage_id.clone());
                semantic_loss.push(stage.stage_id.clone());
            }
            _ => {
                omitted.insert(stage.stage_id.clone());
                semantic_loss.push(stage.stage_id.clone());
            }
        }
    }
    if stage_map
        .values()
        .any(|parent| parent != "root" && !seen.contains(parent))
        || has_cycle(&stage_map)
    {
        return Err(invalid("factory lineage has an orphan parent or cycle"));
    }
    if request.required_stage_order.iter().collect::<BTreeSet<_>>()
        != seen.iter().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "required stage order is not the canonical stage set",
        ));
    }

    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_manifest
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || stages.len() > request.stage_budget;
    if global_block {
        omitted.extend(seen.clone());
        admitted.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global_block {
        "blocked"
    } else if !unknown.is_empty() {
        "unknown"
    } else if !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let stage_order = seen.iter().cloned().collect::<Vec<_>>();
    let body = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request.request_id,
        "purpose": request.purpose,
        "disposition": disposition,
        "stage_order": stage_order,
    });
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|error| FactoryLineageIntegrityError::Digest(error.to_string()))?
        .to_string();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let artifact = FactoryLineageArtifact4 {
        artifact_id: format!("megafactory-lineage:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: if global_block {
            seen.iter().cloned().collect()
        } else {
            semantic_loss
        },
        evidence_digests: evidence.into_iter().collect(),
        boundary: BOUNDARY.into(),
    };
    let card = FactoryLineageCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        disposition: disposition.into(),
        stage_order,
        admitted_order: admitted_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        lineage_order: lineage.into_iter().collect(),
        consumer_order: consumers.into_iter().collect(),
        contract_order: contracts.into_iter().collect(),
        effect_order: effects.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        closure_digest,
        admitted_stage_count: admitted_order.len() as u64,
        total_stage_count: stages.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "qualified" {
            vec![format!("prepare:factory-lineage:{}", request.request_id)]
        } else {
            vec!["block:unsafe-factory-plan".into()]
        },
        artifact,
    };
    validate_card(&card)?;
    let _ = (scale, mode);
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> FactoryLineageRequest4 {
        let digest = "a".repeat(64);
        FactoryLineageRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "factory-1".into(),
            purpose: "qualify a replayable factory".into(),
            stages: vec![
                FactoryStage4 {
                    stage_id: "stage-a".into(),
                    parent_stage: "root".into(),
                    owner_crate: "megafactory".into(),
                    consumer: "workflow compiler".into(),
                    behavior: "compile bounded inputs".into(),
                    input_schema: "RawModalityBundle@1".into(),
                    output_schema: "HarmonizedObject@1".into(),
                    effect: "emit:local-artifact".into(),
                    artifact_digest: digest.clone(),
                    evidence_state: "supported".into(),
                    deterministic: true,
                    idempotent: true,
                    local: true,
                    aggregate_only: true,
                    required: true,
                },
                FactoryStage4 {
                    stage_id: "stage-b".into(),
                    parent_stage: "stage-a".into(),
                    owner_crate: "megafactory".into(),
                    consumer: "replay auditor".into(),
                    behavior: "emit a lineage receipt".into(),
                    input_schema: "HarmonizedObject@1".into(),
                    output_schema: "ExecutionRun@1".into(),
                    effect: "emit:lineage-receipt".into(),
                    artifact_digest: "b".repeat(64),
                    evidence_state: "proven".into(),
                    deterministic: true,
                    idempotent: true,
                    local: true,
                    aggregate_only: true,
                    required: true,
                },
            ],
            required_stage_order: vec!["stage-a".into(), "stage-b".into()],
            replay_identity: "c".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            stage_budget: 4,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn qualifies_topological_lineage_without_execution() {
        let card = qualify(
            &request(),
            "AFA-megafactory-P32-F01",
            "v1",
            "local",
            "inference",
        )
        .expect("valid lineage");
        assert_eq!(card.disposition, "qualified");
        assert_eq!(card.admitted_stage_count, 2);
        assert_eq!(
            card.effect_receipts,
            vec!["prepare:factory-lineage:factory-1"]
        );
    }

    #[test]
    fn policy_failure_blocks_and_preserves_omissions() {
        let mut request = request();
        request.policy_allowed = false;
        let card = qualify(
            &request,
            "AFA-megafactory-P32-F02",
            "v1",
            "local",
            "inference",
        )
        .expect("blocked plans still produce an auditable card");
        assert_eq!(card.disposition, "blocked");
        assert!(card.admitted_order.is_empty());
        assert_eq!(card.omitted_order, vec!["stage-a", "stage-b"]);
    }

    #[test]
    fn orphan_and_cycle_are_rejected_before_admission() {
        let mut request = request();
        request.stages[0].parent_stage = "stage-b".into();
        assert!(matches!(
            qualify(&request, "AFA-megafactory-P32-F03", "v1", "local", "inference"),
            Err(FactoryLineageIntegrityError::Invalid(message)) if message.contains("cycle")
        ));
    }
}
