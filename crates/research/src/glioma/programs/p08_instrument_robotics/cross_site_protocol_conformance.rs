//! Cross-site protocol conformance for federated preclinical glioma acquisition.
//!
//! Conformance is semantic, not string-based.  The comparator pins a reference protocol version,
//! checks ordered step identity/roles/units, bounds numeric adaptations by declared tolerances,
//! requires compatible device capabilities and fresh calibration classes, and separates conformant,
//! adapted, stale, blocked, and unknown sites.  It emits no sample data and never permits pooling
//! unless every included site has an explicit safe conformance state.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaProtocolConformanceMatrix1@1";
pub const MAX_STEPS: usize = 512;
pub const MAX_SITES: usize = 256;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolStepSpec {
    pub step_id: String,
    pub semantic_role: String,
    pub unit: String,
    pub value_milli: i64,
    pub tolerance_milli: u32,
    pub required_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceProtocol {
    pub protocol_id: String,
    pub version: String,
    pub step_order: Vec<ProtocolStepSpec>,
    pub required_calibration_class: String,
    pub expires_at_tick: u64,
    pub protocol_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteProtocolRealization {
    pub site_id: String,
    pub protocol_id: String,
    pub version: String,
    pub step_order: Vec<ProtocolStepSpec>,
    pub capability_order: Vec<String>,
    pub calibration_class: String,
    pub calibration_valid_until_tick: u64,
    pub adaptation_order: Vec<String>,
    pub descriptor_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolConformanceRequest {
    pub reference: ReferenceProtocol,
    pub site_order: Vec<SiteProtocolRealization>,
    pub current_tick: u64,
    pub minimum_quorum: usize,
    pub allow_bounded_adaptations: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtocolConformanceStatus {
    Conformant,
    Adapted,
    Stale,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteConformanceResult {
    pub site_id: String,
    pub status: ProtocolConformanceStatus,
    pub deviation_order: Vec<String>,
    pub allowed_adaptation_order: Vec<String>,
    pub blocking_order: Vec<String>,
    pub calibration_valid_until_tick: u64,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolConformanceMatrix {
    pub feature_id: String,
    pub output_schema: String,
    pub protocol_id: String,
    pub reference_version: String,
    pub site_order: Vec<String>,
    pub results: Vec<SiteConformanceResult>,
    pub conformant_site_order: Vec<String>,
    pub excluded_site_order: Vec<String>,
    pub pooling_permitted: bool,
    pub quorum_met: bool,
    pub negative_evidence_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProtocolConformanceError {
    #[error("protocol conformance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("protocol conformance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("protocol conformance digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique(values: &[String]) -> bool {
    values.len() <= MAX_SITES
        && values.iter().all(|value| safe_text(value))
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn reference_body(reference: &ReferenceProtocol) -> serde_json::Value {
    serde_json::json!({
        "protocol_id": reference.protocol_id,
        "version": reference.version,
        "step_order": reference.step_order,
        "required_calibration_class": reference.required_calibration_class,
        "expires_at_tick": reference.expires_at_tick,
    })
}

fn site_body(site: &SiteProtocolRealization) -> serde_json::Value {
    serde_json::json!({
        "site_id": site.site_id,
        "protocol_id": site.protocol_id,
        "version": site.version,
        "step_order": site.step_order,
        "capability_order": site.capability_order,
        "calibration_class": site.calibration_class,
        "calibration_valid_until_tick": site.calibration_valid_until_tick,
        "adaptation_order": site.adaptation_order,
    })
}

fn matrix_body(matrix: &ProtocolConformanceMatrix) -> serde_json::Value {
    serde_json::json!({
        "feature_id": matrix.feature_id,
        "output_schema": matrix.output_schema,
        "protocol_id": matrix.protocol_id,
        "reference_version": matrix.reference_version,
        "site_order": matrix.site_order,
        "results": matrix.results,
        "conformant_site_order": matrix.conformant_site_order,
        "excluded_site_order": matrix.excluded_site_order,
        "pooling_permitted": matrix.pooling_permitted,
        "quorum_met": matrix.quorum_met,
        "negative_evidence_order": matrix.negative_evidence_order,
    })
}

fn validate_step(step: &ProtocolStepSpec) -> bool {
    safe_text(&step.step_id)
        && safe_text(&step.semantic_role)
        && safe_text(&step.unit)
        && safe_text(&step.required_capability)
        && step.tolerance_milli <= 1_000_000
}

fn validate_request(request: &ProtocolConformanceRequest) -> Result<(), ProtocolConformanceError> {
    let reference = &request.reference;
    if !safe_text(&reference.protocol_id)
        || !safe_text(&reference.version)
        || reference.step_order.is_empty()
        || reference.step_order.len() > MAX_STEPS
        || !safe_text(&reference.required_calibration_class)
        || reference.expires_at_tick == 0
        || !valid_hash(&reference.protocol_digest)
        || request.site_order.is_empty()
        || request.site_order.len() > MAX_SITES
        || request.minimum_quorum == 0
        || request.minimum_quorum > request.site_order.len()
        || request.current_tick == 0
        || request.current_tick > reference.expires_at_tick
        || request
            .site_order
            .iter()
            .map(|site| site.site_id.clone())
            .collect::<BTreeSet<_>>()
            .len()
            != request.site_order.len()
    {
        return Err(ProtocolConformanceError::InvalidRequest(
            "bounded reference, site quorum, expiry, and unique protocol identities are required"
                .into(),
        ));
    }
    if !reference.step_order.iter().all(validate_step)
        || reference
            .step_order
            .windows(2)
            .any(|pair| pair[0].step_id >= pair[1].step_id)
    {
        return Err(ProtocolConformanceError::InvalidRequest(
            "reference steps must be bounded, valid, and canonically ordered".into(),
        ));
    }
    let expected = ContentHash::of_value(&reference_body(reference))
        .map_err(|error| ProtocolConformanceError::Digest(error.to_string()))?;
    if expected != reference.protocol_digest {
        return Err(ProtocolConformanceError::InvalidRequest(
            "reference protocol digest is tampered".into(),
        ));
    }
    for site in &request.site_order {
        if !safe_text(&site.site_id)
            || !safe_text(&site.protocol_id)
            || !safe_text(&site.version)
            || site.step_order.len() > MAX_STEPS
            || !site.step_order.iter().all(validate_step)
            || site
                .step_order
                .windows(2)
                .any(|pair| pair[0].step_id >= pair[1].step_id)
            || !site.capability_order.iter().all(|value| safe_text(value))
            || !canonical(&site.capability_order)
            || !safe_text(&site.calibration_class)
            || site.calibration_valid_until_tick == 0
            || !site.adaptation_order.iter().all(|value| safe_text(value))
            || !canonical(&site.adaptation_order)
            || !valid_hash(&site.descriptor_digest)
        {
            return Err(ProtocolConformanceError::InvalidRequest(format!(
                "site {} has invalid descriptor bounds or ordering",
                site.site_id
            )));
        }
        let expected = ContentHash::of_value(&site_body(site))
            .map_err(|error| ProtocolConformanceError::Digest(error.to_string()))?;
        if expected != site.descriptor_digest {
            return Err(ProtocolConformanceError::InvalidRequest(format!(
                "site {} descriptor digest is tampered",
                site.site_id
            )));
        }
    }
    Ok(())
}

impl ProtocolConformanceMatrix {
    pub fn validate(&self) -> Result<(), ProtocolConformanceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.protocol_id)
            || !safe_text(&self.reference_version)
            || self.site_order.len() != self.results.len()
            || !canonical(&self.site_order)
            || !unique(&self.site_order)
            || !canonical(&self.conformant_site_order)
            || !canonical(&self.excluded_site_order)
            || !canonical(&self.negative_evidence_order)
            || self.results.iter().any(|result| {
                !safe_text(&result.site_id)
                    || !canonical(&result.deviation_order)
                    || !canonical(&result.allowed_adaptation_order)
                    || !canonical(&result.blocking_order)
                    || result.calibration_valid_until_tick == 0
                    || !valid_hash(&result.result_digest)
            })
            || !self.digest.as_str().len().eq(&64)
        {
            return Err(ProtocolConformanceError::InvalidOutput(
                "conformance identity, site alignment, partitions, or digest shape is invalid"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&matrix_body(self))
            .map_err(|error| ProtocolConformanceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProtocolConformanceError::InvalidOutput(
                "conformance matrix digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

/// Compare version-pinned protocol semantics and calibration/capability summaries across sites.
pub fn assess_glioma_cross_site_protocol_conformance(
    request: &ProtocolConformanceRequest,
) -> Result<ProtocolConformanceMatrix, ProtocolConformanceError> {
    validate_request(request)?;
    let reference = &request.reference;
    let mut results = Vec::new();
    for site in &request.site_order {
        let mut deviations = BTreeSet::new();
        let mut allowed = BTreeSet::new();
        let mut blocking = BTreeSet::new();
        if site.protocol_id != reference.protocol_id {
            blocking.insert("protocol-id-mismatch".into());
        }
        if site.version != reference.version {
            blocking.insert("reference-version-mismatch".into());
        }
        if site.calibration_class != reference.required_calibration_class {
            blocking.insert("calibration-class-mismatch".into());
        }
        if site.calibration_valid_until_tick < request.current_tick {
            blocking.insert("calibration-expired".into());
        }
        if site.step_order.len() != reference.step_order.len() {
            blocking.insert("step-count-mismatch".into());
        }
        for (index, reference_step) in reference.step_order.iter().enumerate() {
            let Some(site_step) = site.step_order.get(index) else {
                blocking.insert(format!("missing-step:{}", reference_step.step_id));
                continue;
            };
            if site_step.step_id != reference_step.step_id {
                blocking.insert(format!("step-identity:{}", reference_step.step_id));
                continue;
            }
            if site_step.semantic_role != reference_step.semantic_role {
                blocking.insert(format!("semantic-role:{}", reference_step.step_id));
            }
            if site_step.unit != reference_step.unit {
                blocking.insert(format!("unit:{}", reference_step.step_id));
            }
            if !site
                .capability_order
                .binary_search(&reference_step.required_capability)
                .is_ok()
            {
                blocking.insert(format!("capability:{}", reference_step.required_capability));
            }
            let delta = site_step.value_milli.abs_diff(reference_step.value_milli);
            if delta > reference_step.tolerance_milli as u64 {
                blocking.insert(format!("tolerance:{}", reference_step.step_id));
            } else if delta > 0 {
                deviations.insert(format!(
                    "bounded-value-adaptation:{}",
                    reference_step.step_id
                ));
                allowed.insert(format!("{}:{}", reference_step.step_id, delta));
            }
        }
        for adaptation in &site.adaptation_order {
            if !adaptation.starts_with("bounded-value-adaptation:") {
                deviations.insert(format!("declared-adaptation:{adaptation}"));
            }
        }
        let status = if !blocking.is_empty() {
            if site.calibration_valid_until_tick < request.current_tick {
                ProtocolConformanceStatus::Stale
            } else {
                ProtocolConformanceStatus::Blocked
            }
        } else if deviations.is_empty() {
            ProtocolConformanceStatus::Conformant
        } else if request.allow_bounded_adaptations {
            ProtocolConformanceStatus::Adapted
        } else {
            ProtocolConformanceStatus::Blocked
        };
        let mut result = SiteConformanceResult {
            site_id: site.site_id.clone(),
            status,
            deviation_order: deviations.into_iter().collect(),
            allowed_adaptation_order: allowed.into_iter().collect(),
            blocking_order: blocking.into_iter().collect(),
            calibration_valid_until_tick: site.calibration_valid_until_tick,
            result_digest: ContentHash::of_bytes(b"unsealed-site-conformance"),
        };
        result.result_digest = ContentHash::of_value(&serde_json::json!({
            "site_id": result.site_id,
            "status": result.status,
            "deviation_order": result.deviation_order,
            "allowed_adaptation_order": result.allowed_adaptation_order,
            "blocking_order": result.blocking_order,
            "calibration_valid_until_tick": result.calibration_valid_until_tick,
        }))
        .map_err(|error| ProtocolConformanceError::Digest(error.to_string()))?;
        results.push(result);
    }
    results.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = results
        .iter()
        .map(|result| result.site_id.clone())
        .collect::<Vec<_>>();
    let mut conformant = results
        .iter()
        .filter(|result| {
            matches!(
                result.status,
                ProtocolConformanceStatus::Conformant | ProtocolConformanceStatus::Adapted
            )
        })
        .map(|result| result.site_id.clone())
        .collect::<Vec<_>>();
    conformant.sort();
    let mut excluded = results
        .iter()
        .filter(|result| {
            !matches!(
                result.status,
                ProtocolConformanceStatus::Conformant | ProtocolConformanceStatus::Adapted
            )
        })
        .map(|result| result.site_id.clone())
        .collect::<Vec<_>>();
    excluded.sort();
    let quorum_met = conformant.len() >= request.minimum_quorum;
    let pooling_permitted = quorum_met && excluded.is_empty();
    let negative = results
        .iter()
        .filter(|result| !result.blocking_order.is_empty())
        .map(|result| format!("{}:{}", result.site_id, result.blocking_order.join(",")))
        .collect::<Vec<_>>();
    let mut matrix = ProtocolConformanceMatrix {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        protocol_id: reference.protocol_id.clone(),
        reference_version: reference.version.clone(),
        site_order,
        results,
        conformant_site_order: conformant,
        excluded_site_order: excluded,
        pooling_permitted,
        quorum_met,
        negative_evidence_order: negative,
        digest: ContentHash::of_bytes(b"unsealed-protocol-conformance"),
    };
    matrix.digest = ContentHash::of_value(&matrix_body(&matrix))
        .map_err(|error| ProtocolConformanceError::Digest(error.to_string()))?;
    matrix.validate()?;
    Ok(matrix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &serde_json::Value) -> ContentHash {
        ContentHash::of_value(value).unwrap()
    }

    fn step(id: &str, value: i64) -> ProtocolStepSpec {
        ProtocolStepSpec {
            step_id: id.into(),
            semantic_role: format!("role-{id}"),
            unit: "millivolt".into(),
            value_milli: value,
            tolerance_milli: 20,
            required_capability: "imaging".into(),
        }
    }

    fn reference() -> ReferenceProtocol {
        let mut reference = ReferenceProtocol {
            protocol_id: "glioma-invasion".into(),
            version: "2026.1".into(),
            step_order: vec![step("capture", 100), step("expose", 200)],
            required_calibration_class: "cal-v2".into(),
            expires_at_tick: 100,
            protocol_digest: hash(&serde_json::json!({"unsealed": true})),
        };
        reference.protocol_digest = ContentHash::of_value(&reference_body(&reference)).unwrap();
        reference
    }

    fn site(
        reference: &ReferenceProtocol,
        id: &str,
        steps: Vec<ProtocolStepSpec>,
        calibration: u64,
    ) -> SiteProtocolRealization {
        let mut site = SiteProtocolRealization {
            site_id: id.into(),
            protocol_id: reference.protocol_id.clone(),
            version: reference.version.clone(),
            step_order: steps,
            capability_order: vec!["imaging".into()],
            calibration_class: "cal-v2".into(),
            calibration_valid_until_tick: calibration,
            adaptation_order: Vec::new(),
            descriptor_digest: hash(&serde_json::json!({"unsealed": id})),
        };
        site.descriptor_digest = ContentHash::of_value(&site_body(&site)).unwrap();
        site
    }

    fn request(sites: Vec<SiteProtocolRealization>) -> ProtocolConformanceRequest {
        ProtocolConformanceRequest {
            reference: reference(),
            site_order: sites,
            current_tick: 5,
            minimum_quorum: 1,
            allow_bounded_adaptations: true,
        }
    }

    #[test]
    fn accepts_exact_and_bounded_adapted_sites() {
        let reference = reference();
        let mut adapted = site(
            &reference,
            "site-b",
            vec![step("capture", 110), step("expose", 200)],
            100,
        );
        adapted.descriptor_digest = ContentHash::of_value(&site_body(&adapted)).unwrap();
        let output = assess_glioma_cross_site_protocol_conformance(&request(vec![
            site(&reference, "site-a", reference.step_order.clone(), 100),
            adapted,
        ]))
        .unwrap();
        assert!(output.pooling_permitted);
        assert_eq!(output.conformant_site_order, vec!["site-a", "site-b"]);
        assert_eq!(output.results[1].status, ProtocolConformanceStatus::Adapted);
    }

    #[test]
    fn blocks_semantic_unit_and_capability_mismatch() {
        let reference = reference();
        let mut invalid = site(&reference, "site-a", reference.step_order.clone(), 100);
        invalid.step_order[0].unit = "micron".into();
        invalid.capability_order = vec!["spectroscopy".into()];
        invalid.descriptor_digest = ContentHash::of_value(&site_body(&invalid)).unwrap();
        let output =
            assess_glioma_cross_site_protocol_conformance(&request(vec![invalid])).unwrap();
        assert!(!output.pooling_permitted);
        assert!(output.results[0]
            .blocking_order
            .iter()
            .any(|value| value == "unit:capture"));
        assert!(output.results[0]
            .blocking_order
            .iter()
            .any(|value| value == "capability:imaging"));
    }

    #[test]
    fn stale_calibration_is_excluded_from_quorum() {
        let reference = reference();
        let output = assess_glioma_cross_site_protocol_conformance(&request(vec![site(
            &reference,
            "site-a",
            reference.step_order.clone(),
            4,
        )]))
        .unwrap();
        assert_eq!(output.results[0].status, ProtocolConformanceStatus::Stale);
        assert!(!output.quorum_met);
        assert!(!output.pooling_permitted);
    }

    #[test]
    fn tampered_reference_is_rejected() {
        let mut reference = reference();
        reference.version = "tampered".into();
        let site = site(&reference, "site-a", reference.step_order.clone(), 100);
        assert!(matches!(
            assess_glioma_cross_site_protocol_conformance(&ProtocolConformanceRequest {
                reference,
                site_order: vec![site],
                current_tick: 5,
                minimum_quorum: 1,
                allow_bounded_adaptations: true
            }),
            Err(ProtocolConformanceError::InvalidRequest(_))
        ));
    }
}
