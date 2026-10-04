//! Canonical multi-study panel of registered-outcome evidence records.
//!
//! The detailed P10-F06 source blueprint is not configured in this checkout. This
//! repository-defined typed data primitive binds compatible local per-study outcome records into
//! one replayable panel. It preserves availability and provenance without pooling estimates,
//! inferring direction, or dispatching research actions.

use super::outcome_record::{OutcomeAvailability, RegisteredOutcomeRecord};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = super::replication_assay_mapping_ledger::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaRegisteredOutcomeEvidencePanel1@1";
pub const MAX_STUDIES: usize = 512;
pub const MAX_TEXT_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcomeEvidencePanelRequest {
    pub objective: String,
    pub outcome_id: String,
    pub estimand_id: String,
    pub effect_unit: String,
    pub registry_report_order: Vec<ContentHash>,
    pub result_report_order: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeAvailabilityCount {
    pub availability: OutcomeAvailability,
    pub study_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisteredOutcomeEvidencePanel {
    pub feature_id: String,
    pub output_schema: String,
    pub request: RegisteredOutcomeEvidencePanelRequest,
    pub records: Vec<RegisteredOutcomeRecord>,
    pub input_digest: ContentHash,
    pub study_order: Vec<String>,
    pub estimate_order: Vec<String>,
    pub null_order: Vec<String>,
    pub unavailable_order: Vec<String>,
    pub not_due_order: Vec<String>,
    pub availability_counts: Vec<OutcomeAvailabilityCount>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OutcomeEvidencePanelError {
    #[error("registered-outcome evidence-panel request is invalid: {0}")]
    InvalidRequest(String),
    #[error("registered-outcome evidence-panel studies are invalid: {0}")]
    InvalidStudy(String),
    #[error("registered-outcome evidence-panel output is invalid: {0}")]
    InvalidOutput(String),
    #[error("registered-outcome evidence-panel digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn validate_request(
    request: &RegisteredOutcomeEvidencePanelRequest,
) -> Result<(), OutcomeEvidencePanelError> {
    if request.objective.trim().is_empty()
        || request.objective.len() > 4_096
        || request.outcome_id.trim().is_empty()
        || request.outcome_id.len() > MAX_TEXT_BYTES
        || request.estimand_id.trim().is_empty()
        || request.estimand_id.len() > MAX_TEXT_BYTES
        || request.effect_unit.trim().is_empty()
        || request.effect_unit.len() > MAX_TEXT_BYTES
        || !(2..=MAX_STUDIES).contains(&request.registry_report_order.len())
        || !canonical(&request.registry_report_order)
        || request
            .registry_report_order
            .iter()
            .any(|hash| !valid_hash(hash))
        || request.result_report_order.len() > MAX_STUDIES
        || !canonical(&request.result_report_order)
        || request
            .result_report_order
            .iter()
            .any(|hash| !valid_hash(hash))
    {
        return Err(OutcomeEvidencePanelError::InvalidRequest(
            "objective, exact outcome/estimand/unit, and canonical bounded report digests for at least two studies are required".into(),
        ));
    }
    Ok(())
}

fn validate_studies(
    request: &RegisteredOutcomeEvidencePanelRequest,
    records: &[RegisteredOutcomeRecord],
) -> Result<(), OutcomeEvidencePanelError> {
    if records.len() < 2 || records.len() > MAX_STUDIES {
        return Err(OutcomeEvidencePanelError::InvalidStudy(
            "the panel must contain between two and 512 independent study records".into(),
        ));
    }

    let expected_registry = request
        .registry_report_order
        .iter()
        .collect::<BTreeSet<_>>();
    let expected_results = request.result_report_order.iter().collect::<BTreeSet<_>>();
    let mut seen_registry = BTreeSet::new();
    let mut seen_results = BTreeSet::new();
    let mut study_ids = BTreeSet::new();
    let mut independence_groups = BTreeSet::new();

    for record in records {
        record.validate().map_err(|error| {
            OutcomeEvidencePanelError::InvalidStudy(format!(
                "study record did not validate: {error}"
            ))
        })?;
        let study = &record.study;
        let registry_valid = expected_registry.contains(&&study.registry_report_digest)
            && seen_registry.insert(&study.registry_report_digest);
        let result_valid = match &study.result_report_digest {
            Some(report_digest) => {
                expected_results.contains(&report_digest) && seen_results.insert(report_digest)
            }
            None => true,
        };
        if study.outcome_id != request.outcome_id
            || study.estimand_id != request.estimand_id
            || study.effect_unit != request.effect_unit
            || !study_ids.insert(study.study_id.as_str())
            || !independence_groups.insert(study.independence_group.as_str())
            || !registry_valid
            || !result_valid
        {
            return Err(OutcomeEvidencePanelError::InvalidStudy(
                "each record must match the panel outcome/estimand/unit and bind a unique independent group and exact source report".into(),
            ));
        }
    }

    if seen_registry != expected_registry || seen_results != expected_results {
        return Err(OutcomeEvidencePanelError::InvalidStudy(
            "registry and result report digest sets must exactly cover the supplied study records"
                .into(),
        ));
    }
    Ok(())
}

fn input_digest(
    request: &RegisteredOutcomeEvidencePanelRequest,
    records: &[RegisteredOutcomeRecord],
) -> Result<ContentHash, OutcomeEvidencePanelError> {
    #[derive(Serialize)]
    struct Input<'a> {
        schema: &'static str,
        request: &'a RegisteredOutcomeEvidencePanelRequest,
        records: Vec<&'a RegisteredOutcomeRecord>,
    }
    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.study.study_id.cmp(&right.study.study_id));
    ContentHash::of_serializable(&Input {
        schema: "GliomaRegisteredOutcomeEvidencePanelInput1@1",
        request,
        records: ordered,
    })
    .map_err(|error| OutcomeEvidencePanelError::Digest(error.to_string()))
}

#[derive(Serialize)]
struct OutputDigest<'a> {
    feature_id: &'a str,
    output_schema: &'a str,
    request: &'a RegisteredOutcomeEvidencePanelRequest,
    input_digest: &'a ContentHash,
    study_order: &'a [String],
    estimate_order: &'a [String],
    null_order: &'a [String],
    unavailable_order: &'a [String],
    not_due_order: &'a [String],
    availability_counts: &'a [OutcomeAvailabilityCount],
}

