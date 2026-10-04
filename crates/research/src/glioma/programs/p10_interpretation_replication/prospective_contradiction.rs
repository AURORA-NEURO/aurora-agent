//! Prospective discrimination planning for conflicting preclinical glioma claims.
//!
//! No detailed source blueprint for P10-F03 is configured in this checkout. This repository-defined
//! scientific algorithm therefore accepts only caller-declared rival hypotheses and prospective
//! resolver predictions. It verifies source lineage, independent evidence floors, and prediction
//! interval separation, then compiles a bounded plan. A ready plan is not an executed experiment or
//! a resolved scientific claim.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::lineage_transport::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaProspectiveContradictionPlan1@1";
pub const MAX_RIVALS: usize = 16;
pub const MAX_EVIDENCE: usize = 16_384;
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_SELECTED_CANDIDATES: usize = 32;
pub const MAX_EFFECT_ABS_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalHypothesis {
    pub hypothesis_id: String,
    /// Digest of the exact claim statement retained by the upstream claim record.
    pub statement_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveContradictionRequest {
    pub objective: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub source_evidence_report_order: Vec<ContentHash>,
    pub resolver_design_report_order: Vec<ContentHash>,
    /// These hypotheses are declared mutually exclusive and are kept in identifier order.
    pub rival_hypotheses: Vec<RivalHypothesis>,
    pub min_independent_support_groups: usize,
    pub min_quality_milli: u16,
    pub min_prediction_separation_milli: u64,
    pub min_feasibility_milli: u16,
    pub max_risk_milli: u16,
    pub budget_units: u64,
    pub max_selected_candidates: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAssessment {
    Supports,
    Refutes,
    Null,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionEvidence {
    pub evidence_id: String,
    pub source_report_digest: ContentHash,
    pub hypothesis_id: String,
    pub hypothesis_statement_digest: ContentHash,
    pub estimand_id: String,
    pub effect_unit: String,
    pub study_id: String,
    pub independence_group: String,
    pub assessment: EvidenceAssessment,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolverPrediction {
    pub hypothesis_id: String,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolverCandidate {
    pub candidate_id: String,
    /// Candidates from one design family are alternatives; at most one may be selected.
    pub design_family_id: String,
    pub design_report_digest: ContentHash,
    pub predictions: Vec<ResolverPrediction>,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub protocol_artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RivalPair {
    pub first_hypothesis_id: String,
    pub second_hypothesis_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HypothesisEvidenceSummary {
    pub hypothesis_id: String,
    pub supporting_order: Vec<String>,
    pub refuting_order: Vec<String>,
    pub null_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub low_quality_order: Vec<String>,
    pub independent_support_group_count: usize,
    /// Support groups that do not also support another declared rival.
    pub exclusive_support_group_count: usize,
    pub independent_refuting_group_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RivalPairSeparation {
    pub pair: RivalPair,
    /// Distance between the two prediction intervals; overlapping intervals have zero margin.
    pub lower_bound_separation_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateEligibility {
    Eligible,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateAssessment {
    pub candidate_id: String,
    pub design_family_id: String,
    pub design_report_digest: ContentHash,
    pub protocol_artifact: LocalArtifactRef,
    pub pair_separations: Vec<RivalPairSeparation>,
    pub separable_pair_order: Vec<RivalPair>,
    pub feasibility_milli: u16,
    pub risk_milli: u16,
    pub cost_units: u32,
    pub eligibility: CandidateEligibility,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectedResolver {
    pub candidate_id: String,
    pub newly_covered_pair_order: Vec<RivalPair>,
    pub cost_units: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProspectiveContradictionDisposition {
    InsufficientEvidence,
    NoContradiction,
    NoDiscriminator,
    BudgetBlocked,
    Partial,
    PlanReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveContradictionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub request: ProspectiveContradictionRequest,
    pub input_digest: ContentHash,
    pub hypothesis_evidence: Vec<HypothesisEvidenceSummary>,
    pub contradiction_detected: bool,
    pub rival_pair_order: Vec<RivalPair>,
    pub candidate_order: Vec<String>,
    pub candidate_assessments: Vec<CandidateAssessment>,
    pub selected_order: Vec<String>,
    pub selected_resolvers: Vec<SelectedResolver>,
    pub covered_pair_order: Vec<RivalPair>,
    pub unresolved_pair_order: Vec<RivalPair>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProspectiveContradictionDisposition,
    pub budget_remaining_units: u64,
    /// Planning only. No experiment, assay, provider, or federation action is dispatched.
    pub dispatch: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProspectiveContradictionError {
    #[error("prospective contradiction request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective contradiction evidence is invalid: {0}")]
    InvalidEvidence(String),
    #[error("prospective resolver candidates are invalid: {0}")]
    InvalidCandidate(String),
    #[error("prospective contradiction output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective contradiction digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn validate_request(
    request: &ProspectiveContradictionRequest,
) -> Result<(), ProspectiveContradictionError> {
    let hypothesis_ids = request
        .rival_hypotheses
        .iter()
        .map(|item| item.hypothesis_id.as_str())
        .collect::<Vec<_>>();
    if request.objective.trim().is_empty()
        || request.estimand_id.trim().is_empty()
        || request.effect_unit.trim().is_empty()
        || !(2..=MAX_RIVALS).contains(&request.rival_hypotheses.len())
        || !canonical(&hypothesis_ids)
        || request
            .rival_hypotheses
            .iter()
            .any(|item| item.hypothesis_id.trim().is_empty() || !valid_hash(&item.statement_digest))
        || request.source_evidence_report_order.is_empty()
        || request.source_evidence_report_order.len() > MAX_EVIDENCE
        || !canonical(&request.source_evidence_report_order)
        || request
            .source_evidence_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || !canonical(&request.resolver_design_report_order)
        || request.resolver_design_report_order.len() > MAX_CANDIDATES
        || request
            .resolver_design_report_order
            .iter()
            .any(|digest| !valid_hash(digest))
        || !(2..=MAX_EVIDENCE).contains(&request.min_independent_support_groups)
        || request.min_quality_milli > 1_000
        || request.min_prediction_separation_milli == 0
        || request.min_prediction_separation_milli > MAX_EFFECT_ABS_MILLI.saturating_mul(2)
        || request.min_feasibility_milli > 1_000
        || request.max_risk_milli > 1_000
        || request.budget_units == 0
        || !(1..=MAX_SELECTED_CANDIDATES).contains(&request.max_selected_candidates)
    {
        return Err(ProspectiveContradictionError::InvalidRequest(
            "objective, estimand/unit, ordered digest-bound rivals and source reports, support/QC/separation gates, and bounded planning budget are required".into(),
        ));
    }
    Ok(())
}

fn validate_evidence(
    request: &ProspectiveContradictionRequest,
    evidence: &[ContradictionEvidence],
) -> Result<(), ProspectiveContradictionError> {
    if evidence.len() > MAX_EVIDENCE {
        return Err(ProspectiveContradictionError::InvalidEvidence(
            "evidence count exceeds the declared bound".into(),
        ));
    }
    let hypotheses = request
        .rival_hypotheses
        .iter()
        .map(|item| (item.hypothesis_id.as_str(), &item.statement_digest))
        .collect::<BTreeMap<_, _>>();
    let source_reports = request
        .source_evidence_report_order
        .iter()
        .collect::<BTreeSet<_>>();
    let mut evidence_ids = BTreeSet::new();
    for item in evidence {
        let Some(expected_statement_digest) = hypotheses.get(item.hypothesis_id.as_str()) else {
            return Err(ProspectiveContradictionError::InvalidEvidence(
                "evidence references an undeclared rival hypothesis".into(),
            ));
        };
        if item.evidence_id.trim().is_empty()
            || !evidence_ids.insert(item.evidence_id.as_str())
            || item.study_id.trim().is_empty()
            || item.independence_group.trim().is_empty()
            || item.hypothesis_statement_digest != **expected_statement_digest
            || item.estimand_id != request.estimand_id
            || item.effect_unit != request.effect_unit
            || !source_reports.contains(&&item.source_report_digest)
            || !valid_hash(&item.source_report_digest)
            || item.quality_milli > 1_000
            || item.artifact.validate().is_err()
            || !item.artifact.local_only
            || item.artifact.contains_human_data
            || item.artifact.contains_direct_identifiers
        {
            return Err(ProspectiveContradictionError::InvalidEvidence(
                "each finding must bind a declared claim, estimand, unit, upstream report, independent group, and local de-identified artifact".into(),
            ));
        }
    }
    Ok(())
}

fn validate_candidates(
    request: &ProspectiveContradictionRequest,
    candidates: &[ResolverCandidate],
) -> Result<(), ProspectiveContradictionError> {
    if candidates.len() > MAX_CANDIDATES {
        return Err(ProspectiveContradictionError::InvalidCandidate(
            "candidate count exceeds the declared bound".into(),
        ));
    }
    let hypothesis_ids = request
        .rival_hypotheses
        .iter()
        .map(|item| item.hypothesis_id.as_str())
        .collect::<Vec<_>>();
    let design_reports = request
        .resolver_design_report_order
        .iter()
        .collect::<BTreeSet<_>>();
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        let prediction_ids = candidate
            .predictions
            .iter()
            .map(|prediction| prediction.hypothesis_id.as_str())
            .collect::<Vec<_>>();
        if candidate.candidate_id.trim().is_empty()
            || !candidate_ids.insert(candidate.candidate_id.as_str())
            || candidate.design_family_id.trim().is_empty()
            || !design_reports.contains(&&candidate.design_report_digest)
            || !valid_hash(&candidate.design_report_digest)
            || prediction_ids != hypothesis_ids
            || candidate.predictions.iter().any(|prediction| {
                prediction.effect_milli.unsigned_abs() > MAX_EFFECT_ABS_MILLI
                    || prediction.uncertainty_milli > MAX_EFFECT_ABS_MILLI
            })
            || candidate.feasibility_milli > 1_000
            || candidate.risk_milli > 1_000
            || candidate.cost_units == 0
            || candidate.protocol_artifact.validate().is_err()
            || !candidate.protocol_artifact.local_only
            || candidate.protocol_artifact.contains_human_data
            || candidate.protocol_artifact.contains_direct_identifiers
        {
            return Err(ProspectiveContradictionError::InvalidCandidate(
                "each candidate needs one ordered bounded prediction per rival, a declared P06 design report, bounded feasibility/risk/cost, and a local de-identified protocol artifact".into(),
            ));
        }
    }
    Ok(())
}

fn input_digest(
    request: &ProspectiveContradictionRequest,
    evidence: &[ContradictionEvidence],
    candidates: &[ResolverCandidate],
) -> Result<ContentHash, ProspectiveContradictionError> {
    #[derive(Serialize)]
    struct Input<'a> {
        input_schema: &'static str,
        request: &'a ProspectiveContradictionRequest,
        evidence: &'a [ContradictionEvidence],
        candidates: &'a [ResolverCandidate],
    }
    let mut ordered_evidence = evidence.to_vec();
    ordered_evidence.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let mut ordered_candidates = candidates.to_vec();
    ordered_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    ContentHash::of_serializable(&Input {
        input_schema: "GliomaProspectiveContradictionInput1@1",
        request,
        evidence: &ordered_evidence,
        candidates: &ordered_candidates,
    })
    .map_err(|error| ProspectiveContradictionError::Digest(error.to_string()))
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a ProspectiveContradictionRequest,
    input_digest: &'a ContentHash,
    hypothesis_evidence: &'a [HypothesisEvidenceSummary],
    contradiction_detected: bool,
    rival_pair_order: &'a [RivalPair],
    candidate_order: &'a [String],
    candidate_assessments: &'a [CandidateAssessment],
    selected_order: &'a [String],
    selected_resolvers: &'a [SelectedResolver],
    covered_pair_order: &'a [RivalPair],
    unresolved_pair_order: &'a [RivalPair],
    negative_evidence: &'a [String],
    uncertainty: &'a [String],
    disposition: ProspectiveContradictionDisposition,
    budget_remaining_units: u64,
    dispatch: &'a str,
}

fn output_digest(
    output: &ProspectiveContradictionPlan,
) -> Result<ContentHash, ProspectiveContradictionError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &output.feature_id,
        output_schema: &output.output_schema,
        request: &output.request,
        input_digest: &output.input_digest,
        hypothesis_evidence: &output.hypothesis_evidence,
        contradiction_detected: output.contradiction_detected,
        rival_pair_order: &output.rival_pair_order,
        candidate_order: &output.candidate_order,
        candidate_assessments: &output.candidate_assessments,
        selected_order: &output.selected_order,
        selected_resolvers: &output.selected_resolvers,
        covered_pair_order: &output.covered_pair_order,
        unresolved_pair_order: &output.unresolved_pair_order,
        negative_evidence: &output.negative_evidence,
        uncertainty: &output.uncertainty,
        disposition: output.disposition,
        budget_remaining_units: output.budget_remaining_units,
        dispatch: &output.dispatch,
    })
    .map_err(|error| ProspectiveContradictionError::Digest(error.to_string()))
}

#[derive(Default)]
struct GroupAssessment {
    supports: bool,
    refutes: bool,
    null: bool,
    unresolved: bool,
}

fn summarize_evidence(
    request: &ProspectiveContradictionRequest,
    evidence: &[ContradictionEvidence],
) -> Vec<HypothesisEvidenceSummary> {
    let mut summaries = Vec::with_capacity(request.rival_hypotheses.len());
    let mut clean_support_groups = BTreeMap::<String, BTreeSet<String>>::new();
    for hypothesis in &request.rival_hypotheses {
        let rows = evidence
            .iter()
            .filter(|item| item.hypothesis_id == hypothesis.hypothesis_id)
            .collect::<Vec<_>>();
        let mut groups = BTreeMap::<String, GroupAssessment>::new();
        let mut summary = HypothesisEvidenceSummary {
            hypothesis_id: hypothesis.hypothesis_id.clone(),
            supporting_order: Vec::new(),
            refuting_order: Vec::new(),
            null_order: Vec::new(),
            unresolved_order: Vec::new(),
            low_quality_order: Vec::new(),
            independent_support_group_count: 0,
            exclusive_support_group_count: 0,
            independent_refuting_group_count: 0,
        };
        for row in rows {
            if row.quality_milli < request.min_quality_milli {
                summary.low_quality_order.push(row.evidence_id.clone());
                continue;
            }
            let group = groups.entry(row.independence_group.clone()).or_default();
            match row.assessment {
                EvidenceAssessment::Supports => {
                    summary.supporting_order.push(row.evidence_id.clone());
                    group.supports = true;
                }
                EvidenceAssessment::Refutes => {
                    summary.refuting_order.push(row.evidence_id.clone());
                    group.refutes = true;
                }
                EvidenceAssessment::Null => {
                    summary.null_order.push(row.evidence_id.clone());
                    group.null = true;
                }
                EvidenceAssessment::Unresolved => {
                    summary.unresolved_order.push(row.evidence_id.clone());
                    group.unresolved = true;
                }
            }
        }
        for (group_id, group) in groups {
            // Conflicting classifications from one independent source cannot manufacture either
            // support or refutation; preserve that source as unresolved instead.
            if group.supports && !group.refutes && !group.null && !group.unresolved {
                clean_support_groups
                    .entry(hypothesis.hypothesis_id.clone())
                    .or_default()
                    .insert(group_id);
            }
            if group.refutes && !group.supports && !group.null && !group.unresolved {
                summary.independent_refuting_group_count += 1;
            }
        }
        summary.independent_support_group_count = clean_support_groups
            .get(&hypothesis.hypothesis_id)
            .map_or(0, BTreeSet::len);
        summaries.push(summary);
    }
    // A group that supports more than one rival cannot establish independent support for either
    // side of a declared mutually exclusive contrast.
    let mut group_owners = BTreeMap::<String, usize>::new();
    for groups in clean_support_groups.values() {
        for group in groups {
            *group_owners.entry(group.clone()).or_default() += 1;
        }
    }
    for summary in &mut summaries {
        summary.exclusive_support_group_count = clean_support_groups
            .get(&summary.hypothesis_id)
            .map_or(0, |groups| {
                groups
                    .iter()
                    .filter(|group| group_owners.get(*group) == Some(&1))
                    .count()
            });
        summary.supporting_order.sort();
        summary.refuting_order.sort();
        summary.null_order.sort();
        summary.unresolved_order.sort();
        summary.low_quality_order.sort();
    }
    summaries
}

fn rival_pairs(hypotheses: &[RivalHypothesis]) -> Vec<RivalPair> {
    let mut pairs = Vec::new();
    for first in 0..hypotheses.len() {
        for second in (first + 1)..hypotheses.len() {
            pairs.push(RivalPair {
                first_hypothesis_id: hypotheses[first].hypothesis_id.clone(),
                second_hypothesis_id: hypotheses[second].hypothesis_id.clone(),
            });
        }
    }
    pairs
}

fn separation(left: &ResolverPrediction, right: &ResolverPrediction) -> u64 {
    let center_distance =
        (i128::from(left.effect_milli) - i128::from(right.effect_milli)).unsigned_abs();
    center_distance
        .saturating_sub(u128::from(left.uncertainty_milli))
        .saturating_sub(u128::from(right.uncertainty_milli))
        .min(u128::from(u64::MAX)) as u64
}

fn assess_candidate(
    request: &ProspectiveContradictionRequest,
    pairs: &[RivalPair],
    candidate: &ResolverCandidate,
) -> CandidateAssessment {
    let predictions = candidate
        .predictions
        .iter()
        .map(|prediction| (prediction.hypothesis_id.as_str(), prediction))
        .collect::<BTreeMap<_, _>>();
    let pair_separations = pairs
        .iter()
        .map(|pair| RivalPairSeparation {
            pair: pair.clone(),
            lower_bound_separation_milli: separation(
                predictions[pair.first_hypothesis_id.as_str()],
                predictions[pair.second_hypothesis_id.as_str()],
            ),
        })
        .collect::<Vec<_>>();
    let separable_pair_order = pair_separations
        .iter()
        .filter(|item| item.lower_bound_separation_milli >= request.min_prediction_separation_milli)
        .map(|item| item.pair.clone())
        .collect::<Vec<_>>();
    let mut reason_order = Vec::new();
    if candidate.feasibility_milli < request.min_feasibility_milli {
        reason_order.push("below_minimum_feasibility".into());
    }
    if candidate.risk_milli > request.max_risk_milli {
        reason_order.push("above_maximum_risk".into());
    }
    if separable_pair_order.is_empty() {
        reason_order.push("no_rival_pair_prediction_intervals_are_separated".into());
    }
    reason_order.sort();
    CandidateAssessment {
        candidate_id: candidate.candidate_id.clone(),
        design_family_id: candidate.design_family_id.clone(),
        design_report_digest: candidate.design_report_digest.clone(),
        protocol_artifact: candidate.protocol_artifact.clone(),
        pair_separations,
        separable_pair_order,
        feasibility_milli: candidate.feasibility_milli,
        risk_milli: candidate.risk_milli,
        cost_units: candidate.cost_units,
        eligibility: if reason_order.is_empty() {
            CandidateEligibility::Eligible
        } else {
            CandidateEligibility::Rejected
        },
        reason_order,
    }
}

fn compile(
    request: &ProspectiveContradictionRequest,
    evidence: &[ContradictionEvidence],
    candidates: &[ResolverCandidate],
) -> Result<ProspectiveContradictionPlan, ProspectiveContradictionError> {
    let hypothesis_evidence = summarize_evidence(request, evidence);
    let supported = hypothesis_evidence
        .iter()
        .filter(|summary| {
            summary.exclusive_support_group_count >= request.min_independent_support_groups
        })
        .collect::<Vec<_>>();
    let contradiction_detected = supported.len() >= 2
        || supported
            .iter()
            .any(|summary| summary.independent_refuting_group_count > 0);
    let disposition_without_candidates = if supported.is_empty() {
        Some(ProspectiveContradictionDisposition::InsufficientEvidence)
    } else if !contradiction_detected {
        Some(ProspectiveContradictionDisposition::NoContradiction)
    } else {
        None
    };
    let pairs = rival_pairs(&request.rival_hypotheses);
    let mut ordered_candidates = candidates.to_vec();
    ordered_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_assessments = ordered_candidates
        .iter()
        .map(|candidate| assess_candidate(request, &pairs, candidate))
        .collect::<Vec<_>>();
    let candidate_order = candidate_assessments
        .iter()
        .map(|assessment| assessment.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut selected_resolvers = Vec::<SelectedResolver>::new();
    let mut covered = BTreeSet::<RivalPair>::new();
    let mut used_families = BTreeSet::<String>::new();
    let mut spent = 0_u64;
    if contradiction_detected {
        while selected_resolvers.len() < request.max_selected_candidates {
            let mut options = candidate_assessments
                .iter()
                .filter(|candidate| {
                    candidate.eligibility == CandidateEligibility::Eligible
                        && !used_families.contains(&candidate.design_family_id)
                        && u64::from(candidate.cost_units)
                            <= request.budget_units.saturating_sub(spent)
                })
                .filter_map(|candidate| {
                    let newly_covered = candidate
                        .separable_pair_order
                        .iter()
                        .filter(|pair| !covered.contains(*pair))
                        .cloned()
                        .collect::<Vec<_>>();
                    (!newly_covered.is_empty()).then_some((candidate, newly_covered))
                })
                .collect::<Vec<_>>();
            if options.is_empty() {
                break;
            }
            options.sort_by(|(left, left_pairs), (right, right_pairs)| {
                let left_ratio = (left_pairs.len() as u128) * u128::from(right.cost_units);
                let right_ratio = (right_pairs.len() as u128) * u128::from(left.cost_units);
                right_ratio
                    .cmp(&left_ratio)
                    .then_with(|| right.feasibility_milli.cmp(&left.feasibility_milli))
                    .then_with(|| left.risk_milli.cmp(&right.risk_milli))
                    .then_with(|| left.candidate_id.cmp(&right.candidate_id))
            });
            let (candidate, newly_covered_pair_order) = options.remove(0);
            spent = spent.saturating_add(u64::from(candidate.cost_units));
            used_families.insert(candidate.design_family_id.clone());
            covered.extend(newly_covered_pair_order.iter().cloned());
            selected_resolvers.push(SelectedResolver {
                candidate_id: candidate.candidate_id.clone(),
                newly_covered_pair_order,
                cost_units: candidate.cost_units,
            });
        }
    }
    let selected_order = selected_resolvers
        .iter()
        .map(|selected| selected.candidate_id.clone())
        .collect::<Vec<_>>();
    let covered_pair_order = pairs
        .iter()
        .filter(|pair| covered.contains(*pair))
        .cloned()
        .collect::<Vec<_>>();
    let unresolved_pair_order = pairs
        .iter()
        .filter(|pair| !covered.contains(*pair))
        .cloned()
        .collect::<Vec<_>>();
    let eligible_candidates_exist = candidate_assessments
        .iter()
        .any(|assessment| assessment.eligibility == CandidateEligibility::Eligible);
    let affordable_candidates_exist = candidate_assessments.iter().any(|assessment| {
        assessment.eligibility == CandidateEligibility::Eligible
            && u64::from(assessment.cost_units) <= request.budget_units
    });
    let disposition = disposition_without_candidates.unwrap_or({
        if covered_pair_order.len() == pairs.len() {
            ProspectiveContradictionDisposition::PlanReady
        } else if !selected_resolvers.is_empty() {
            ProspectiveContradictionDisposition::Partial
        } else if eligible_candidates_exist && !affordable_candidates_exist {
            ProspectiveContradictionDisposition::BudgetBlocked
        } else {
            ProspectiveContradictionDisposition::NoDiscriminator
        }
    });
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    match disposition {
        ProspectiveContradictionDisposition::InsufficientEvidence => {
            negative_evidence
                .push("no_rival_has_the_required_exclusive_independent_support".into());
        }
        ProspectiveContradictionDisposition::NoContradiction => {
            negative_evidence
                .push("no_declared_rival_conflict_met_the_independent_evidence_gate".into());
        }
        ProspectiveContradictionDisposition::NoDiscriminator => {
            negative_evidence.push("no_feasible_bounded_resolver_separates_a_rival_pair".into());
        }
        ProspectiveContradictionDisposition::BudgetBlocked => {
            negative_evidence.push("all_eligible_resolvers_exceed_the_available_budget".into());
        }
        ProspectiveContradictionDisposition::Partial => {
            negative_evidence.push("available_resolvers_do_not_cover_every_rival_pair".into());
        }
        ProspectiveContradictionDisposition::PlanReady => {}
    }
    if hypothesis_evidence.iter().any(|summary| {
        !summary.low_quality_order.is_empty()
            || !summary.null_order.is_empty()
            || !summary.unresolved_order.is_empty()
            || summary.independent_support_group_count > summary.exclusive_support_group_count
    }) {
        uncertainty
            .push("low_quality_neutral_unresolved_or_shared_group_evidence_was_retained".into());
    }
    if !unresolved_pair_order.is_empty() && contradiction_detected {
        uncertainty
            .push("some_declared_rival_pairs_remain_unresolved_by_the_selected_portfolio".into());
    }
    let mut output = ProspectiveContradictionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        input_digest: input_digest(request, evidence, candidates)?,
        hypothesis_evidence,
        contradiction_detected,
        rival_pair_order: pairs,
        candidate_order,
        candidate_assessments,
        selected_order,
        selected_resolvers,
        covered_pair_order,
        unresolved_pair_order,
        negative_evidence,
        uncertainty,
        disposition,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        dispatch: "not_dispatched_planning_only".into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-prospective-contradiction"),
    };
    output.digest = output_digest(&output)?;
    Ok(output)
}

impl ProspectiveContradictionPlan {
    pub fn validate(&self) -> Result<(), ProspectiveContradictionError> {
        validate_request(&self.request)?;
        let expected_pairs = rival_pairs(&self.request.rival_hypotheses);
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_hash(&self.input_digest)
            || self.rival_pair_order != expected_pairs
            || !canonical(&self.candidate_order)
            || self.candidate_order
                != self
                    .candidate_assessments
                    .iter()
                    .map(|item| item.candidate_id.clone())
                    .collect::<Vec<_>>()
            || self.selected_order
                != self
                    .selected_resolvers
                    .iter()
                    .map(|item| item.candidate_id.clone())
                    .collect::<Vec<_>>()
            || !canonical(&self.covered_pair_order)
            || !canonical(&self.unresolved_pair_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.dispatch != "not_dispatched_planning_only"
        {
            return Err(ProspectiveContradictionError::InvalidOutput(
                "identity, lineage digest, pair order, candidate order, dispatch boundary, or canonical ordering is invalid".into(),
            ));
        }
        if self.hypothesis_evidence.len() != self.request.rival_hypotheses.len()
            || self
                .hypothesis_evidence
                .iter()
                .zip(&self.request.rival_hypotheses)
                .any(|(summary, rival)| {
                    summary.hypothesis_id != rival.hypothesis_id
                        || summary.independent_support_group_count
                            < summary.exclusive_support_group_count
                        || !canonical(&summary.supporting_order)
                        || !canonical(&summary.refuting_order)
                        || !canonical(&summary.null_order)
                        || !canonical(&summary.unresolved_order)
                        || !canonical(&summary.low_quality_order)
                })
            || self.candidate_assessments.iter().any(|assessment| {
                assessment.candidate_id.trim().is_empty()
                    || assessment.design_family_id.trim().is_empty()
                    || assessment.design_report_digest.as_str().len() != 64
                    || assessment.cost_units == 0
                    || assessment.feasibility_milli > 1_000
                    || assessment.risk_milli > 1_000
                    || assessment.protocol_artifact.validate().is_err()
                    || assessment
                        .pair_separations
                        .iter()
                        .map(|item| item.pair.clone())
                        .collect::<Vec<_>>()
                        != expected_pairs
                    || !canonical(&assessment.separable_pair_order)
                    || assessment.separable_pair_order
                        != assessment
                            .pair_separations
                            .iter()
                            .filter(|item| {
                                item.lower_bound_separation_milli
                                    >= self.request.min_prediction_separation_milli
                            })
                            .map(|item| item.pair.clone())
                            .collect::<Vec<_>>()
                    || !canonical(&assessment.reason_order)
                    || assessment.eligibility == CandidateEligibility::Eligible
                        && !assessment.reason_order.is_empty()
                    || assessment.eligibility == CandidateEligibility::Rejected
                        && assessment.reason_order.is_empty()
            })
        {
            return Err(ProspectiveContradictionError::InvalidOutput(
                "evidence summaries or candidate assessments violate their declared bounds".into(),
            ));
        }
        let selected = self
            .selected_resolvers
            .iter()
            .map(|item| item.candidate_id.as_str())
            .collect::<BTreeSet<_>>();
        let eligible_by_id = self
            .candidate_assessments
            .iter()
            .map(|item| (item.candidate_id.as_str(), item))
            .collect::<BTreeMap<_, _>>();
        let mut covered = BTreeSet::new();
        let mut families = BTreeSet::new();
        let mut spent = 0_u64;
        for selected_item in &self.selected_resolvers {
            let Some(assessment) = eligible_by_id.get(selected_item.candidate_id.as_str()) else {
                return Err(ProspectiveContradictionError::InvalidOutput(
                    "selected resolver is absent from the candidate ledger".into(),
                ));
            };
            if assessment.eligibility != CandidateEligibility::Eligible
                || assessment.cost_units != selected_item.cost_units
                || !families.insert(assessment.design_family_id.as_str())
                || selected_item.newly_covered_pair_order.is_empty()
                || selected_item.newly_covered_pair_order.iter().any(|pair| {
                    !assessment.separable_pair_order.contains(pair) || !covered.insert(pair.clone())
                })
            {
                return Err(ProspectiveContradictionError::InvalidOutput(
                    "selected resolver coverage, family, eligibility, or cost is inconsistent"
                        .into(),
                ));
            }
            spent = spent.saturating_add(u64::from(selected_item.cost_units));
        }
        let expected_covered = expected_pairs
            .iter()
            .filter(|pair| covered.contains(*pair))
            .cloned()
            .collect::<Vec<_>>();
        let expected_unresolved = expected_pairs
            .iter()
            .filter(|pair| !covered.contains(*pair))
            .cloned()
            .collect::<Vec<_>>();
        let supported = self
            .hypothesis_evidence
            .iter()
            .filter(|summary| {
                summary.exclusive_support_group_count >= self.request.min_independent_support_groups
            })
            .collect::<Vec<_>>();
        let contradiction = supported.len() >= 2
            || supported
                .iter()
                .any(|summary| summary.independent_refuting_group_count > 0);
        let expected_disposition = if supported.is_empty() {
            ProspectiveContradictionDisposition::InsufficientEvidence
        } else if !contradiction {
            ProspectiveContradictionDisposition::NoContradiction
        } else if covered.len() == expected_pairs.len() {
            ProspectiveContradictionDisposition::PlanReady
        } else if !self.selected_resolvers.is_empty() {
            ProspectiveContradictionDisposition::Partial
        } else if self
            .candidate_assessments
            .iter()
            .any(|item| item.eligibility == CandidateEligibility::Eligible)
            && !self.candidate_assessments.iter().any(|item| {
                item.eligibility == CandidateEligibility::Eligible
                    && u64::from(item.cost_units) <= self.request.budget_units
            })
        {
            ProspectiveContradictionDisposition::BudgetBlocked
        } else {
            ProspectiveContradictionDisposition::NoDiscriminator
        };
        if self.contradiction_detected != contradiction
            || self.covered_pair_order != expected_covered
            || self.unresolved_pair_order != expected_unresolved
            || self.selected_order.len() != selected.len()
            || self.selected_order.len() > self.request.max_selected_candidates
            || spent > self.request.budget_units
            || self.budget_remaining_units != self.request.budget_units.saturating_sub(spent)
            || self.disposition != expected_disposition
            || output_digest(self)? != self.digest
        {
            return Err(ProspectiveContradictionError::InvalidOutput(
                "coverage, evidence gate, budget, disposition, or content digest is inconsistent"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Verify the plan against the exact supplied evidence and candidate ledger.
    pub fn validate_against(
        &self,
        request: &ProspectiveContradictionRequest,
        evidence: &[ContradictionEvidence],
        candidates: &[ResolverCandidate],
    ) -> Result<(), ProspectiveContradictionError> {
        self.validate()?;
        validate_request(request)?;
        validate_evidence(request, evidence)?;
        validate_candidates(request, candidates)?;
        if &self.request != request
            || input_digest(request, evidence, candidates)? != self.input_digest
        {
            return Err(ProspectiveContradictionError::InvalidOutput(
                "plan input digest does not match the supplied request, evidence, and resolver designs".into(),
            ));
        }
        let expected = compile(request, evidence, candidates)?;
        if self != &expected {
            return Err(ProspectiveContradictionError::InvalidOutput(
                "candidate assessments, evidence summaries, selection, or lineage do not derive from supplied inputs".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a bounded, replayable prospective plan. No experimental action is dispatched.
pub fn analyze_glioma_prospective_contradiction(
    request: &ProspectiveContradictionRequest,
    evidence: &[ContradictionEvidence],
    candidates: &[ResolverCandidate],
) -> Result<ProspectiveContradictionPlan, ProspectiveContradictionError> {
    validate_request(request)?;
    validate_evidence(request, evidence)?;
    validate_candidates(request, candidates)?;
    let output = compile(request, evidence, candidates)?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: label.into(),
            content_hash: hash(label),
            content_type: "application/vnd.aurora.local-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> ProspectiveContradictionRequest {
        ProspectiveContradictionRequest {
            objective: "discriminate declared preclinical response claims".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            source_evidence_report_order: vec![hash("source-report")],
            resolver_design_report_order: vec![hash("design-report")],
            rival_hypotheses: vec![
                RivalHypothesis {
                    hypothesis_id: "A".into(),
                    statement_digest: hash("claim-A"),
                },
                RivalHypothesis {
                    hypothesis_id: "B".into(),
                    statement_digest: hash("claim-B"),
                },
                RivalHypothesis {
                    hypothesis_id: "C".into(),
                    statement_digest: hash("claim-C"),
                },
            ],
            min_independent_support_groups: 2,
            min_quality_milli: 800,
            min_prediction_separation_milli: 100,
            min_feasibility_milli: 600,
            max_risk_milli: 500,
            budget_units: 10,
            max_selected_candidates: 3,
        }
    }

    fn evidence(
        id: &str,
        hypothesis: &str,
        group: &str,
        assessment: EvidenceAssessment,
        quality_milli: u16,
    ) -> ContradictionEvidence {
        let request = request();
        let rival = request
            .rival_hypotheses
            .iter()
            .find(|rival| rival.hypothesis_id == hypothesis)
            .unwrap();
        ContradictionEvidence {
            evidence_id: id.into(),
            source_report_digest: hash("source-report"),
            hypothesis_id: hypothesis.into(),
            hypothesis_statement_digest: rival.statement_digest.clone(),
            estimand_id: request.estimand_id,
            effect_unit: request.effect_unit,
            study_id: format!("study-{group}"),
            independence_group: group.into(),
            assessment,
            quality_milli,
            artifact: artifact(id),
        }
    }

    fn supported_evidence() -> Vec<ContradictionEvidence> {
        vec![
            evidence("a-1", "A", "a-site-1", EvidenceAssessment::Supports, 950),
            evidence("a-2", "A", "a-site-2", EvidenceAssessment::Supports, 900),
            evidence("b-1", "B", "b-site-1", EvidenceAssessment::Supports, 950),
            evidence("b-2", "B", "b-site-2", EvidenceAssessment::Supports, 900),
        ]
    }

    fn candidate(
        id: &str,
        effects: [i64; 3],
        uncertainty: u64,
        feasibility_milli: u16,
        risk_milli: u16,
        cost_units: u32,
    ) -> ResolverCandidate {
        ResolverCandidate {
            candidate_id: id.into(),
            design_family_id: format!("family-{id}"),
            design_report_digest: hash("design-report"),
            predictions: ["A", "B", "C"]
                .into_iter()
                .zip(effects)
                .map(|(hypothesis_id, effect_milli)| ResolverPrediction {
                    hypothesis_id: hypothesis_id.into(),
                    effect_milli,
                    uncertainty_milli: uncertainty,
                })
                .collect(),
            feasibility_milli,
            risk_milli,
            cost_units,
            protocol_artifact: artifact(&format!("protocol-{id}")),
        }
    }

    #[test]
    fn plans_ready_portfolio_and_replays_independently_of_input_row_order() {
        let request = request();
        let evidence = supported_evidence();
        let candidates = vec![
            candidate("all-pairs", [0, 500, 1_000], 10, 900, 100, 4),
            candidate("partial", [0, 500, 0], 10, 900, 100, 3),
        ];
        let plan =
            analyze_glioma_prospective_contradiction(&request, &evidence, &candidates).unwrap();
        assert!(plan.contradiction_detected);
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::PlanReady
        );
        assert_eq!(plan.covered_pair_order.len(), 3);
        assert_eq!(plan.selected_order, vec!["all-pairs"]);
        assert_eq!(plan.budget_remaining_units, 6);
        assert_eq!(plan.dispatch, "not_dispatched_planning_only");
        plan.validate_against(&request, &evidence, &candidates)
            .unwrap();

        let mut shuffled_evidence = evidence.clone();
        shuffled_evidence.reverse();
        let mut shuffled_candidates = candidates.clone();
        shuffled_candidates.reverse();
        let replay = analyze_glioma_prospective_contradiction(
            &request,
            &shuffled_evidence,
            &shuffled_candidates,
        )
        .unwrap();
        assert_eq!(plan, replay);
    }

    #[test]
    fn shared_groups_cannot_manufacture_exclusive_support_for_two_rivals() {
        let request = request();
        let evidence = vec![
            evidence(
                "a-shared",
                "A",
                "shared-site",
                EvidenceAssessment::Supports,
                950,
            ),
            evidence(
                "a-private",
                "A",
                "a-private",
                EvidenceAssessment::Supports,
                950,
            ),
            evidence(
                "b-shared",
                "B",
                "shared-site",
                EvidenceAssessment::Supports,
                950,
            ),
            evidence(
                "b-private",
                "B",
                "b-private",
                EvidenceAssessment::Supports,
                950,
            ),
        ];
        let plan = analyze_glioma_prospective_contradiction(&request, &evidence, &[]).unwrap();
        assert!(!plan.contradiction_detected);
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::InsufficientEvidence
        );
        assert_eq!(
            plan.hypothesis_evidence[0].independent_support_group_count,
            2
        );
        assert_eq!(plan.hypothesis_evidence[0].exclusive_support_group_count, 1);
        assert_eq!(plan.hypothesis_evidence[1].exclusive_support_group_count, 1);
        plan.validate_against(&request, &evidence, &[]).unwrap();
    }

    #[test]
    fn preserves_low_quality_null_and_unresolved_rows_without_promoting_them() {
        let request = request();
        let evidence = vec![
            evidence("low-a", "A", "low-site", EvidenceAssessment::Supports, 200),
            evidence("null-b", "B", "null-site", EvidenceAssessment::Null, 950),
            evidence(
                "unresolved-c",
                "C",
                "uncertain-site",
                EvidenceAssessment::Unresolved,
                950,
            ),
        ];
        let plan = analyze_glioma_prospective_contradiction(&request, &evidence, &[]).unwrap();
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::InsufficientEvidence
        );
        assert_eq!(plan.hypothesis_evidence[0].low_quality_order, vec!["low-a"]);
        assert_eq!(plan.hypothesis_evidence[1].null_order, vec!["null-b"]);
        assert_eq!(
            plan.hypothesis_evidence[2].unresolved_order,
            vec!["unresolved-c"]
        );
        assert!(!plan.contradiction_detected);
        plan.validate_against(&request, &evidence, &[]).unwrap();
    }

    #[test]
    fn one_supported_rival_with_independent_refutation_is_a_contradiction() {
        let request = request();
        let evidence = vec![
            evidence("a-1", "A", "site-1", EvidenceAssessment::Supports, 950),
            evidence("a-2", "A", "site-2", EvidenceAssessment::Supports, 950),
            evidence("a-refute", "A", "site-3", EvidenceAssessment::Refutes, 950),
        ];
        let plan = analyze_glioma_prospective_contradiction(&request, &evidence, &[]).unwrap();
        assert!(plan.contradiction_detected);
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::NoDiscriminator
        );
        assert_eq!(
            plan.hypothesis_evidence[0].independent_refuting_group_count,
            1
        );
    }

    #[test]
    fn candidate_gates_reject_risk_feasibility_and_overlapping_intervals() {
        let request = request();
        let evidence = supported_evidence();
        let candidates = vec![
            candidate("risk", [0, 500, 1_000], 10, 900, 900, 1),
            candidate("feasibility", [0, 500, 1_000], 10, 100, 100, 1),
            candidate("overlap", [0, 50, 100], 30, 900, 100, 1),
        ];
        let plan =
            analyze_glioma_prospective_contradiction(&request, &evidence, &candidates).unwrap();
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::NoDiscriminator
        );
        assert!(plan.selected_order.is_empty());
        assert!(plan.candidate_assessments.iter().all(|assessment| {
            assessment.eligibility == CandidateEligibility::Rejected
                && !assessment.reason_order.is_empty()
        }));
    }

    #[test]
    fn portfolio_selection_is_partial_when_budget_covers_only_the_best_pair_set() {
        let mut request = request();
        request.budget_units = 1;
        let evidence = supported_evidence();
        let candidates = vec![
            candidate("ab-bc", [0, 500, 0], 10, 900, 100, 1),
            candidate("ac-only", [0, 100, 1_000], 10, 900, 100, 1),
        ];
        let plan =
            analyze_glioma_prospective_contradiction(&request, &evidence, &candidates).unwrap();
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::Partial
        );
        assert_eq!(plan.selected_order, vec!["ab-bc"]);
        assert_eq!(plan.covered_pair_order.len(), 2);
        assert_eq!(plan.unresolved_pair_order.len(), 1);
        plan.validate_against(&request, &evidence, &candidates)
            .unwrap();
    }

    #[test]
    fn reports_budget_blocked_and_detects_tampered_output() {
        let mut request = request();
        request.budget_units = 1;
        let evidence = supported_evidence();
        let candidates = vec![candidate("expensive", [0, 500, 1_000], 10, 900, 100, 2)];
        let mut plan =
            analyze_glioma_prospective_contradiction(&request, &evidence, &candidates).unwrap();
        assert_eq!(
            plan.disposition,
            ProspectiveContradictionDisposition::BudgetBlocked
        );
        plan.candidate_assessments[0].risk_milli = 499;
        assert!(plan.validate().is_err());
        assert!(
            plan.validate_against(&request, &evidence, &candidates)
                .is_err()
        );
    }
}
