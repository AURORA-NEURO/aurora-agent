//! A typed, independent-site replication contract for preclinical glioma work.
//!
//! This is deliberately a protocol schema rather than a dispatch primitive.  It makes the
//! estimand, assay scope, randomization, masking, stopping rule, deviations, and unknowns
//! explicit before a local laboratory can execute anything.  Source-site values are copied only
//! as provenance; independent-site fields must be supplied by the caller and are never inherited.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationProtocol1@1";
pub const MAX_SITES: usize = 128;
pub const MAX_VARIABLES: usize = 512;
pub const MAX_TEXT: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationEstimand {
    pub outcome_id: String,
    pub population: String,
    pub contrast: String,
    pub timepoint: String,
    pub effect_measure: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RandomizationUnit {
    Well,
    Organoid,
    Animal,
    CultureBatch,
    SimulationRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RandomizationSpec {
    pub unit: RandomizationUnit,
    pub arm_order: Vec<String>,
    pub block_size: u32,
    pub seed: ContentHash,
    pub allocation_ratio_milli: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskingSpec {
    pub operator_masked: bool,
    pub analyst_masked: bool,
    pub evaluator_masked: bool,
    pub unavailable_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoppingRule {
    pub minimum_units: u32,
    pub maximum_units: u32,
    pub interim_looks: u32,
    pub positive_boundary_milli: u16,
    pub negative_boundary_milli: u16,
    pub stop_for_harm: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrespecifiedAnalysis {
    pub outcome_order: Vec<String>,
    pub primary_contrast: String,
    pub model_family: String,
    pub confidence_level_milli: u16,
    pub missingness_rule: String,
    pub multiplicity_rule: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndependentSiteSpec {
    pub site_id: String,
    pub local_assay_capability_ids: Vec<String>,
    pub local_protocol_digest: ContentHash,
    pub authorization_reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolDeviation {
    pub field: String,
    pub source_value: String,
    pub replication_value: String,
    pub rationale: String,
    pub preapproved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolUnknown {
    pub field: String,
    pub consequence: String,
    pub blocking: bool,
    pub resolution_owner: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationProtocolRequest {
    pub source_research_object: LocalArtifactRef,
    pub source_site_id: String,
    pub source_protocol_digest: ContentHash,
    pub objective: String,
    pub replication_question: String,
    pub estimand: ReplicationEstimand,
    pub model_system: GliomaModelSystem,
    pub source_assay_id: String,
    pub replication_assay_id: String,
    pub independent_sites: Vec<IndependentSiteSpec>,
    pub randomization: RandomizationSpec,
    pub masking: MaskingSpec,
    pub stopping_rule: StoppingRule,
    pub prespecified_analysis: PrespecifiedAnalysis,
    pub declared_deviations: Vec<ProtocolDeviation>,
    pub explicit_unknowns: Vec<ProtocolUnknown>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicationProtocolDisposition {
    Ready,
    BlockedByUnknown,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationProtocol {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub source_research_object: LocalArtifactRef,
    pub source_site_id: String,
    pub source_protocol_digest: ContentHash,
    pub objective: String,
    pub replication_question: String,
    pub estimand: ReplicationEstimand,
    pub model_system: GliomaModelSystem,
    pub source_assay_id: String,
    pub replication_assay_id: String,
    pub independent_site_order: Vec<String>,
    pub independent_sites: Vec<IndependentSiteSpec>,
    pub randomization: RandomizationSpec,
    pub masking: MaskingSpec,
    pub stopping_rule: StoppingRule,
    pub prespecified_analysis: PrespecifiedAnalysis,
    pub declared_deviations: Vec<ProtocolDeviation>,
    pub explicit_unknowns: Vec<ProtocolUnknown>,
    pub disposition: ReplicationProtocolDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplicationProtocolError {
    #[error("replication protocol request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replication protocol output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication protocol digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn required_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.len() <= MAX_TEXT
}

fn digest_input(protocol: &ReplicationProtocol) -> serde_json::Value {
    serde_json::json!({
        "feature_id": protocol.feature_id,
        "output_schema": protocol.output_schema,
        "boundary": protocol.boundary,
        "source_research_object": protocol.source_research_object,
        "source_site_id": protocol.source_site_id,
        "source_protocol_digest": protocol.source_protocol_digest,
        "objective": protocol.objective,
        "replication_question": protocol.replication_question,
        "estimand": protocol.estimand,
        "model_system": protocol.model_system,
        "source_assay_id": protocol.source_assay_id,
        "replication_assay_id": protocol.replication_assay_id,
        "independent_site_order": protocol.independent_site_order,
        "independent_sites": protocol.independent_sites,
        "randomization": protocol.randomization,
        "masking": protocol.masking,
        "stopping_rule": protocol.stopping_rule,
        "prespecified_analysis": protocol.prespecified_analysis,
        "declared_deviations": protocol.declared_deviations,
        "explicit_unknowns": protocol.explicit_unknowns,
        "disposition": protocol.disposition,
        "next_action": protocol.next_action,
    })
}

fn validate_request(request: &ReplicationProtocolRequest) -> Result<(), ReplicationProtocolError> {
    request
        .source_research_object
        .validate()
        .map_err(|error| ReplicationProtocolError::InvalidRequest(error.to_string()))?;
    let text_fields = [
        request.source_site_id.as_str(),
        request.objective.as_str(),
        request.replication_question.as_str(),
        request.estimand.outcome_id.as_str(),
        request.estimand.population.as_str(),
        request.estimand.contrast.as_str(),
        request.estimand.timepoint.as_str(),
        request.estimand.effect_measure.as_str(),
        request.source_assay_id.as_str(),
        request.replication_assay_id.as_str(),
        request.prespecified_analysis.primary_contrast.as_str(),
        request.prespecified_analysis.model_family.as_str(),
        request.prespecified_analysis.missingness_rule.as_str(),
        request.prespecified_analysis.multiplicity_rule.as_str(),
    ];
    if text_fields.iter().any(|value| !required_text(value))
        || request.independent_sites.is_empty()
        || request.independent_sites.len() > MAX_SITES
        || request.randomization.arm_order.len() < 2
        || request
            .randomization
            .arm_order
            .iter()
            .any(|value| !required_text(value))
        || request.randomization.block_size == 0
        || request.randomization.seed.as_str().len() != 64
        || request.source_protocol_digest.as_str().len() != 64
        || request.randomization.allocation_ratio_milli.len()
            != request.randomization.arm_order.len()
        || request.randomization.allocation_ratio_milli.contains(&0)
        || request.stopping_rule.minimum_units == 0
        || request.stopping_rule.maximum_units < request.stopping_rule.minimum_units
        || request.stopping_rule.interim_looks == 0
        || request.stopping_rule.positive_boundary_milli > 1_000
        || request.stopping_rule.negative_boundary_milli > 1_000
        || request.prespecified_analysis.outcome_order.is_empty()
        || request
            .prespecified_analysis
            .outcome_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.prespecified_analysis.confidence_level_milli < 500
        || request.prespecified_analysis.confidence_level_milli >= 1_000
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ReplicationProtocolError::InvalidRequest(
            "estimand, assay scope, independent sites, randomization, masking, stopping, analysis, and replay bounds are required".into(),
        ));
    }
    if !request.masking.operator_masked
        && !request.masking.analyst_masked
        && !request.masking.evaluator_masked
        && request
            .masking
            .unavailable_reason
            .as_ref()
            .is_none_or(|reason| !required_text(reason))
    {
        return Err(ReplicationProtocolError::InvalidRequest(
            "masking must be present or its unavailability must be explained".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    for site in &request.independent_sites {
        if !required_text(&site.site_id)
            || site.site_id == request.source_site_id
            || !site_ids.insert(site.site_id.clone())
            || site.local_assay_capability_ids.is_empty()
            || site
                .local_assay_capability_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || site
                .local_assay_capability_ids
                .iter()
                .any(|value| !required_text(value))
            || site.authorization_reference.trim().is_empty()
            || site.local_protocol_digest.as_str().len() != 64
        {
            return Err(ReplicationProtocolError::InvalidRequest(
                "independent sites must be unique, distinct from the source, capability-bound, authorized, and content-addressed".into(),
            ));
        }
    }
    let mut arms = BTreeSet::new();
    if request
        .randomization
        .arm_order
        .iter()
        .any(|arm| !arms.insert(arm))
    {
        return Err(ReplicationProtocolError::InvalidRequest(
            "randomization arms must be unique".into(),
        ));
    }
    for deviation in &request.declared_deviations {
        if !required_text(&deviation.field)
            || !required_text(&deviation.source_value)
            || !required_text(&deviation.replication_value)
            || !required_text(&deviation.rationale)
        {
            return Err(ReplicationProtocolError::InvalidRequest(
                "declared deviations require source and replication values plus rationale".into(),
            ));
        }
    }
    for unknown in &request.explicit_unknowns {
        if !required_text(&unknown.field)
            || !required_text(&unknown.consequence)
            || !required_text(&unknown.resolution_owner)
        {
            return Err(ReplicationProtocolError::InvalidRequest(
                "unknowns require a field, consequence, and resolution owner".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(protocol: &ReplicationProtocol) -> Result<(), ReplicationProtocolError> {
    if protocol.feature_id != FEATURE_ID
        || protocol.output_schema != OUTPUT_SCHEMA
        || protocol.boundary != PRECLINICAL_BOUNDARY
        || !required_text(&protocol.source_site_id)
        || !required_text(&protocol.objective)
        || !canonical(&protocol.independent_site_order)
        || protocol.independent_sites.len() != protocol.independent_site_order.len()
        || protocol
            .independent_sites
            .iter()
            .map(|site| &site.site_id)
            .ne(protocol.independent_site_order.iter())
        || protocol.next_action.trim().is_empty()
        || (protocol.disposition == ReplicationProtocolDisposition::Ready
            && protocol
                .explicit_unknowns
                .iter()
                .any(|unknown| unknown.blocking))
    {
        return Err(ReplicationProtocolError::InvalidOutput(
            "identity, preclinical boundary, site ordering, disposition, or next action is invalid"
                .into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(protocol))
        .map_err(|error| ReplicationProtocolError::Digest(error.to_string()))?;
    if expected != protocol.digest {
        return Err(ReplicationProtocolError::InvalidOutput(
            "protocol digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl ReplicationProtocol {
    pub fn validate(&self) -> Result<(), ReplicationProtocolError> {
        validate_output(self)
    }
}

/// Compile a complete independent-site replication contract without dispatching any work.
pub fn compile_glioma_replication_protocol_schema(
    request: &ReplicationProtocolRequest,
) -> Result<ReplicationProtocol, ReplicationProtocolError> {
    validate_request(request)?;
    let mut independent_sites = request.independent_sites.clone();
    independent_sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let independent_site_order = independent_sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let mut deviations = request.declared_deviations.clone();
    deviations.sort_by(|left, right| {
        (&left.field, &left.replication_value, &left.source_value).cmp(&(
            &right.field,
            &right.replication_value,
            &right.source_value,
        ))
    });
    let mut unknowns = request.explicit_unknowns.clone();
    unknowns.sort_by(|left, right| left.field.cmp(&right.field));
    let disposition = if unknowns.iter().any(|unknown| unknown.blocking) {
        ReplicationProtocolDisposition::BlockedByUnknown
    } else {
        ReplicationProtocolDisposition::Ready
    };
    let next_action = match disposition {
        ReplicationProtocolDisposition::Ready => {
            "submit the explicit protocol to each independent laboratory for local approval".into()
        }
        ReplicationProtocolDisposition::BlockedByUnknown => {
            "resolve every blocking unknown before protocol execution or pooling".into()
        }
        ReplicationProtocolDisposition::Unresolved => {
            "repair the unresolved protocol fields before execution".into()
        }
    };
    let mut protocol = ReplicationProtocol {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        source_research_object: request.source_research_object.clone(),
        source_site_id: request.source_site_id.clone(),
        source_protocol_digest: request.source_protocol_digest.clone(),
        objective: request.objective.clone(),
        replication_question: request.replication_question.clone(),
        estimand: request.estimand.clone(),
        model_system: request.model_system,
        source_assay_id: request.source_assay_id.clone(),
        replication_assay_id: request.replication_assay_id.clone(),
        independent_site_order,
        independent_sites,
        randomization: request.randomization.clone(),
        masking: request.masking.clone(),
        stopping_rule: request.stopping_rule.clone(),
        prespecified_analysis: request.prespecified_analysis.clone(),
        declared_deviations: deviations,
        explicit_unknowns: unknowns,
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-protocol"),
    };
    protocol.digest = ContentHash::of_value(&digest_input(&protocol))
        .map_err(|error| ReplicationProtocolError::Digest(error.to_string()))?;
    validate_output(&protocol)?;
    Ok(protocol)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn test_request() -> ReplicationProtocolRequest {
        let seed = ContentHash::of_bytes(b"replication-seed");
        ReplicationProtocolRequest {
            source_research_object: LocalArtifactRef {
                artifact_id: "source-ro".into(),
                content_hash: ContentHash::of_bytes(b"source-ro"),
                content_type: "application/ro-crate+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            source_site_id: "site-source".into(),
            source_protocol_digest: ContentHash::of_bytes(b"source-protocol"),
            objective: "replicate invasion contrast".into(),
            replication_question: "does the preclinical contrast transport to independent sites?"
                .into(),
            estimand: ReplicationEstimand {
                outcome_id: "invasion-mm".into(),
                population: "organoids passing QC".into(),
                contrast: "treated minus control at day 7".into(),
                timepoint: "day-7".into(),
                effect_measure: "mean difference".into(),
            },
            model_system: GliomaModelSystem::Organoid,
            source_assay_id: "source-invasion".into(),
            replication_assay_id: "local-invasion".into(),
            independent_sites: vec![
                IndependentSiteSpec {
                    site_id: "site-b".into(),
                    local_assay_capability_ids: vec!["invasion".into()],
                    local_protocol_digest: ContentHash::of_bytes(b"site-b-protocol"),
                    authorization_reference: "approval-b".into(),
                },
                IndependentSiteSpec {
                    site_id: "site-a".into(),
                    local_assay_capability_ids: vec!["invasion".into()],
                    local_protocol_digest: ContentHash::of_bytes(b"site-a-protocol"),
                    authorization_reference: "approval-a".into(),
                },
            ],
            randomization: RandomizationSpec {
                unit: RandomizationUnit::Organoid,
                arm_order: vec!["control".into(), "treated".into()],
                block_size: 4,
                seed,
                allocation_ratio_milli: vec![500, 500],
            },
            masking: MaskingSpec {
                operator_masked: false,
                analyst_masked: true,
                evaluator_masked: true,
                unavailable_reason: None,
            },
            stopping_rule: StoppingRule {
                minimum_units: 20,
                maximum_units: 80,
                interim_looks: 2,
                positive_boundary_milli: 800,
                negative_boundary_milli: 200,
                stop_for_harm: true,
            },
            prespecified_analysis: PrespecifiedAnalysis {
                outcome_order: vec!["invasion-mm".into()],
                primary_contrast: "treated-control".into(),
                model_family: "robust-linear".into(),
                confidence_level_milli: 950,
                missingness_rule: "report missingness; do not impute primary endpoint".into(),
                multiplicity_rule: "one primary outcome".into(),
            },
            declared_deviations: vec![ProtocolDeviation {
                field: "matrix".into(),
                source_value: "matrigel".into(),
                replication_value: "collagen-i".into(),
                rationale: "local material availability; analyze as context-limited".into(),
                preapproved: true,
            }],
            explicit_unknowns: vec![ProtocolUnknown {
                field: "reader-calibration".into(),
                consequence: "may widen measurement uncertainty".into(),
                blocking: false,
                resolution_owner: "site-statistician".into(),
            }],
            replay_identity: ContentHash::of_bytes(b"replay"),
        }
    }

    #[test]
    fn protocol_is_canonical_and_replay_stable() {
        let protocol = compile_glioma_replication_protocol_schema(&test_request()).unwrap();
        assert_eq!(protocol.independent_site_order, vec!["site-a", "site-b"]);
        assert_eq!(protocol.disposition, ReplicationProtocolDisposition::Ready);
        protocol.validate().unwrap();
        assert_eq!(
            protocol,
            compile_glioma_replication_protocol_schema(&test_request()).unwrap()
        );
    }

    #[test]
    fn source_site_cannot_be_reused_as_an_independent_site() {
        let mut input = test_request();
        input.independent_sites[0].site_id = input.source_site_id.clone();
        assert!(matches!(
            compile_glioma_replication_protocol_schema(&input),
            Err(ReplicationProtocolError::InvalidRequest(_))
        ));
    }

    #[test]
    fn blocking_unknown_is_preserved_and_blocks_execution() {
        let mut input = test_request();
        input.explicit_unknowns[0].blocking = true;
        let protocol = compile_glioma_replication_protocol_schema(&input).unwrap();
        assert_eq!(
            protocol.disposition,
            ReplicationProtocolDisposition::BlockedByUnknown
        );
        assert!(protocol.next_action.contains("blocking unknown"));
    }

    #[test]
    fn human_source_artifact_is_rejected() {
        let mut input = test_request();
        input.source_research_object.contains_human_data = true;
        assert!(compile_glioma_replication_protocol_schema(&input).is_err());
    }
}