fn output_digest(
    panel: &RegisteredOutcomeEvidencePanel,
) -> Result<ContentHash, OutcomeEvidencePanelError> {
    ContentHash::of_serializable(&OutputDigest {
        feature_id: &panel.feature_id,
        output_schema: &panel.output_schema,
        request: &panel.request,
        input_digest: &panel.input_digest,
        study_order: &panel.study_order,
        estimate_order: &panel.estimate_order,
        null_order: &panel.null_order,
        unavailable_order: &panel.unavailable_order,
        not_due_order: &panel.not_due_order,
        availability_counts: &panel.availability_counts,
    })
    .map_err(|error| OutcomeEvidencePanelError::Digest(error.to_string()))
}

fn compile(
    request: &RegisteredOutcomeEvidencePanelRequest,
    records: &[RegisteredOutcomeRecord],
) -> Result<RegisteredOutcomeEvidencePanel, OutcomeEvidencePanelError> {
    let mut ordered = records.to_vec();
    ordered.sort_by(|left, right| left.study.study_id.cmp(&right.study.study_id));
    let study_order = ordered
        .iter()
        .map(|record| record.study.study_id.clone())
        .collect::<Vec<_>>();
    let estimate_order = ordered
        .iter()
        .filter(|record| record.study.availability == OutcomeAvailability::Estimate)
        .map(|record| record.study.study_id.clone())
        .collect();
    let null_order = ordered
        .iter()
        .filter(|record| record.study.availability == OutcomeAvailability::Null)
        .map(|record| record.study.study_id.clone())
        .collect();
    let unavailable_order = ordered
        .iter()
        .filter(|record| {
            matches!(
                record.study.availability,
                OutcomeAvailability::Missing
                    | OutcomeAvailability::Incomplete
                    | OutcomeAvailability::Ambiguous
                    | OutcomeAvailability::Unresolved
            )
        })
        .map(|record| record.study.study_id.clone())
        .collect();
    let not_due_order = ordered
        .iter()
        .filter(|record| record.study.availability == OutcomeAvailability::NotDue)
        .map(|record| record.study.study_id.clone())
        .collect();
    let availability_counts = [
        OutcomeAvailability::Estimate,
        OutcomeAvailability::Null,
        OutcomeAvailability::Missing,
        OutcomeAvailability::Incomplete,
        OutcomeAvailability::Ambiguous,
        OutcomeAvailability::NotDue,
        OutcomeAvailability::Unresolved,
    ]
    .into_iter()
    .map(|availability| OutcomeAvailabilityCount {
        availability,
        study_count: ordered
            .iter()
            .filter(|record| record.study.availability == availability)
            .count(),
    })
    .collect();

    let mut panel = RegisteredOutcomeEvidencePanel {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        records: ordered,
        input_digest: input_digest(request, records)?,
        study_order,
        estimate_order,
        null_order,
        unavailable_order,
        not_due_order,
        availability_counts,
        digest: ContentHash::of_bytes(b"unsealed-glioma-registered-outcome-evidence-panel"),
    };
    panel.digest = output_digest(&panel)?;
    Ok(panel)
}

