//! Independent-evidence consensus for preclinical glioma mechanism research.
//!
//! Mechanism posteriors from imaging, pathway activity, clonal profiles, and computational models
//! should not be concatenated as if every record were an independent observation. This module
//! aggregates evidence by source, weights each source by declared reliability, reports source
//! conflict, and computes leave-one-source-out sensitivity. It is a planning and audit product,
//! not a causal oracle and never emits clinical conclusions.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F22";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismConsensus1@1";
pub const MAX_MECHANISMS: usize = 128;
pub const MAX_EVIDENCE: usize = 8_192;
pub const MAX_SOURCES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismEvidencePacket {
    pub source_id: String,
    pub evidence_id: String,
    pub mechanism_id: String,
    pub posterior_milli: u16,
    pub reliability_milli: u16,
    pub independence_group: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismConsensusRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<MechanismEvidencePacket>,
    pub min_sources_per_mechanism: usize,
    pub max_conflict_milli: u16,
    pub max_leave_one_out_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismConsensusRecord {
    pub mechanism_id: String,
    pub posterior_milli: u16,
    pub source_count: usize,
    pub weighted_support_milli: u64,
    pub leave_one_out_min_milli: u16,
    pub leave_one_out_max_milli: u16,
    pub negative_source_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismSourceAgreement {
    pub source_id: String,
    pub independence_group: String,
    pub mechanism_count: usize,
    pub coverage_milli: u16,
    pub conflict_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismConsensusDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismConsensus {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub records: Vec<MechanismConsensusRecord>,
    pub source_order: Vec<String>,
    pub source_agreement: Vec<MechanismSourceAgreement>,
    pub conflict_pair_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: MechanismConsensusDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismConsensusError {
    #[error("mechanism consensus request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism consensus evidence is invalid: {0}")]
    InvalidEvidence(String),
    #[error("mechanism consensus output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism consensus digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(consensus: &MechanismConsensus) -> serde_json::Value {
    serde_json::json!({
        "feature_id": consensus.feature_id,
        "output_schema": consensus.output_schema,
        "objective": consensus.objective,
        "model_system": consensus.model_system,
        "mechanism_order": consensus.mechanism_order,
        "records": consensus.records,
        "source_order": consensus.source_order,
        "source_agreement": consensus.source_agreement,
        "conflict_pair_order": consensus.conflict_pair_order,
        "negative_evidence_order": consensus.negative_evidence_order,
        "uncertainty_order": consensus.uncertainty_order,
        "disposition": consensus.disposition,
        "next_action": consensus.next_action,
    })
}

fn validate_request(request: &MechanismConsensusRequest) -> Result<(), MechanismConsensusError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.is_empty()
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.evidence.is_empty()
        || request.evidence.len() > MAX_EVIDENCE
        || request.min_sources_per_mechanism == 0
        || request.min_sources_per_mechanism > MAX_SOURCES
        || request.max_conflict_milli > 1_000
        || request.max_leave_one_out_milli > 1_000
    {
        return Err(MechanismConsensusError::InvalidRequest(
            "objective, bounded mechanisms/evidence, positive source floor, and conflict bounds are required".into(),
        ));
    }
    let mechanism_set = request.mechanisms.iter().collect::<BTreeSet<_>>();
    if mechanism_set.len() != request.mechanisms.len()
        || request.mechanisms.iter().any(|id| id.trim().is_empty())
    {
        return Err(MechanismConsensusError::InvalidRequest(
            "mechanisms require unique non-empty identifiers".into(),
        ));
    }
    let mut evidence_ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for packet in &request.evidence {
        if packet.source_id.trim().is_empty()
            || packet.evidence_id.trim().is_empty()
            || packet.independence_group.trim().is_empty()
            || !mechanism_set.contains(&packet.mechanism_id)
            || packet.posterior_milli > 1_000
            || packet.reliability_milli == 0
            || packet.reliability_milli > 1_000
            || !evidence_ids.insert(packet.evidence_id.clone())
            || !packet.artifact.local_only
            || packet.artifact.contains_human_data
            || packet.artifact.contains_direct_identifiers
            || packet.artifact.validate().is_err()
        {
            return Err(MechanismConsensusError::InvalidEvidence(
                "evidence requires unique local de-identified ids, known mechanisms, bounded posterior/reliability, and a valid artifact".into(),
            ));
        }
        sources.insert(packet.source_id.clone());
    }
    if sources.len() > MAX_SOURCES {
        return Err(MechanismConsensusError::InvalidEvidence(
            "source bound exceeded".into(),
        ));
    }
    Ok(())
}

fn validate_output(consensus: &MechanismConsensus) -> Result<(), MechanismConsensusError> {
    if consensus.feature_id != FEATURE_ID
        || consensus.output_schema != OUTPUT_SCHEMA
        || consensus.objective.trim().is_empty()
        || consensus.mechanism_order.is_empty()
        || !canonical(&consensus.mechanism_order)
        || consensus.records.len() != consensus.mechanism_order.len()
        || consensus
            .source_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || consensus.source_agreement.len() != consensus.source_order.len()
        || consensus
            .source_agreement
            .windows(2)
            .any(|pair| pair[0].source_id >= pair[1].source_id)
        || !canonical(&consensus.conflict_pair_order)
        || !canonical(&consensus.negative_evidence_order)
        || !canonical(&consensus.uncertainty_order)
        || consensus.next_action.trim().is_empty()
        || consensus.records.windows(2).any(|pair| {
            pair[0].posterior_milli < pair[1].posterior_milli
                || (pair[0].posterior_milli == pair[1].posterior_milli
                    && pair[0].mechanism_id > pair[1].mechanism_id)
        })
        || consensus.records.iter().any(|record| {
            record.source_count == 0
                || record.weighted_support_milli > (MAX_EVIDENCE as u64 * 1_000_000)
                || record.leave_one_out_min_milli > record.leave_one_out_max_milli
                || !canonical(&record.negative_source_order)
        })
        || consensus.source_agreement.iter().any(|agreement| {
            agreement.source_id.trim().is_empty()
                || agreement.independence_group.trim().is_empty()
                || agreement.coverage_milli > 1_000
                || agreement.conflict_milli > 1_000
        })
    {
        return Err(MechanismConsensusError::InvalidOutput(
            "identity, ordering, posterior, source agreement, or sensitivity invariants are invalid".into(),
        ));
    }
    let mechanism_ids = consensus
        .mechanism_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let record_ids = consensus
        .records
        .iter()
        .map(|record| record.mechanism_id.clone())
        .collect::<BTreeSet<_>>();
    let source_ids = consensus
        .source_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let agreement_ids = consensus
        .source_agreement
        .iter()
        .map(|agreement| agreement.source_id.clone())
        .collect::<BTreeSet<_>>();
    if mechanism_ids != record_ids || source_ids != agreement_ids {
        return Err(MechanismConsensusError::InvalidOutput(
            "mechanism and source partitions do not reconcile".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(consensus))
        .map_err(|error| MechanismConsensusError::Digest(error.to_string()))?;
    if expected != consensus.digest {
        return Err(MechanismConsensusError::InvalidOutput(
            "mechanism consensus digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl MechanismConsensus {
    pub fn validate(&self) -> Result<(), MechanismConsensusError> {
        validate_output(self)
    }
}

fn source_means(
    evidence: &[MechanismEvidencePacket],
    mechanisms: &[String],
) -> BTreeMap<String, BTreeMap<String, (u64, u64)>> {
    let mut grouped = BTreeMap::<String, BTreeMap<String, (u64, u64)>>::new();
    for packet in evidence {
        let entry = grouped
            .entry(packet.source_id.clone())
            .or_default()
            .entry(packet.mechanism_id.clone())
            .or_insert((0, 0));
        entry.0 = entry.0.saturating_add(
            u64::from(packet.posterior_milli) * u64::from(packet.reliability_milli),
        );
        entry.1 = entry.1.saturating_add(u64::from(packet.reliability_milli));
    }
    for values in grouped.values_mut() {
        for mechanism in mechanisms {
            values.entry(mechanism.clone()).or_insert((0, 0));
        }
    }
    grouped
}

fn consensus_for_sources(
    source_values: &BTreeMap<String, BTreeMap<String, (u64, u64)>>,
    mechanisms: &[String],
    excluded_source: Option<&str>,
) -> BTreeMap<String, u16> {
    let mut output = mechanisms
        .iter()
        .map(|mechanism| (mechanism.clone(), 0_u16))
        .collect::<BTreeMap<_, _>>();
    for mechanism in mechanisms {
        let mut numerator = 0_u64;
        let mut denominator = 0_u64;
        for (source, values) in source_values {
            if excluded_source == Some(source.as_str()) {
                continue;
            }
            let (weighted, reliability) = values.get(mechanism).copied().unwrap_or((0, 0));
            if reliability > 0 {
                numerator = numerator.saturating_add(weighted / reliability);
                denominator = denominator.saturating_add(1);
            }
        }
        let value = if denominator == 0 {
            0
        } else {
            (numerator / denominator).min(1_000) as u16
        };
        output.insert(mechanism.clone(), value);
    }
    let total = output.values().map(|value| u32::from(*value)).sum::<u32>();
    if total == 0 {
        let share = 1_000_u16 / mechanisms.len().max(1) as u16;
        for (index, mechanism) in mechanisms.iter().enumerate() {
            output.insert(
                mechanism.clone(),
                share
                    + if index == 0 {
                        1_000 - share * mechanisms.len() as u16
                    } else {
                        0
                    },
            );
        }
    } else {
        let mut assigned = 0_u16;
        for (index, mechanism) in mechanisms.iter().enumerate() {
            let current = output.get(mechanism).copied().unwrap_or(0);
            let normalized = if index + 1 == mechanisms.len() {
                1_000_u16.saturating_sub(assigned)
            } else {
                (u32::from(current) * 1_000 / total).min(1_000) as u16
            };
            assigned = assigned.saturating_add(normalized);
            output.insert(mechanism.clone(), normalized);
        }
    }
    output
}

/// Aggregate independent typed evidence streams into a conflict-aware mechanism consensus.
pub fn compile_glioma_mechanism_consensus(
    request: &MechanismConsensusRequest,
) -> Result<MechanismConsensus, MechanismConsensusError> {
    validate_request(request)?;
    let mut mechanisms = request.mechanisms.clone();
    mechanisms.sort();
    let source_values = source_means(&request.evidence, &mechanisms);
    let source_order = source_values.keys().cloned().collect::<Vec<_>>();
    let posterior = consensus_for_sources(&source_values, &mechanisms, None);
    let mut records = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for mechanism in &mechanisms {
        let source_packets = request
            .evidence
            .iter()
            .filter(|packet| packet.mechanism_id == *mechanism)
            .collect::<Vec<_>>();
        let mut leave_one_out = Vec::new();
        for source in &source_order {
            let without = consensus_for_sources(&source_values, &mechanisms, Some(source));
            leave_one_out.push(without.get(mechanism).copied().unwrap_or(0));
        }
        let min_loo = leave_one_out.iter().copied().min().unwrap_or(0);
        let max_loo = leave_one_out.iter().copied().max().unwrap_or(0);
        let negative_sources = source_packets
            .iter()
            .filter(|packet| packet.posterior_milli < 300)
            .map(|packet| packet.source_id.clone())
            .collect::<BTreeSet<_>>();
        if source_packets.len() < request.min_sources_per_mechanism {
            uncertainty.insert(format!(
                "mechanism:{mechanism}:sources-{}",
                source_packets.len()
            ));
        }
        if max_loo.saturating_sub(min_loo) > request.max_leave_one_out_milli {
            uncertainty.insert(format!(
                "mechanism:{mechanism}:leave-one-out-{}",
                max_loo.saturating_sub(min_loo)
            ));
        }
        for source in &negative_sources {
            negative_evidence.insert(format!("mechanism:{mechanism}:source:{source}"));
        }
        records.push(MechanismConsensusRecord {
            mechanism_id: mechanism.clone(),
            posterior_milli: posterior.get(mechanism).copied().unwrap_or(0),
            source_count: source_packets
                .iter()
                .map(|packet| packet.source_id.clone())
                .collect::<BTreeSet<_>>()
                .len(),
            weighted_support_milli: source_packets
                .iter()
                .map(|packet| {
                    u64::from(packet.posterior_milli) * u64::from(packet.reliability_milli)
                })
                .sum::<u64>(),
            leave_one_out_min_milli: min_loo,
            leave_one_out_max_milli: max_loo,
            negative_source_order: negative_sources.into_iter().collect(),
        });
    }
    let mut source_agreement = Vec::new();
    for source in &source_order {
        let packets = request
            .evidence
            .iter()
            .filter(|packet| packet.source_id == *source)
            .collect::<Vec<_>>();
        let group = packets
            .first()
            .map(|packet| packet.independence_group.clone())
            .unwrap_or_else(|| "unclassified".into());
        let values = source_values.get(source).cloned().unwrap_or_default();
        let covered = values
            .values()
            .filter(|(_, reliability)| *reliability > 0)
            .count();
        let coverage = ((covered * 1_000) / mechanisms.len()).min(1_000) as u16;
        source_agreement.push(MechanismSourceAgreement {
            source_id: source.clone(),
            independence_group: group,
            mechanism_count: covered,
            coverage_milli: coverage,
            conflict_milli: 0,
        });
    }
    let mut conflict_pairs = Vec::new();
    for left_index in 0..source_order.len() {
        for right_index in left_index + 1..source_order.len() {
            let left = source_values.get(&source_order[left_index]).unwrap();
            let right = source_values.get(&source_order[right_index]).unwrap();
            let divergence = mechanisms
                .iter()
                .map(|mechanism| {
                    let l = left
                        .get(mechanism)
                        .map(|(value, weight)| if *weight == 0 { 0 } else { value / weight })
                        .unwrap_or(0);
                    let r = right
                        .get(mechanism)
                        .map(|(value, weight)| if *weight == 0 { 0 } else { value / weight })
                        .unwrap_or(0);
                    l.abs_diff(r)
                })
                .sum::<u64>()
                / mechanisms.len().max(1) as u64;
            if divergence > u64::from(request.max_conflict_milli) {
                let key = format!("{}~{}", source_order[left_index], source_order[right_index]);
                conflict_pairs.push((key, divergence.min(1_000) as u16));
                uncertainty.insert(format!(
                    "sources:{}:conflict-{}",
                    source_order[left_index], divergence
                ));
            }
        }
    }
    conflict_pairs.sort_by(|left, right| left.0.cmp(&right.0));
    let conflict_pair_order = conflict_pairs
        .into_iter()
        .map(|(key, _)| key)
        .collect::<Vec<_>>();
    records.sort_by(|left, right| {
        right
            .posterior_milli
            .cmp(&left.posterior_milli)
            .then_with(|| left.mechanism_id.cmp(&right.mechanism_id))
    });
    let complete = records.iter().all(|record| {
        record.source_count >= request.min_sources_per_mechanism
            && record
                .leave_one_out_max_milli
                .saturating_sub(record.leave_one_out_min_milli)
                <= request.max_leave_one_out_milli
    }) && conflict_pair_order.is_empty();
    let disposition = if records.is_empty() {
        MechanismConsensusDisposition::Unresolved
    } else if complete {
        MechanismConsensusDisposition::Qualified
    } else {
        MechanismConsensusDisposition::Partial
    };
    let next_action = match disposition {
        MechanismConsensusDisposition::Qualified => {
            "send the source-weighted consensus to state smoothing, counterfactual analysis, and experiment selection"
        }
        MechanismConsensusDisposition::Partial => {
            "acquire an independent source or a conflict-discriminating feature before promoting consensus"
        }
        MechanismConsensusDisposition::Unresolved => {
            "provide at least one bounded, local, typed evidence packet per mechanism"
        }
    }
    .to_string();
    let mut result = MechanismConsensus {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order: mechanisms,
        records,
        source_order,
        source_agreement,
        conflict_pair_order,
        negative_evidence_order: negative_evidence.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-consensus"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| MechanismConsensusError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn packet(
        source: &str,
        evidence: &str,
        mechanism: &str,
        posterior: u16,
    ) -> MechanismEvidencePacket {
        MechanismEvidencePacket {
            source_id: source.into(),
            evidence_id: evidence.into(),
            mechanism_id: mechanism.into(),
            posterior_milli: posterior,
            reliability_milli: 900,
            independence_group: source.into(),
            artifact: artifact(evidence),
        }
    }

    fn request(evidence: Vec<MechanismEvidencePacket>) -> MechanismConsensusRequest {
        MechanismConsensusRequest {
            objective: "reconcile glioma invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec!["growth".into(), "stress".into()],
            evidence,
            min_sources_per_mechanism: 2,
            max_conflict_milli: 500,
            max_leave_one_out_milli: 500,
        }
    }

    #[test]
    fn consensus_is_source_weighted_and_normalized() {
        let result = compile_glioma_mechanism_consensus(&request(vec![
            packet("imaging", "i-growth", "growth", 900),
            packet("imaging", "i-stress", "stress", 100),
            packet("pathway", "p-growth", "growth", 800),
            packet("pathway", "p-stress", "stress", 200),
        ]))
        .unwrap();
        result.validate().unwrap();
        assert_eq!(result.disposition, MechanismConsensusDisposition::Qualified);
        assert_eq!(
            result
                .records
                .iter()
                .map(|record| u32::from(record.posterior_milli))
                .sum::<u32>(),
            1_000
        );
        assert_eq!(result.source_order, vec!["imaging", "pathway"]);
    }

    #[test]
    fn conflict_and_leave_one_out_sensitivity_remain_explicit() {
        let result = compile_glioma_mechanism_consensus(&request(vec![
            packet("imaging", "i-growth", "growth", 950),
            packet("imaging", "i-stress", "stress", 50),
            packet("pathway", "p-growth", "growth", 50),
            packet("pathway", "p-stress", "stress", 950),
        ]))
        .unwrap();
        assert_eq!(result.disposition, MechanismConsensusDisposition::Partial);
        assert!(!result.conflict_pair_order.is_empty());
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("conflict")));
        assert!(result.records.iter().any(|record| {
            record
                .leave_one_out_max_milli
                .saturating_sub(record.leave_one_out_min_milli)
                > 0
        }));
    }

    #[test]
    fn underpowered_and_negative_sources_are_not_silently_promoted() {
        let mut request = request(vec![
            packet("imaging", "i-growth", "growth", 900),
            packet("imaging", "i-stress", "stress", 100),
            packet("pathway", "p-growth", "growth", 100),
        ]);
        request.min_sources_per_mechanism = 2;
        let result = compile_glioma_mechanism_consensus(&request).unwrap();
        assert_eq!(result.disposition, MechanismConsensusDisposition::Partial);
        assert!(!result.negative_evidence_order.is_empty());
        assert!(result
            .uncertainty_order
            .iter()
            .any(|item| item.contains("sources")));
    }
}
