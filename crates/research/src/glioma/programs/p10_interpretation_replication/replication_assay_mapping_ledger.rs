//! Cross-site assay equivalence ledger for independent preclinical glioma replication.
//!
//! The ledger makes unit, semantic, detection-limit, and calibration differences executable.
//! It refuses to pool uncertain mappings: a replication can remain useful while explicitly
//! reporting that a measure is context-limited, non-equivalent, or unresolved.

use super::replication_protocol_schema::{ReplicationProtocol, ReplicationProtocolDisposition};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationAssayMappingLedger1@1";
pub const MAX_VARIABLES: usize = 512;
pub const MAX_MAPPINGS: usize = 2_048;
pub const MAX_CALIBRATION_POINTS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayVariableSpec {
    pub variable_id: String,
    pub semantic_role: String,
    pub unit: String,
    pub lower_limit_milli: i32,
    pub upper_limit_milli: i32,
    pub direction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalibrationObservation {
    pub source_milli: i32,
    pub replication_milli: i32,
    pub tolerance_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceTier {
    Exact,
    CalibratedTransform,
    ContextLimited,
    NonEquivalent,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingTransform {
    Identity,
    Affine { scale_milli: i32, offset_milli: i32 },
    Log1p,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MappingProposal {
    pub source_variable_id: String,
    pub replication_variable_id: String,
    pub tier: EquivalenceTier,
    pub transform: MappingTransform,
    pub calibration: Vec<CalibrationObservation>,
    pub evidence_digest: ContentHash,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayMappingLedgerRequest {
    pub protocol: ReplicationProtocol,
    pub source_variables: Vec<AssayVariableSpec>,
    pub replication_variables: Vec<AssayVariableSpec>,
    pub proposals: Vec<MappingProposal>,
    pub replay_identity: ContentHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayMappingDisposition {
    Equivalent,
    ContextLimited,
    NonEquivalent,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayMappingLedger {
    pub feature_id: String,
    pub output_schema: String,
    pub protocol_digest: ContentHash,
    pub source_assay_id: String,
    pub replication_assay_id: String,
    pub mapping_order: Vec<String>,
    pub accepted_mapping_order: Vec<String>,
    pub non_pooled_mapping_order: Vec<String>,
    pub unresolved_mapping_order: Vec<String>,
    pub mappings: Vec<MappingProposal>,
    pub disposition: AssayMappingDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub replay_identity: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AssayMappingLedgerError {
    #[error("assay mapping request is invalid: {0}")]
    InvalidRequest(String),
    #[error("assay mapping output is invalid: {0}")]
    InvalidOutput(String),
    #[error("assay mapping digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn mapping_key(mapping: &MappingProposal) -> String {
    format!(
        "{}::{}",
        mapping.source_variable_id, mapping.replication_variable_id
    )
}

fn digest_input(ledger: &AssayMappingLedger) -> serde_json::Value {
    serde_json::json!({
        "feature_id": ledger.feature_id,
        "output_schema": ledger.output_schema,
        "protocol_digest": ledger.protocol_digest,
        "source_assay_id": ledger.source_assay_id,
        "replication_assay_id": ledger.replication_assay_id,
        "mapping_order": ledger.mapping_order,
        "accepted_mapping_order": ledger.accepted_mapping_order,
        "non_pooled_mapping_order": ledger.non_pooled_mapping_order,
        "unresolved_mapping_order": ledger.unresolved_mapping_order,
        "mappings": ledger.mappings,
        "disposition": ledger.disposition,
        "negative_evidence": ledger.negative_evidence,
        "uncertainty": ledger.uncertainty,
        "next_action": ledger.next_action,
        "replay_identity": ledger.replay_identity,
    })
}

fn validate_variable(variable: &AssayVariableSpec) -> bool {
    !variable.variable_id.trim().is_empty()
        && !variable.semantic_role.trim().is_empty()
        && !variable.unit.trim().is_empty()
        && !variable.direction.trim().is_empty()
        && variable.lower_limit_milli < variable.upper_limit_milli
}

fn transform_value(transform: &MappingTransform, source: i32) -> Option<i64> {
    match transform {
        MappingTransform::Identity => Some(i64::from(source)),
        MappingTransform::Affine {
            scale_milli,
            offset_milli,
        } => Some(
            i64::from(source)
                .checked_mul(i64::from(*scale_milli))?
                .checked_div(1_000)?
                .checked_add(i64::from(*offset_milli))?,
        ),
        MappingTransform::Log1p => (source >= 0).then(|| i64::from(source)),
        MappingTransform::None => None,
    }
}

fn validate_proposal(
    proposal: &MappingProposal,
    source: &AssayVariableSpec,
    replication: &AssayVariableSpec,
) -> Result<(), AssayMappingLedgerError> {
    if proposal.evidence_digest.as_str().len() != 64
        || proposal.rationale.trim().is_empty()
        || proposal.calibration.len() > MAX_CALIBRATION_POINTS
        || proposal
            .calibration
            .iter()
            .any(|point| point.tolerance_milli == 0)
    {
        return Err(AssayMappingLedgerError::InvalidRequest(
            "each mapping requires evidence, rationale, bounded calibration, and positive tolerances".into(),
        ));
    }
    let identity = matches!(proposal.transform, MappingTransform::Identity);
    let transform_present = !matches!(proposal.transform, MappingTransform::None);
    match proposal.tier {
        EquivalenceTier::Exact => {
            if source.semantic_role != replication.semantic_role
                || source.unit != replication.unit
                || !identity
            {
                return Err(AssayMappingLedgerError::InvalidRequest(
                    "exact mappings require identical semantics and units with identity transform"
                        .into(),
                ));
            }
        }
        EquivalenceTier::CalibratedTransform => {
            if !transform_present || proposal.calibration.len() < 3 {
                return Err(AssayMappingLedgerError::InvalidRequest(
                    "calibrated mappings require a transform and at least three calibration observations".into(),
                ));
            }
            for point in &proposal.calibration {
                let predicted = transform_value(&proposal.transform, point.source_milli)
                    .ok_or_else(|| {
                        AssayMappingLedgerError::InvalidRequest(
                            "calibration transform cannot be evaluated for every point".into(),
                        )
                    })?;
                if (predicted - i64::from(point.replication_milli)).unsigned_abs()
                    > u64::from(point.tolerance_milli)
                {
                    return Err(AssayMappingLedgerError::InvalidRequest(
                        "calibration residual exceeds its declared tolerance".into(),
                    ));
                }
            }
        }
        EquivalenceTier::ContextLimited => {
            if !transform_present || source.semantic_role != replication.semantic_role {
                return Err(AssayMappingLedgerError::InvalidRequest(
                    "context-limited mappings require a declared transform and matching semantic role".into(),
                ));
            }
        }
        EquivalenceTier::NonEquivalent | EquivalenceTier::Unresolved => {
            if !matches!(proposal.transform, MappingTransform::None) {
                return Err(AssayMappingLedgerError::InvalidRequest(
                    "non-equivalent and unresolved mappings cannot carry a pooling transform"
                        .into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_request(request: &AssayMappingLedgerRequest) -> Result<(), AssayMappingLedgerError> {
    request
        .protocol
        .validate()
        .map_err(|error| AssayMappingLedgerError::InvalidRequest(error.to_string()))?;
    if request.source_variables.is_empty()
        || request.replication_variables.is_empty()
        || request.source_variables.len() > MAX_VARIABLES
        || request.replication_variables.len() > MAX_VARIABLES
        || request.proposals.is_empty()
        || request.proposals.len() > MAX_MAPPINGS
        || request.replay_identity.as_str().len() != 64
    {
        return Err(AssayMappingLedgerError::InvalidRequest(
            "source and replication dictionaries, proposals, and replay identity are required"
                .into(),
        ));
    }
    let mut source = BTreeMap::new();
    for variable in &request.source_variables {
        if !validate_variable(variable)
            || source
                .insert(variable.variable_id.clone(), variable)
                .is_some()
        {
            return Err(AssayMappingLedgerError::InvalidRequest(
                "source assay variable ids must be unique and typed".into(),
            ));
        }
    }
    let mut replication = BTreeMap::new();
    for variable in &request.replication_variables {
        if !validate_variable(variable)
            || replication
                .insert(variable.variable_id.clone(), variable)
                .is_some()
        {
            return Err(AssayMappingLedgerError::InvalidRequest(
                "replication assay variable ids must be unique and typed".into(),
            ));
        }
    }
    let mut pairs = BTreeSet::new();
    for proposal in &request.proposals {
        let source_variable = source.get(&proposal.source_variable_id).ok_or_else(|| {
            AssayMappingLedgerError::InvalidRequest(
                "mapping references unknown source variable".into(),
            )
        })?;
        let replication_variable = replication
            .get(&proposal.replication_variable_id)
            .ok_or_else(|| {
                AssayMappingLedgerError::InvalidRequest(
                    "mapping references unknown replication variable".into(),
                )
            })?;
        if !pairs.insert(mapping_key(proposal)) {
            return Err(AssayMappingLedgerError::InvalidRequest(
                "each source/replication variable pair may appear once".into(),
            ));
        }
        validate_proposal(proposal, source_variable, replication_variable)?;
    }
    Ok(())
}

fn validate_output(ledger: &AssayMappingLedger) -> Result<(), AssayMappingLedgerError> {
    if ledger.feature_id != FEATURE_ID
        || ledger.output_schema != OUTPUT_SCHEMA
        || !canonical(&ledger.mapping_order)
        || !canonical(&ledger.accepted_mapping_order)
        || !canonical(&ledger.non_pooled_mapping_order)
        || !canonical(&ledger.unresolved_mapping_order)
        || ledger.mappings.len() != ledger.mapping_order.len()
        || ledger.next_action.trim().is_empty()
        || ledger
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        || ledger.uncertainty.iter().any(|item| item.trim().is_empty())
    {
        return Err(AssayMappingLedgerError::InvalidOutput(
            "ledger identity, ordering, mapping count, rationale, or uncertainty fields are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(ledger))
        .map_err(|error| AssayMappingLedgerError::Digest(error.to_string()))?;
    if expected != ledger.digest {
        return Err(AssayMappingLedgerError::InvalidOutput(
            "ledger digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl AssayMappingLedger {
    pub fn validate(&self) -> Result<(), AssayMappingLedgerError> {
        validate_output(self)
    }
}

/// Compile a mapping ledger.  No uncertain mapping is promoted into a pooled equivalence.
pub fn compile_glioma_replication_assay_mapping_ledger(
    request: &AssayMappingLedgerRequest,
) -> Result<AssayMappingLedger, AssayMappingLedgerError> {
    validate_request(request)?;
    let mut mappings = request.proposals.clone();
    mappings.sort_by_key(mapping_key);
    let mapping_order = mappings.iter().map(mapping_key).collect::<Vec<_>>();
    let accepted_mapping_order = mappings
        .iter()
        .filter(|mapping| {
            matches!(
                mapping.tier,
                EquivalenceTier::Exact | EquivalenceTier::CalibratedTransform
            )
        })
        .map(mapping_key)
        .collect::<Vec<_>>();
    let non_pooled_mapping_order = mappings
        .iter()
        .filter(|mapping| {
            matches!(
                mapping.tier,
                EquivalenceTier::ContextLimited | EquivalenceTier::NonEquivalent
            )
        })
        .map(mapping_key)
        .collect::<Vec<_>>();
    let unresolved_mapping_order = mappings
        .iter()
        .filter(|mapping| mapping.tier == EquivalenceTier::Unresolved)
        .map(mapping_key)
        .collect::<Vec<_>>();
    let mut negative_evidence = Vec::new();
    if !non_pooled_mapping_order.is_empty() {
        negative_evidence.push("one or more assay mappings are not poolable".into());
    }
    if !unresolved_mapping_order.is_empty() {
        negative_evidence.push("unresolved assay mappings remain open".into());
    }
    let has_context_limited = mappings
        .iter()
        .any(|mapping| mapping.tier == EquivalenceTier::ContextLimited);
    let has_non_equivalent = mappings
        .iter()
        .any(|mapping| mapping.tier == EquivalenceTier::NonEquivalent);
    let disposition = if !unresolved_mapping_order.is_empty() {
        AssayMappingDisposition::Unresolved
    } else if has_context_limited {
        AssayMappingDisposition::ContextLimited
    } else if has_non_equivalent {
        AssayMappingDisposition::NonEquivalent
    } else if request.protocol.disposition != ReplicationProtocolDisposition::Ready {
        AssayMappingDisposition::Unresolved
    } else {
        AssayMappingDisposition::Equivalent
    };
    let mut uncertainty = vec![
        "equivalence describes measurement comparability, not biological replication success"
            .into(),
    ];
    if request.protocol.disposition != ReplicationProtocolDisposition::Ready {
        uncertainty.push("upstream protocol is not execution-ready".into());
    }
    let next_action = match disposition {
        AssayMappingDisposition::Equivalent =>
            "attach the ledger to the prespecified replication analysis and retain calibration evidence".into(),
        AssayMappingDisposition::ContextLimited =>
            "report context-limited measures separately and do not pool them without a new validation".into(),
        AssayMappingDisposition::NonEquivalent =>
            "select a comparable assay or report the measure as non-equivalent".into(),
        AssayMappingDisposition::Unresolved =>
            "resolve mapping or protocol uncertainty before interpretation".into(),
    };
    let mut ledger = AssayMappingLedger {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        protocol_digest: request.protocol.digest.clone(),
        source_assay_id: request.protocol.source_assay_id.clone(),
        replication_assay_id: request.protocol.replication_assay_id.clone(),
        mapping_order,
        accepted_mapping_order,
        non_pooled_mapping_order,
        unresolved_mapping_order,
        mappings,
        disposition,
        negative_evidence,
        uncertainty,
        next_action,
        replay_identity: request.replay_identity.clone(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-assay-mapping-ledger"),
    };
    ledger.digest = ContentHash::of_value(&digest_input(&ledger))
        .map_err(|error| AssayMappingLedgerError::Digest(error.to_string()))?;
    validate_output(&ledger)?;
    Ok(ledger)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::replication_protocol_schema::tests::test_request as protocol_request;

    fn variable(id: &str, role: &str, unit: &str) -> AssayVariableSpec {
        AssayVariableSpec {
            variable_id: id.into(),
            semantic_role: role.into(),
            unit: unit.into(),
            lower_limit_milli: 0,
            upper_limit_milli: 10_000,
            direction: "higher-is-more".into(),
        }
    }

    fn request() -> AssayMappingLedgerRequest {
        let protocol =
            super::super::replication_protocol_schema::compile_glioma_replication_protocol_schema(
                &protocol_request(),
            )
            .unwrap();
        AssayMappingLedgerRequest {
            protocol,
            source_variables: vec![variable("invasion", "invasion-distance", "micron")],
            replication_variables: vec![variable("local-invasion", "invasion-distance", "micron")],
            proposals: vec![MappingProposal {
                source_variable_id: "invasion".into(),
                replication_variable_id: "local-invasion".into(),
                tier: EquivalenceTier::Exact,
                transform: MappingTransform::Identity,
                calibration: Vec::new(),
                evidence_digest: ContentHash::of_bytes(b"mapping-evidence"),
                rationale: "same semantic endpoint and unit".into(),
            }],
            replay_identity: ContentHash::of_bytes(b"ledger-replay"),
        }
    }

    #[test]
    fn exact_mapping_is_poolable_and_replay_stable() {
        let ledger = compile_glioma_replication_assay_mapping_ledger(&request()).unwrap();
        assert_eq!(ledger.disposition, AssayMappingDisposition::Equivalent);
        assert_eq!(
            ledger.accepted_mapping_order,
            vec!["invasion::local-invasion"]
        );
        ledger.validate().unwrap();
        assert_eq!(
            ledger,
            compile_glioma_replication_assay_mapping_ledger(&request()).unwrap()
        );
    }

    #[test]
    fn unit_mismatch_cannot_be_declared_exact() {
        let mut input = request();
        input.replication_variables[0].unit = "pixel".into();
        assert!(compile_glioma_replication_assay_mapping_ledger(&input).is_err());
    }

    #[test]
    fn unresolved_mapping_is_never_poolable() {
        let mut input = request();
        input.proposals[0].tier = EquivalenceTier::Unresolved;
        input.proposals[0].transform = MappingTransform::None;
        let ledger = compile_glioma_replication_assay_mapping_ledger(&input).unwrap();
        assert_eq!(ledger.disposition, AssayMappingDisposition::Unresolved);
        assert!(ledger.accepted_mapping_order.is_empty());
        assert_eq!(ledger.unresolved_mapping_order.len(), 1);
    }
}
