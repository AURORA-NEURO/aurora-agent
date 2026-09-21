//! Local preclinical evidence verification gate for the glioma research engine.
//!
//! The gate is a release boundary for typed evidence, not a confidence veneer. It checks whether
//! a bounded claim has enough independent, fresh, quality-controlled, multimodal/model coverage
//! to move into knowledge and workflow planning. Contradictions, negatives, stale/uncertain
//! states, and every rejected record remain explicit. No claim is inferred from wording and no
//! clinical decision is made.

use crate::glioma::evidence::{EvidenceRecord, EvidenceSourceKind, EvidenceState};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceVerificationGate1@1";
pub const MAX_RECORDS: usize = 16_384;
pub const MAX_TERMS: usize = 128;
pub const MAX_FINDINGS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerificationDisposition {
    Verified,
    Conditional,
    Insufficient,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceVerificationSeverity {
    Info,
    Warning,
    Blocking,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceVerificationOmission {
    pub evidence_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceVerificationFinding {
    pub code: String,
    pub severity: EvidenceVerificationSeverity,
    pub message: String,
    pub evidence_order: Vec<String>,
    pub remediation_route: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceVerificationRequest {
    pub objective: String,
    pub claim_terms: Vec<String>,
    pub scope_terms: Vec<String>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub required_source_kinds: BTreeSet<EvidenceSourceKind>,
    pub minimum_quality_milli: u16,
    pub minimum_reproducibility_milli: u16,
    pub current_epoch: u32,
    pub maximum_age_epochs: u32,
    pub minimum_supporting_records: usize,
    pub minimum_independent_source_kinds: usize,
    pub minimum_modalities: usize,
    pub minimum_model_systems: usize,
    pub allow_unknown: bool,
    pub require_negative_review: bool,
    pub require_contradiction_resolution: bool,
    pub records: Vec<EvidenceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceVerificationReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub query_digest: ContentHash,
    pub record_order: Vec<String>,
    pub eligible_order: Vec<String>,
    pub ineligible_order: Vec<String>,
    pub omissions: Vec<EvidenceVerificationOmission>,
    pub support_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub source_kind_counts: BTreeMap<String, usize>,
    pub modality_counts: BTreeMap<String, usize>,
    pub model_system_counts: BTreeMap<String, usize>,
    pub independent_source_kind_order: Vec<String>,
    pub support_count: usize,
    pub support_quality_milli: u16,
    pub support_reproducibility_milli: u16,
    pub coverage_milli: u16,
    pub modality_coverage_milli: u16,
    pub model_coverage_milli: u16,
    pub source_independence_milli: u16,
    pub contradiction_milli: u16,
    pub freshness_milli: u16,
    pub quality_floor_passed: bool,
    pub reproducibility_floor_passed: bool,
    pub findings: Vec<EvidenceVerificationFinding>,
    pub disposition: EvidenceVerificationDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceVerificationError {
    #[error("evidence verification request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence verification record is invalid: {0}")]
    InvalidRecord(String),
    #[error("evidence verification output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence verification digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn normalized_terms(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| {
            value
                .split(|character: char| !character.is_alphanumeric())
                .filter(|term| !term.is_empty())
                .map(|term| term.to_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn token_set(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect()
}

fn label<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn mean(values: impl Iterator<Item = u16>) -> u16 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| u32::from(*value)).sum::<u32>() / values.len() as u32) as u16
    }
}

fn is_uncertain(state: EvidenceState) -> bool {
    matches!(
        state,
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
    )
}

fn digest_input(report: &EvidenceVerificationReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "query_digest": report.query_digest,
        "record_order": report.record_order,
        "eligible_order": report.eligible_order,
        "ineligible_order": report.ineligible_order,
        "omissions": report.omissions,
        "support_order": report.support_order,
        "negative_order": report.negative_order,
        "contradicted_order": report.contradicted_order,
        "uncertain_order": report.uncertain_order,
        "stale_order": report.stale_order,
        "source_kind_counts": report.source_kind_counts,
        "modality_counts": report.modality_counts,
        "model_system_counts": report.model_system_counts,
        "independent_source_kind_order": report.independent_source_kind_order,
        "support_count": report.support_count,
        "support_quality_milli": report.support_quality_milli,
        "support_reproducibility_milli": report.support_reproducibility_milli,
        "coverage_milli": report.coverage_milli,
        "modality_coverage_milli": report.modality_coverage_milli,
        "model_coverage_milli": report.model_coverage_milli,
        "source_independence_milli": report.source_independence_milli,
        "contradiction_milli": report.contradiction_milli,
        "freshness_milli": report.freshness_milli,
        "quality_floor_passed": report.quality_floor_passed,
        "reproducibility_floor_passed": report.reproducibility_floor_passed,
        "findings": report.findings,
        "disposition": report.disposition,
        "next_route": report.next_route,
    })
}

fn validate_request(
    request: &EvidenceVerificationRequest,
) -> Result<(), EvidenceVerificationError> {
    if request.objective.trim().is_empty()
        || request.current_epoch == 0
        || request.maximum_age_epochs == 0
        || request.minimum_supporting_records == 0
        || request.minimum_independent_source_kinds == 0
        || request.minimum_modalities == 0
        || request.minimum_model_systems == 0
        || request.minimum_quality_milli > 1_000
        || request.minimum_reproducibility_milli > 1_000
        || request.records.len() > MAX_RECORDS
        || request.claim_terms.len() + request.scope_terms.len() > MAX_TERMS
    {
        return Err(EvidenceVerificationError::InvalidRequest(
            "objective, positive coverage thresholds, bounded records/terms, and score bounds are required".into(),
        ));
    }
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    if claim_terms.is_empty()
        && scope_terms.is_empty()
        && request.required_modalities.is_empty()
        && request.required_model_systems.is_empty()
        && request.required_source_kinds.is_empty()
    {
        return Err(EvidenceVerificationError::InvalidRequest(
            "at least one claim/scope term or structured evidence filter is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for record in &request.records {
        if record.evidence_id.trim().is_empty()
            || record.claim.trim().is_empty()
            || record.scope.trim().is_empty()
            || record.release_epoch > request.current_epoch
            || record.relevance_milli > 1_000
            || record.quality_milli > 1_000
            || record.reproducibility_milli > 1_000
            || record.source_artifact.validate().is_err()
            || !ids.insert(record.evidence_id.clone())
        {
            return Err(EvidenceVerificationError::InvalidRecord(
                "records must be unique local de-identified artifacts with nonempty claim/scope, bounded epochs, and bounded scores".into(),
            ));
        }
    }
    Ok(())
}

fn push_finding(
    findings: &mut Vec<EvidenceVerificationFinding>,
    code: &str,
    severity: EvidenceVerificationSeverity,
    message: impl Into<String>,
    evidence_order: Vec<String>,
    remediation_route: &str,
) {
    findings.push(EvidenceVerificationFinding {
        code: code.into(),
        severity,
        message: message.into(),
        evidence_order,
        remediation_route: remediation_route.into(),
    });
}

/// Verify whether a local typed evidence surface is ready to enter knowledge/workflow planning.
pub fn verify_glioma_evidence(
    request: &EvidenceVerificationRequest,
) -> Result<EvidenceVerificationReport, EvidenceVerificationError> {
    validate_request(request)?;
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    let mut record_order = request
        .records
        .iter()
        .map(|record| record.evidence_id.clone())
        .collect::<Vec<_>>();
    record_order.sort();
    let mut eligible_order = Vec::new();
    let mut omissions = Vec::new();
    let mut support_order = Vec::new();
    let mut negative_order = Vec::new();
    let mut contradicted_order = Vec::new();
    let mut uncertain_order = Vec::new();
    let mut stale_order = Vec::new();
    let mut source_kind_counts = BTreeMap::new();
    let mut modality_counts = BTreeMap::new();
    let mut model_system_counts = BTreeMap::new();
    let mut matched_claim_terms = BTreeSet::new();
    let mut matched_scope_terms = BTreeSet::new();
    let mut eligible_records = Vec::new();
    let mut max_quality_passed = true;
    let mut max_reproducibility_passed = true;

    for record in &request.records {
        match record.state {
            EvidenceState::Negative => negative_order.push(record.evidence_id.clone()),
            EvidenceState::Contradicted => contradicted_order.push(record.evidence_id.clone()),
            EvidenceState::Unknown | EvidenceState::Unmeasured => {
                uncertain_order.push(record.evidence_id.clone())
            }
            EvidenceState::Stale => {
                stale_order.push(record.evidence_id.clone());
                uncertain_order.push(record.evidence_id.clone());
            }
            EvidenceState::Supported => {}
        }
        let age_epochs = request.current_epoch.saturating_sub(record.release_epoch);
        if age_epochs > request.maximum_age_epochs {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "maximum-age-filter".into(),
            });
            continue;
        }
        let claim_tokens = token_set(&record.claim);
        let scope_tokens = token_set(&record.scope);
        let matched_claim = claim_terms
            .iter()
            .filter(|term| claim_tokens.contains(*term))
            .count();
        let matched_scope = scope_terms
            .iter()
            .filter(|term| scope_tokens.contains(*term))
            .count();
        if !claim_terms.is_empty() && matched_claim == 0 {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "claim-term-mismatch".into(),
            });
            continue;
        }
        if !scope_terms.is_empty() && matched_scope == 0 {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "scope-term-mismatch".into(),
            });
            continue;
        }
        matched_claim_terms.extend(
            claim_terms
                .iter()
                .filter(|term| claim_tokens.contains(*term))
                .cloned(),
        );
        matched_scope_terms.extend(
            scope_terms
                .iter()
                .filter(|term| scope_tokens.contains(*term))
                .cloned(),
        );
        if record.quality_milli < request.minimum_quality_milli {
            max_quality_passed = false;
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "quality-floor".into(),
            });
            continue;
        }
        if record.reproducibility_milli < request.minimum_reproducibility_milli {
            max_reproducibility_passed = false;
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "reproducibility-floor".into(),
            });
            continue;
        }
        if !request.required_modalities.is_empty()
            && !request.required_modalities.contains(&record.modality)
        {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "modality-filter".into(),
            });
            continue;
        }
        if !request.required_model_systems.is_empty()
            && !record
                .model_system
                .is_some_and(|model| request.required_model_systems.contains(&model))
        {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "model-system-filter".into(),
            });
            continue;
        }
        if !request.required_source_kinds.is_empty()
            && !request.required_source_kinds.contains(&record.source_kind)
        {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "source-kind-filter".into(),
            });
            continue;
        }
        if record.state == EvidenceState::Stale {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "stale-state".into(),
            });
            continue;
        }
        if is_uncertain(record.state) && !request.allow_unknown {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "uncertain-state".into(),
            });
            continue;
        }
        eligible_order.push(record.evidence_id.clone());
        eligible_records.push((record, age_epochs));
        *source_kind_counts
            .entry(label(record.source_kind))
            .or_insert(0_usize) += 1;
        *modality_counts
            .entry(label(record.modality))
            .or_insert(0_usize) += 1;
        if let Some(model) = record.model_system {
            *model_system_counts.entry(label(model)).or_insert(0_usize) += 1;
        }
        if record.state == EvidenceState::Supported {
            support_order.push(record.evidence_id.clone());
        }
    }
    eligible_order.sort();
    support_order.sort();
    negative_order.sort();
    contradicted_order.sort();
    uncertain_order.sort();
    stale_order.sort();
    omissions.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    omissions.dedup_by(|left, right| left.evidence_id == right.evidence_id);
    for record in &request.records {
        if !eligible_order.contains(&record.evidence_id)
            && !omissions
                .iter()
                .any(|omission| omission.evidence_id == record.evidence_id)
        {
            omissions.push(EvidenceVerificationOmission {
                evidence_id: record.evidence_id.clone(),
                reason: "verification-filter".into(),
            });
        }
    }
    omissions.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let ineligible_order = omissions
        .iter()
        .map(|omission| omission.evidence_id.clone())
        .collect::<Vec<_>>();
    let eligible_id_set = eligible_order.iter().cloned().collect::<BTreeSet<_>>();
    let eligible_negative_order = negative_order
        .iter()
        .filter(|id| eligible_id_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let eligible_contradicted_order = contradicted_order
        .iter()
        .filter(|id| eligible_id_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let eligible_uncertain_order = uncertain_order
        .iter()
        .filter(|id| eligible_id_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let support_records = eligible_records
        .iter()
        .filter(|(record, _)| record.state == EvidenceState::Supported)
        .collect::<Vec<_>>();
    let source_kind_order = source_kind_counts.keys().cloned().collect::<Vec<_>>();
    let modality_order = modality_counts.keys().cloned().collect::<Vec<_>>();
    let model_order = model_system_counts.keys().cloned().collect::<Vec<_>>();
    let support_count = support_records.len();
    let support_quality_milli = mean(
        support_records
            .iter()
            .map(|(record, _)| record.quality_milli),
    );
    let support_reproducibility_milli = mean(
        support_records
            .iter()
            .map(|(record, _)| record.reproducibility_milli),
    );
    let modality_coverage_milli = if request.required_modalities.is_empty() {
        ((modality_order.len() as u32 * 1_000) / request.minimum_modalities as u32).min(1_000)
            as u16
    } else {
        let covered = request
            .required_modalities
            .iter()
            .filter(|modality| {
                eligible_records
                    .iter()
                    .any(|(record, _)| record.modality == **modality)
            })
            .count();
        ((covered as u32 * 1_000) / request.required_modalities.len() as u32).min(1_000) as u16
    };
    let model_coverage_milli = if request.required_model_systems.is_empty() {
        ((model_order.len() as u32 * 1_000) / request.minimum_model_systems as u32).min(1_000)
            as u16
    } else {
        let covered = request
            .required_model_systems
            .iter()
            .filter(|model| {
                eligible_records
                    .iter()
                    .any(|(record, _)| record.model_system == Some(**model))
            })
            .count();
        ((covered as u32 * 1_000) / request.required_model_systems.len() as u32).min(1_000) as u16
    };
    let source_independence_milli = ((source_kind_order.len() as u32 * 1_000)
        / request.minimum_independent_source_kinds as u32)
        .min(1_000) as u16;
    let coverage_milli = ((modality_coverage_milli as u32
        + model_coverage_milli as u32
        + source_independence_milli as u32)
        / 3) as u16;
    let contradiction_milli = if eligible_order.is_empty() {
        0
    } else {
        ((eligible_contradicted_order.len() as u32 * 1_000) / eligible_order.len() as u32) as u16
    };
    let freshness_milli = mean(eligible_records.iter().map(|(_, age)| {
        (request
            .maximum_age_epochs
            .saturating_sub(*age)
            .min(request.maximum_age_epochs)
            * 1_000
            / request.maximum_age_epochs) as u16
    }));
    let mut findings = Vec::new();
    if request.records.is_empty() {
        push_finding(
            &mut findings,
            "no-local-evidence",
            EvidenceVerificationSeverity::Blocking,
            "no local evidence records were supplied",
            Vec::new(),
            "glioma_evidence_acquisition_plan",
        );
    }
    if support_count < request.minimum_supporting_records {
        push_finding(
            &mut findings,
            "support-debt",
            EvidenceVerificationSeverity::Blocking,
            format!(
                "{} supporting records are available but {} are required",
                support_count, request.minimum_supporting_records
            ),
            support_order.clone(),
            "glioma_evidence_acquisition_plan",
        );
    }
    if source_kind_order.len() < request.minimum_independent_source_kinds {
        push_finding(
            &mut findings,
            "source-independence-debt",
            EvidenceVerificationSeverity::Blocking,
            format!(
                "{} independent source kinds are available but {} are required",
                source_kind_order.len(),
                request.minimum_independent_source_kinds
            ),
            eligible_order.clone(),
            "glioma_federated_evidence_acquisition_policy",
        );
    }
    if modality_coverage_milli < 1_000 {
        push_finding(
            &mut findings,
            "modality-coverage-debt",
            EvidenceVerificationSeverity::Blocking,
            "required multimodal coverage is incomplete",
            eligible_order.clone(),
            "glioma_multimodal_evidence_gap_router",
        );
    }
    if model_coverage_milli < 1_000 {
        push_finding(
            &mut findings,
            "model-coverage-debt",
            EvidenceVerificationSeverity::Blocking,
            "required preclinical model-system coverage is incomplete",
            eligible_order.clone(),
            "glioma_multimodal_evidence_gap_router",
        );
    }
    if request.require_contradiction_resolution && !eligible_contradicted_order.is_empty() {
        push_finding(
            &mut findings,
            "contradiction-unresolved",
            EvidenceVerificationSeverity::Blocking,
            "contradicted records require explicit resolution before promotion",
            eligible_contradicted_order.clone(),
            "plan_glioma_evidence_contradiction_cut",
        );
    } else if !eligible_contradicted_order.is_empty() {
        push_finding(
            &mut findings,
            "contradiction-present",
            EvidenceVerificationSeverity::Warning,
            "contradicted records remain visible and are not counted as support",
            eligible_contradicted_order.clone(),
            "plan_glioma_evidence_contradiction_cut",
        );
    }
    if request.require_negative_review && eligible_negative_order.is_empty() {
        push_finding(
            &mut findings,
            "negative-review-missing",
            EvidenceVerificationSeverity::Blocking,
            "negative-result review is required but no explicit negative record is present",
            Vec::new(),
            "glioma_evidence_acquisition_plan",
        );
    } else if !eligible_negative_order.is_empty() {
        push_finding(
            &mut findings,
            "negative-evidence-present",
            EvidenceVerificationSeverity::Info,
            "negative results remain part of the verification surface",
            eligible_negative_order.clone(),
            "glioma_evidence_frontier_join",
        );
    }
    if !eligible_uncertain_order.is_empty() {
        push_finding(
            &mut findings,
            "uncertain-evidence-present",
            EvidenceVerificationSeverity::Warning,
            "unknown, stale, or unmeasured records remain outside verified support",
            eligible_uncertain_order.clone(),
            "glioma_evidence_prospective_triage",
        );
    }
    if !max_quality_passed {
        push_finding(
            &mut findings,
            "quality-floor-debt",
            EvidenceVerificationSeverity::Warning,
            "one or more claim-matched records failed the quality floor",
            ineligible_order.clone(),
            "glioma_evidence_researcher_workbench",
        );
    }
    if !max_reproducibility_passed {
        push_finding(
            &mut findings,
            "reproducibility-floor-debt",
            EvidenceVerificationSeverity::Warning,
            "one or more claim-matched records failed the reproducibility floor",
            ineligible_order.clone(),
            "glioma_evidence_researcher_workbench",
        );
    }
    findings.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.code.cmp(&right.code))
    });
    if findings.len() > MAX_FINDINGS {
        findings.truncate(MAX_FINDINGS);
    }
    let blocking = findings
        .iter()
        .any(|finding| finding.severity == EvidenceVerificationSeverity::Blocking);
    let warnings = findings
        .iter()
        .any(|finding| finding.severity == EvidenceVerificationSeverity::Warning);
    let disposition = if blocking {
        if support_count == 0 {
            EvidenceVerificationDisposition::Blocked
        } else {
            EvidenceVerificationDisposition::Insufficient
        }
    } else if warnings {
        EvidenceVerificationDisposition::Conditional
    } else {
        EvidenceVerificationDisposition::Verified
    };
    let next_route = match disposition {
        EvidenceVerificationDisposition::Verified
        | EvidenceVerificationDisposition::Conditional => "glioma_knowledge_protocol_gateway",
        EvidenceVerificationDisposition::Insufficient => "glioma_multimodal_evidence_gap_router",
        EvidenceVerificationDisposition::Blocked => "glioma_evidence_acquisition_plan",
    };
    let query_digest = ContentHash::of_value(&serde_json::json!({
        "objective": request.objective,
        "claim_terms": claim_terms,
        "scope_terms": scope_terms,
        "required_modalities": request.required_modalities,
        "required_model_systems": request.required_model_systems,
        "required_source_kinds": request.required_source_kinds,
        "minimum_quality_milli": request.minimum_quality_milli,
        "minimum_reproducibility_milli": request.minimum_reproducibility_milli,
        "current_epoch": request.current_epoch,
        "maximum_age_epochs": request.maximum_age_epochs,
        "minimum_supporting_records": request.minimum_supporting_records,
        "minimum_independent_source_kinds": request.minimum_independent_source_kinds,
        "minimum_modalities": request.minimum_modalities,
        "minimum_model_systems": request.minimum_model_systems,
        "allow_unknown": request.allow_unknown,
        "require_negative_review": request.require_negative_review,
        "require_contradiction_resolution": request.require_contradiction_resolution,
        "record_order": record_order,
    }))
    .map_err(|error| EvidenceVerificationError::Digest(error.to_string()))?;
    let mut report = EvidenceVerificationReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        query_digest,
        record_order,
        eligible_order,
        ineligible_order,
        omissions,
        support_order,
        negative_order,
        contradicted_order,
        uncertain_order,
        stale_order,
        source_kind_counts,
        modality_counts,
        model_system_counts,
        independent_source_kind_order: source_kind_order,
        support_count,
        support_quality_milli,
        support_reproducibility_milli,
        coverage_milli,
        modality_coverage_milli,
        model_coverage_milli,
        source_independence_milli,
        contradiction_milli,
        freshness_milli,
        quality_floor_passed: max_quality_passed,
        reproducibility_floor_passed: max_reproducibility_passed,
        findings,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-evidence-verification"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| EvidenceVerificationError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