impl RegisteredOutcomeEvidencePanel {
    pub fn validate(&self) -> Result<(), OutcomeEvidencePanelError> {
        self.validate_against(&self.request, &self.records)
    }

    pub fn validate_against(
        &self,
        request: &RegisteredOutcomeEvidencePanelRequest,
        records: &[RegisteredOutcomeRecord],
    ) -> Result<(), OutcomeEvidencePanelError> {
        validate_request(request)?;
        validate_studies(request, records)?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || &self.request != request
            || self.input_digest != input_digest(request, records)?
        {
            return Err(OutcomeEvidencePanelError::InvalidOutput(
                "feature identity, schema, or input binding is invalid".into(),
            ));
        }
        let expected = compile(request, records)?;
        if self != &expected {
            return Err(OutcomeEvidencePanelError::InvalidOutput(
                "panel rows, orders, availability counts, or canonical digest do not replay from the bound study records".into(),
            ));
        }
        Ok(())
    }
}

/// Build a typed, canonical panel from at least two independent registered-outcome records.
/// This is data binding and completeness projection only; it does not estimate a shared effect.
pub fn build_glioma_registered_outcome_evidence_panel(
    request: &RegisteredOutcomeEvidencePanelRequest,
    records: &[RegisteredOutcomeRecord],
) -> Result<RegisteredOutcomeEvidencePanel, OutcomeEvidencePanelError> {
    validate_request(request)?;
    validate_studies(request, records)?;
    let panel = compile(request, records)?;
    panel.validate()?;
    Ok(panel)
}

