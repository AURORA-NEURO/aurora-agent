//! Typed, privacy-preserving research-site capability envelopes for glioma federation planning.
//!
//! A site advertises only approved capability metadata: model systems, assay classes, standards,
//! local compute, and review capacity. Expired, unapproved, weakly attested, or protected fields
//! are omitted deterministically so a planner cannot mistake stale capacity for executable work.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaSiteCapabilityEnvelope1@1";
pub const MAX_CAPABILITIES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteCapabilityRecord {
    pub capability_id: String,
    pub model_system: String,
    pub assay_class: String,
    pub standard_order: Vec<String>,
    pub compute_class: String,
    pub review_capacity: u16,
    pub confidence_milli: u16,
    pub valid_until_epoch: u64,
    pub approved: bool,
    pub attestation_digest: ContentHash,
    pub local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteCapabilityPolicy {
    pub now_epoch: u64,
    pub required_model_systems: Vec<String>,
    pub required_assay_classes: Vec<String>,
    pub required_standards: Vec<String>,
    pub min_confidence_milli: u16,
    pub require_local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteCapabilityRequest {
    pub site_id: String,
    pub registry_version: String,
    pub capabilities: Vec<SiteCapabilityRecord>,
    pub policy: SiteCapabilityPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteCapabilityDisposition {
    Eligible,
    Partial,
    Ineligible,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteCapabilityEnvelope {
    pub feature_id: String,
    pub output_schema: String,
    pub site_id: String,
    pub registry_version: String,
    pub eligible_capability_order: Vec<String>,
    pub omitted_capability_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub model_system_order: Vec<String>,
    pub assay_class_order: Vec<String>,
    pub standard_order: Vec<String>,
    pub total_review_capacity: u32,
    pub min_confidence_milli: u16,
    pub disposition: SiteCapabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SiteCapabilityError {
    #[error("site capability request is invalid: {0}")]
    InvalidRequest(String),
    #[error("site capability envelope is invalid: {0}")]
    InvalidOutput(String),
    #[error("site capability digest failed: {0}")]
    Digest(String),
}

fn identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-@".contains(&byte))
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &SiteCapabilityEnvelope) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "site_id": output.site_id,
        "registry_version": output.registry_version,
        "eligible_capability_order": output.eligible_capability_order,
        "omitted_capability_order": output.omitted_capability_order,
        "omission_order": output.omission_order,
        "model_system_order": output.model_system_order,
        "assay_class_order": output.assay_class_order,
        "standard_order": output.standard_order,
        "total_review_capacity": output.total_review_capacity,
        "min_confidence_milli": output.min_confidence_milli,
        "disposition": output.disposition,
    })
}

impl SiteCapabilityEnvelope {
    pub fn validate(&self) -> Result<(), SiteCapabilityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !identifier(&self.site_id)
            || !identifier(&self.registry_version)
            || !canonical(&self.eligible_capability_order)
            || !canonical(&self.omitted_capability_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.model_system_order)
            || !canonical(&self.assay_class_order)
            || !canonical(&self.standard_order)
            || self.min_confidence_milli > 1_000
            || self
                .eligible_capability_order
                .iter()
                .any(|id| self.omitted_capability_order.binary_search(id).is_ok())
        {
            return Err(SiteCapabilityError::InvalidOutput(
                "envelope identity, ordering, bounds, or partition is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SiteCapabilityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SiteCapabilityError::InvalidOutput(
                "site capability digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &SiteCapabilityRequest) -> Result<(), SiteCapabilityError> {
    if !identifier(&request.site_id)
        || !identifier(&request.registry_version)
        || request.capabilities.is_empty()
        || request.capabilities.len() > MAX_CAPABILITIES
        || request.policy.now_epoch == 0
        || request.policy.min_confidence_milli > 1_000
    {
        return Err(SiteCapabilityError::InvalidRequest(
            "site identity, registry version, bounded capabilities, epoch, and confidence policy are required".into(),
        ));
    }
    for values in [
        &request.policy.required_model_systems,
        &request.policy.required_assay_classes,
        &request.policy.required_standards,
    ] {
        if !canonical(values) || values.iter().any(|value| !identifier(value)) {
            return Err(SiteCapabilityError::InvalidRequest(
                "required capability sets must be canonical and identifier-safe".into(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    for capability in &request.capabilities {
        if !identifier(&capability.capability_id)
            || !ids.insert(capability.capability_id.clone())
            || !identifier(&capability.model_system)
            || !identifier(&capability.assay_class)
            || !canonical(&capability.standard_order)
            || capability
                .standard_order
                .iter()
                .any(|value| !identifier(value))
            || !identifier(&capability.compute_class)
            || capability.confidence_milli > 1_000
            || capability.review_capacity == 0
            || capability.valid_until_epoch == 0
            || capability.attestation_digest.as_str().len() != 64
        {
            return Err(SiteCapabilityError::InvalidRequest(
                "capabilities require unique identity, bounded typed fields, review capacity, validity, and attestation".into(),
            ));
        }
    }
    Ok(())
}

/// Compile the site-local capability envelope used by an autonomous federation planner.
pub fn compile_glioma_site_capability_envelope(
    request: &SiteCapabilityRequest,
) -> Result<SiteCapabilityEnvelope, SiteCapabilityError> {
    validate_request(request)?;
    let required_models = request
        .policy
        .required_model_systems
        .iter()
        .collect::<BTreeSet<_>>();
    let required_assays = request
        .policy
        .required_assay_classes
        .iter()
        .collect::<BTreeSet<_>>();
    let required_standards = request
        .policy
        .required_standards
        .iter()
        .collect::<BTreeSet<_>>();
    let mut eligible = Vec::new();
    let mut omitted = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut assays = BTreeSet::new();
    let mut standards = BTreeSet::new();
    let mut total_review_capacity = 0_u32;
    let mut min_confidence = 1_000_u16;
    for capability in &request.capabilities {
        let mut reasons = Vec::new();
        if !capability.approved {
            reasons.push("not-approved".to_string());
        }
        if capability.valid_until_epoch < request.policy.now_epoch {
            reasons.push("expired".to_string());
        }
        if capability.confidence_milli < request.policy.min_confidence_milli {
            reasons.push("confidence-below-policy".to_string());
        }
        if request.policy.require_local_only && !capability.local_only {
            reasons.push("not-local-only".to_string());
        }
        if !required_models.is_empty() && !required_models.contains(&capability.model_system) {
            reasons.push("model-system-not-requested".to_string());
        }
        if !required_assays.is_empty() && !required_assays.contains(&capability.assay_class) {
            reasons.push("assay-class-not-requested".to_string());
        }
        if !required_standards.is_empty()
            && required_standards
                .difference(&capability.standard_order.iter().collect::<BTreeSet<_>>())
                .next()
                .is_some()
        {
            reasons.push("required-standard-missing".to_string());
        }
        if reasons.is_empty() {
            eligible.push(capability);
            models.insert(capability.model_system.clone());
            assays.insert(capability.assay_class.clone());
            standards.extend(capability.standard_order.iter().cloned());
            total_review_capacity =
                total_review_capacity.saturating_add(capability.review_capacity as u32);
            min_confidence = min_confidence.min(capability.confidence_milli);
        } else {
            omitted.insert(capability.capability_id.clone());
            for reason in reasons {
                omissions.insert(format!("{}:{reason}", capability.capability_id));
            }
        }
    }
    let disposition = if eligible.is_empty() {
        SiteCapabilityDisposition::Ineligible
    } else if omitted.is_empty() {
        SiteCapabilityDisposition::Eligible
    } else {
        SiteCapabilityDisposition::Partial
    };
    let mut output = SiteCapabilityEnvelope {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        site_id: request.site_id.clone(),
        registry_version: request.registry_version.clone(),
        eligible_capability_order: eligible
            .iter()
            .map(|capability| capability.capability_id.clone())
            .collect::<Vec<_>>(),
        omitted_capability_order: omitted.into_iter().collect(),
        omission_order: omissions.into_iter().collect(),
        model_system_order: models.into_iter().collect(),
        assay_class_order: assays.into_iter().collect(),
        standard_order: standards.into_iter().collect(),
        total_review_capacity,
        min_confidence_milli: if eligible.is_empty() {
            0
        } else {
            min_confidence
        },
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-site-capability-envelope"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| SiteCapabilityError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn capability(id: &str, model: &str, assay: &str) -> SiteCapabilityRecord {
        SiteCapabilityRecord {
            capability_id: id.into(),
            model_system: model.into(),
            assay_class: assay.into(),
            standard_order: vec!["ome-ngff-0.5".into()],
            compute_class: "gpu-local".into(),
            review_capacity: 4,
            confidence_milli: 900,
            valid_until_epoch: 110,
            approved: true,
            attestation_digest: hash(id),
            local_only: true,
        }
    }

    fn request() -> SiteCapabilityRequest {
        SiteCapabilityRequest {
            site_id: "site-a".into(),
            registry_version: "registry-1".into(),
            capabilities: vec![
                capability("cap-a", "organoid", "imaging"),
                capability("cap-b", "cell-line", "transcriptomics"),
            ],
            policy: SiteCapabilityPolicy {
                now_epoch: 100,
                required_model_systems: vec!["organoid".into()],
                required_assay_classes: vec!["imaging".into()],
                required_standards: vec!["ome-ngff-0.5".into()],
                min_confidence_milli: 800,
                require_local_only: true,
            },
        }
    }

    #[test]
    fn matching_capability_is_eligible() {
        let output = compile_glioma_site_capability_envelope(&request()).unwrap();
        assert_eq!(output.disposition, SiteCapabilityDisposition::Partial);
        assert_eq!(output.eligible_capability_order, vec!["cap-a"]);
        assert_eq!(output.total_review_capacity, 4);
        output.validate().unwrap();
    }

    #[test]
    fn expired_or_unapproved_capability_is_omitted() {
        let mut request = request();
        request.capabilities[0].valid_until_epoch = 99;
        request.capabilities[1].approved = false;
        let output = compile_glioma_site_capability_envelope(&request).unwrap();
        assert_eq!(output.disposition, SiteCapabilityDisposition::Ineligible);
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("expired")));
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("not-approved")));
    }

    #[test]
    fn policy_rejects_non_local_capability() {
        let mut request = request();
        request.capabilities[0].local_only = false;
        let output = compile_glioma_site_capability_envelope(&request).unwrap();
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("not-local-only")));
    }

    #[test]
    fn required_standard_is_enforced() {
        let mut request = request();
        request.capabilities[0].standard_order = vec!["other-standard".into()];
        let output = compile_glioma_site_capability_envelope(&request).unwrap();
        assert!(output
            .omission_order
            .iter()
            .any(|reason| reason.contains("required-standard-missing")));
    }

    #[test]
    fn envelope_mutation_breaks_digest() {
        let mut output = compile_glioma_site_capability_envelope(&request()).unwrap();
        output.model_system_order.push("tampered".into());
        assert!(matches!(
            output.validate(),
            Err(SiteCapabilityError::InvalidOutput(_))
        ));
    }
}