impl EvidenceVerificationReport {
    pub fn validate(&self) -> Result<(), EvidenceVerificationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.query_digest.as_str().len() != 64
            || !canonical(&self.record_order)
            || !unique_nonempty(&self.record_order)
            || !canonical(&self.eligible_order)
            || !unique_nonempty(&self.eligible_order)
            || !canonical(&self.ineligible_order)
            || !unique_nonempty(&self.ineligible_order)
            || !canonical(&self.support_order)
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.uncertain_order)
            || !canonical(&self.stale_order)
            || !canonical(&self.independent_source_kind_order)
            || !canonical(&self.source_kind_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.modality_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.model_system_counts.keys().cloned().collect::<Vec<_>>())
            || self.coverage_milli > 1_000
            || self.modality_coverage_milli > 1_000
            || self.model_coverage_milli > 1_000
            || self.source_independence_milli > 1_000
            || self.contradiction_milli > 1_000
            || self.freshness_milli > 1_000
            || self.support_quality_milli > 1_000
            || self.support_reproducibility_milli > 1_000
            || self.support_count != self.support_order.len()
            || self.findings.len() > MAX_FINDINGS
            || self.next_route.trim().is_empty()
        {
            return Err(EvidenceVerificationError::InvalidOutput(
                "identity, ordering, score bounds, support count, or finding shape is invalid"
                    .into(),
            ));
        }
        let record_ids = self.record_order.iter().cloned().collect::<BTreeSet<_>>();
        let eligible_ids = self.eligible_order.iter().cloned().collect::<BTreeSet<_>>();
        let ineligible_ids = self
            .ineligible_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partition = eligible_ids
            .union(&ineligible_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let omission_ids = self
            .omissions
            .iter()
            .map(|omission| omission.evidence_id.clone())
            .collect::<BTreeSet<_>>();
        if partition != record_ids
            || eligible_ids.len() + ineligible_ids.len() != partition.len()
            || omission_ids != ineligible_ids
            || self.omissions.len() != ineligible_ids.len()
            || self
                .omissions
                .iter()
                .map(|omission| omission.evidence_id.clone())
                .collect::<Vec<_>>()
                != self.ineligible_order
            || self.omissions.iter().any(|omission| {
                omission.evidence_id.trim().is_empty() || omission.reason.trim().is_empty()
            })
            || self.findings.iter().any(|finding| {
                finding.code.trim().is_empty()
                    || finding.message.trim().is_empty()
                    || finding.remediation_route.trim().is_empty()
                    || !canonical(&finding.evidence_order)
                    || !unique_nonempty(&finding.evidence_order)
                    || finding
                        .evidence_order
                        .iter()
                        .any(|id| !record_ids.contains(id))
            })
            || self.findings.windows(2).any(|pair| {
                pair[0].severity < pair[1].severity
                    || (pair[0].severity == pair[1].severity && pair[0].code > pair[1].code)
            })
            || self.source_kind_counts.values().any(|count| *count == 0)
            || self.modality_counts.values().any(|count| *count == 0)
            || self.model_system_counts.values().any(|count| *count == 0)
        {
            return Err(EvidenceVerificationError::InvalidOutput(
                "record/omission partition, finding ordering, finding references, or facet counts do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceVerificationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceVerificationError::Digest(
                "evidence verification digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> crate::glioma_engine::LocalArtifactRef {
        crate::glioma_engine::LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-evidence+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn record(
        id: &str,
        source_kind: EvidenceSourceKind,
        modality: GliomaModality,
        model_system: GliomaModelSystem,
        state: EvidenceState,
        claim: &str,
    ) -> EvidenceRecord {
        EvidenceRecord {
            evidence_id: id.into(),
            source_artifact: artifact(&format!("artifact-{id}")),
            source_kind,
            claim: claim.into(),
            scope: "glioma invasion organoid".into(),
            modality,
            model_system: Some(model_system),
            state,
            relevance_milli: 900,
            quality_milli: 900,
            reproducibility_milli: 850,
            release_epoch: 5,
        }
    }

    fn request() -> EvidenceVerificationRequest {
        EvidenceVerificationRequest {
            objective: "verify EGFR invasion evidence before knowledge compilation".into(),
            claim_terms: vec!["invasion".into()],
            scope_terms: vec!["organoid".into()],
            required_modalities: BTreeSet::new(),
            required_model_systems: BTreeSet::new(),
            required_source_kinds: BTreeSet::new(),
            minimum_quality_milli: 500,
            minimum_reproducibility_milli: 500,
            current_epoch: 5,
            maximum_age_epochs: 10,
            minimum_supporting_records: 2,
            minimum_independent_source_kinds: 2,
            minimum_modalities: 2,
            minimum_model_systems: 2,
            allow_unknown: false,
            require_negative_review: false,
            require_contradiction_resolution: true,
            records: vec![
                record(
                    "evidence-a",
                    EvidenceSourceKind::Literature,
                    GliomaModality::Imaging,
                    GliomaModelSystem::Organoid,
                    EvidenceState::Supported,
                    "EGFR drives invasion",
                ),
                record(
                    "evidence-b",
                    EvidenceSourceKind::Assay,
                    GliomaModality::Transcriptomics,
                    GliomaModelSystem::MouseModel,
                    EvidenceState::Supported,
                    "EGFR drives invasion",
                ),
            ],
        }
    }

    #[test]
    fn verifies_independent_multimodal_preclinical_support() {
        let report = verify_glioma_evidence(&request()).unwrap();
        assert_eq!(
            report.disposition,
            EvidenceVerificationDisposition::Verified
        );
        assert_eq!(report.support_count, 2);
        assert_eq!(report.source_independence_milli, 1_000);
        assert_eq!(report.modality_coverage_milli, 1_000);
        assert_eq!(report.model_coverage_milli, 1_000);
        assert_eq!(report.next_route, "glioma_knowledge_protocol_gateway");
        report.validate().unwrap();
    }

    #[test]
    fn contradiction_blocks_promotion_and_routes_resolution() {
        let mut request = request();
        request.records.push(record(
            "evidence-c",
            EvidenceSourceKind::Replication,
            GliomaModality::Proteomics,
            GliomaModelSystem::MouseModel,
            EvidenceState::Contradicted,
            "EGFR fails invasion",
        ));
        let report = verify_glioma_evidence(&request).unwrap();
        assert_eq!(
            report.disposition,
            EvidenceVerificationDisposition::Insufficient
        );
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "contradiction-unresolved"));
        assert_eq!(report.next_route, "glioma_multimodal_evidence_gap_router");
        report.validate().unwrap();
    }

    #[test]
    fn unrelated_contradiction_is_visible_but_does_not_block_target_claim() {
        let mut request = request();
        request.records.push(record(
            "evidence-unrelated",
            EvidenceSourceKind::Replication,
            GliomaModality::Proteomics,
            GliomaModelSystem::MouseModel,
            EvidenceState::Contradicted,
            "unrelated metabolism claim",
        ));
        let report = verify_glioma_evidence(&request).unwrap();
        assert_eq!(
            report.disposition,
            EvidenceVerificationDisposition::Verified
        );
        assert!(report
            .contradicted_order
            .contains(&"evidence-unrelated".to_string()));
        assert!(!report
            .findings
            .iter()
            .any(|finding| finding.code == "contradiction-unresolved"));
        report.validate().unwrap();
    }

    #[test]
    fn missing_coverage_is_explicit_not_silently_passed() {
        let mut request = request();
        request.minimum_modalities = 3;
        let report = verify_glioma_evidence(&request).unwrap();
        assert_eq!(
            report.disposition,
            EvidenceVerificationDisposition::Insufficient
        );
        assert!(report
            .findings
            .iter()
            .any(|finding| finding.code == "modality-coverage-debt"));
        report.validate().unwrap();
    }

    #[test]
    fn empty_local_surface_is_blocked() {
        let mut request = request();
        request.records.clear();
        let report = verify_glioma_evidence(&request).unwrap();
        assert_eq!(report.disposition, EvidenceVerificationDisposition::Blocked);
        assert_eq!(report.next_route, "glioma_evidence_acquisition_plan");
        report.validate().unwrap();
    }

    #[test]
    fn verification_replays_to_identical_digest() {
        let first = verify_glioma_evidence(&request()).unwrap();
        let second = verify_glioma_evidence(&request()).unwrap();
        assert_eq!(first.digest, second.digest);
        assert_eq!(first.query_digest, second.query_digest);
    }
}