#[cfg(test)]
mod tests {
    use super::super::outcome_record::OutcomeSensitivityStudy;
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: label.into(),
            content_hash: hash(label),
            content_type: "application/vnd.aurora.local-study-outcome+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn study(
        id: &str,
        availability: OutcomeAvailability,
        effect: Option<i64>,
    ) -> OutcomeSensitivityStudy {
        let reported = matches!(
            availability,
            OutcomeAvailability::Estimate
                | OutcomeAvailability::Null
                | OutcomeAvailability::Incomplete
                | OutcomeAvailability::Ambiguous
        );
        OutcomeSensitivityStudy {
            study_id: id.into(),
            independence_group: format!("group-{id}"),
            registry_report_digest: hash(&format!("registry-{id}")),
            registry_artifact: artifact(&format!("registry-artifact-{id}")),
            result_report_digest: reported.then(|| hash(&format!("result-{id}"))),
            result_artifact: reported.then(|| artifact(&format!("result-artifact-{id}"))),
            outcome_id: "primary-viability".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            availability,
            effect_milli: effect,
            uncertainty_milli: effect.map(|_| 20),
            quality_milli: effect.map(|_| 950),
        }
    }

    fn record(
        id: &str,
        availability: OutcomeAvailability,
        effect: Option<i64>,
    ) -> RegisteredOutcomeRecord {
        super::super::outcome_record::build_glioma_registered_outcome_record(&study(
            id,
            availability,
            effect,
        ))
        .unwrap()
    }

    fn panel_request(records: &[RegisteredOutcomeRecord]) -> RegisteredOutcomeEvidencePanelRequest {
        let studies = records
            .iter()
            .map(|record| &record.study)
            .collect::<Vec<_>>();
        let mut registry_report_order = studies
            .iter()
            .map(|study| study.registry_report_digest.clone())
            .collect::<Vec<_>>();
        registry_report_order.sort();
        let mut result_report_order = studies
            .iter()
            .filter_map(|study| study.result_report_digest.clone())
            .collect::<Vec<_>>();
        result_report_order.sort();
        RegisteredOutcomeEvidencePanelRequest {
            objective: "bind aligned study outcome records without pooling".into(),
            outcome_id: "primary-viability".into(),
            estimand_id: "viability-change-at-24h".into(),
            effect_unit: "normalized-signal-milli".into(),
            registry_report_order,
            result_report_order,
        }
    }

    #[test]
    fn panel_is_canonical_digest_bound_and_preserves_each_availability_state() {
        let records = vec![
            record("a", OutcomeAvailability::Estimate, Some(200)),
            record("b", OutcomeAvailability::Null, Some(0)),
            record("c", OutcomeAvailability::Missing, None),
            record("d", OutcomeAvailability::NotDue, None),
        ];
        let request = panel_request(&records);
        let panel = build_glioma_registered_outcome_evidence_panel(&request, &records).unwrap();
        assert_eq!(panel.feature_id, FEATURE_ID);
        assert_eq!(panel.study_order, vec!["a", "b", "c", "d"]);
        assert_eq!(panel.estimate_order, vec!["a"]);
        assert_eq!(panel.null_order, vec!["b"]);
        assert_eq!(panel.unavailable_order, vec!["c"]);
        assert_eq!(panel.not_due_order, vec!["d"]);
        assert_eq!(
            panel.availability_counts[0],
            OutcomeAvailabilityCount {
                availability: OutcomeAvailability::Estimate,
                study_count: 1
            }
        );
        panel.validate_against(&request, &records).unwrap();
    }

    #[test]
    fn input_permutation_does_not_change_the_panel() {
        let first = vec![
            record("a", OutcomeAvailability::Estimate, Some(200)),
            record("b", OutcomeAvailability::Incomplete, None),
        ];
        let second = first.iter().cloned().rev().collect::<Vec<_>>();
        let request = panel_request(&first);
        let left = build_glioma_registered_outcome_evidence_panel(&request, &first).unwrap();
        let right = build_glioma_registered_outcome_evidence_panel(&request, &second).unwrap();
        assert_eq!(left, right);
    }

    #[test]
    fn mismatched_estimands_and_duplicate_independence_groups_are_rejected() {
        let mut records = vec![
            record("a", OutcomeAvailability::Estimate, Some(200)),
            record("b", OutcomeAvailability::Estimate, Some(250)),
        ];
        let request = panel_request(&records);
        records[1].study.independence_group = records[0].study.independence_group.clone();
        assert!(matches!(
            build_glioma_registered_outcome_evidence_panel(&request, &records),
            Err(OutcomeEvidencePanelError::InvalidStudy(_))
        ));
        records[1] = record("b", OutcomeAvailability::Estimate, Some(250));
        records[1].study.estimand_id = "different-estimand".into();
        assert!(matches!(
            build_glioma_registered_outcome_evidence_panel(&request, &records),
            Err(OutcomeEvidencePanelError::InvalidStudy(_))
        ));
    }

    #[test]
    fn report_digest_coverage_and_local_artifact_boundaries_fail_closed() {
        let mut records = vec![
            record("a", OutcomeAvailability::Estimate, Some(200)),
            record("b", OutcomeAvailability::Estimate, Some(250)),
        ];
        let mut request = panel_request(&records);
        request
            .registry_report_order
            .push(hash("unbound-registry-report"));
        request.registry_report_order.sort();
        assert!(matches!(
            build_glioma_registered_outcome_evidence_panel(&request, &records),
            Err(OutcomeEvidencePanelError::InvalidStudy(_))
        ));
        request = panel_request(&records);
        records[1]
            .study
            .registry_artifact
            .contains_direct_identifiers = true;
        assert!(matches!(
            build_glioma_registered_outcome_evidence_panel(&request, &records),
            Err(OutcomeEvidencePanelError::InvalidStudy(_))
        ));
    }

    #[test]
    fn tampered_counts_and_digest_are_rejected() {
        let records = vec![
            record("a", OutcomeAvailability::Estimate, Some(200)),
            record("b", OutcomeAvailability::Ambiguous, None),
        ];
        let request = panel_request(&records);
        let mut panel = build_glioma_registered_outcome_evidence_panel(&request, &records).unwrap();
        panel.availability_counts[0].study_count = 50;
        panel.digest = output_digest(&panel).unwrap();
        assert!(matches!(
            panel.validate(),
            Err(OutcomeEvidencePanelError::InvalidOutput(_))
        ));
    }

    #[test]
    fn panel_schema_and_feature_id_are_explicit_and_versioned() {
        assert_eq!(FEATURE_ID, "GAF-GLIOMA-P10-F06");
        assert_eq!(OUTPUT_SCHEMA, "GliomaRegisteredOutcomeEvidencePanel1@1");
    }
}
