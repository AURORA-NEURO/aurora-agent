//! Federated device capability manifests for preclinical glioma research.
//!
//! A manifest is a schedulable, metadata-only declaration of what a local device can do.  The
//! public payload contains no credentials, raw sample identifiers, or instrument traces.  It is
//! signed by a content-bound attestation, expires independently from calibration, and is rejected
//! when revoked, stale, out of availability, or missing the explicit locality/exclusion proofs
//! required by the consortium scheduler.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedDeviceCapabilityManifest1@1";
pub const MAX_CAPABILITIES: usize = 256;
pub const MAX_WINDOWS: usize = 256;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCapabilityClaim {
    pub assay_id: String,
    pub protocol_version: String,
    pub max_parallel_units: u32,
    pub required_calibration_class: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailabilityWindow {
    pub start_tick: u64,
    pub end_tick: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataLocality {
    LocalOnly,
    AggregateMetadata,
    ApprovedArtifacts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDeviceCapabilityRequest {
    pub site_id: String,
    pub device_id: String,
    pub device_class: String,
    pub capabilities: Vec<DeviceCapabilityClaim>,
    pub availability: Vec<AvailabilityWindow>,
    pub calibration_valid_until_tick: u64,
    pub calibration_digest: ContentHash,
    pub attestation_digest: ContentHash,
    pub signing_key_id: String,
    pub signature_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub issued_tick: u64,
    pub expires_tick: u64,
    pub current_tick: u64,
    pub revoked: bool,
    pub revocation_digest: Option<ContentHash>,
    pub data_locality: DataLocality,
    pub secrets_excluded: bool,
    pub raw_sample_identifiers_excluded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestAvailabilityState {
    Available,
    Expired,
    CalibrationStale,
    Revoked,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCompatibility {
    pub assay_id: String,
    pub protocol_version: String,
    pub schedulable: bool,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDeviceCapabilityManifest {
    pub feature_id: String,
    pub output_schema: String,
    pub site_id: String,
    pub device_id: String,
    pub device_class: String,
    pub availability_state: ManifestAvailabilityState,
    pub issued_tick: u64,
    pub expires_tick: u64,
    pub calibration_valid_until_tick: u64,
    pub data_locality: DataLocality,
    pub capability_order: Vec<CapabilityCompatibility>,
    pub available_window_order: Vec<AvailabilityWindow>,
    pub reason_order: Vec<String>,
    pub schedulable: bool,
    pub federation_payload_digest: ContentHash,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedDeviceManifestError {
    #[error("federated device capability request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated device capability output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated device capability digest failed: {0}")]
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

fn canonical_capabilities(values: &[DeviceCapabilityClaim]) -> bool {
    values.windows(2).all(|pair| {
        (pair[0].assay_id.as_str(), pair[0].protocol_version.as_str())
            < (pair[1].assay_id.as_str(), pair[1].protocol_version.as_str())
    })
}

fn canonical_windows(values: &[AvailabilityWindow]) -> bool {
    values.windows(2).all(|pair| {
        pair[0].start_tick < pair[1].start_tick && pair[0].end_tick < pair[1].start_tick
    })
}

fn unsigned_body(request: &FederatedDeviceCapabilityRequest) -> serde_json::Value {
    serde_json::json!({
        "site_id": request.site_id,
        "device_id": request.device_id,
        "device_class": request.device_class,
        "capabilities": request.capabilities,
        "availability": request.availability,
        "calibration_valid_until_tick": request.calibration_valid_until_tick,
        "calibration_digest": request.calibration_digest,
        "attestation_digest": request.attestation_digest,
        "signing_key_id": request.signing_key_id,
        "policy_digest": request.policy_digest,
        "issued_tick": request.issued_tick,
        "expires_tick": request.expires_tick,
        "current_tick": request.current_tick,
        "revoked": request.revoked,
        "revocation_digest": request.revocation_digest,
        "data_locality": request.data_locality,
        "secrets_excluded": request.secrets_excluded,
        "raw_sample_identifiers_excluded": request.raw_sample_identifiers_excluded,
    })
}

fn payload_body(manifest: &FederatedDeviceCapabilityManifest) -> serde_json::Value {
    serde_json::json!({
        "feature_id": manifest.feature_id,
        "output_schema": manifest.output_schema,
        "site_id": manifest.site_id,
        "device_id": manifest.device_id,
        "device_class": manifest.device_class,
        "availability_state": manifest.availability_state,
        "issued_tick": manifest.issued_tick,
        "expires_tick": manifest.expires_tick,
        "calibration_valid_until_tick": manifest.calibration_valid_until_tick,
        "data_locality": manifest.data_locality,
        "capability_order": manifest.capability_order,
        "available_window_order": manifest.available_window_order,
        "reason_order": manifest.reason_order,
        "schedulable": manifest.schedulable,
    })
}

fn output_body(manifest: &FederatedDeviceCapabilityManifest) -> serde_json::Value {
    serde_json::json!({
        "feature_id": manifest.feature_id,
        "output_schema": manifest.output_schema,
        "site_id": manifest.site_id,
        "device_id": manifest.device_id,
        "device_class": manifest.device_class,
        "availability_state": manifest.availability_state,
        "issued_tick": manifest.issued_tick,
        "expires_tick": manifest.expires_tick,
        "calibration_valid_until_tick": manifest.calibration_valid_until_tick,
        "data_locality": manifest.data_locality,
        "capability_order": manifest.capability_order,
        "available_window_order": manifest.available_window_order,
        "reason_order": manifest.reason_order,
        "schedulable": manifest.schedulable,
        "federation_payload_digest": manifest.federation_payload_digest,
    })
}

fn validate_request(
    request: &FederatedDeviceCapabilityRequest,
) -> Result<(), FederatedDeviceManifestError> {
    if !safe_text(&request.site_id)
        || !safe_text(&request.device_id)
        || !safe_text(&request.device_class)
        || !safe_text(&request.signing_key_id)
        || request.capabilities.is_empty()
        || request.capabilities.len() > MAX_CAPABILITIES
        || request.availability.is_empty()
        || request.availability.len() > MAX_WINDOWS
        || !canonical_capabilities(&request.capabilities)
        || !canonical_windows(&request.availability)
        || request.calibration_valid_until_tick == 0
        || request.issued_tick == 0
        || request.expires_tick <= request.issued_tick
        || request.current_tick < request.issued_tick
        || !valid_hash(&request.calibration_digest)
        || !valid_hash(&request.attestation_digest)
        || !valid_hash(&request.signature_digest)
        || !valid_hash(&request.policy_digest)
        || (request.revoked && request.revocation_digest.is_none())
        || request
            .revocation_digest
            .as_ref()
            .is_some_and(|digest| !valid_hash(digest))
    {
        return Err(FederatedDeviceManifestError::InvalidRequest(
            "bounded identity, signed capability/availability claims, calibration, policy, expiry, and revocation fields are required".into(),
        ));
    }
    for capability in &request.capabilities {
        if !safe_text(&capability.assay_id)
            || !safe_text(&capability.protocol_version)
            || !safe_text(&capability.required_calibration_class)
            || capability.max_parallel_units == 0
        {
            return Err(FederatedDeviceManifestError::InvalidRequest(
                "capability claims require stable assay/protocol identity, calibration class, and positive capacity".into(),
            ));
        }
    }
    if request.availability.iter().any(|window| {
        window.start_tick == 0
            || window.end_tick <= window.start_tick
            || window.end_tick < request.current_tick.saturating_sub(1)
    }) {
        return Err(FederatedDeviceManifestError::InvalidRequest(
            "availability windows must be positive, ordered, and bounded around the current epoch"
                .into(),
        ));
    }
    let expected_signature = ContentHash::of_value(&unsigned_body(request))
        .map_err(|error| FederatedDeviceManifestError::Digest(error.to_string()))?;
    if expected_signature != request.signature_digest {
        return Err(FederatedDeviceManifestError::InvalidRequest(
            "manifest signature is tampered or not bound to its claims".into(),
        ));
    }
    Ok(())
}

impl FederatedDeviceCapabilityManifest {
    pub fn validate(&self) -> Result<(), FederatedDeviceManifestError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.site_id)
            || !safe_text(&self.device_id)
            || !safe_text(&self.device_class)
            || self.issued_tick == 0
            || self.expires_tick <= self.issued_tick
            || self.capability_order.windows(2).any(|pair| {
                (pair[0].assay_id.as_str(), pair[0].protocol_version.as_str())
                    >= (pair[1].assay_id.as_str(), pair[1].protocol_version.as_str())
            })
            || self.available_window_order.windows(2).any(|pair| {
                pair[0].start_tick >= pair[1].start_tick || pair[0].end_tick >= pair[1].start_tick
            })
            || !valid_hash(&self.federation_payload_digest)
        {
            return Err(FederatedDeviceManifestError::InvalidOutput(
                "manifest identity, bounds, canonical ordering, or federation digest is invalid"
                    .into(),
            ));
        }
        let expected_payload = ContentHash::of_value(&payload_body(self))
            .map_err(|error| FederatedDeviceManifestError::Digest(error.to_string()))?;
        if expected_payload != self.federation_payload_digest {
            return Err(FederatedDeviceManifestError::InvalidOutput(
                "federation payload digest is not content-bound".into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederatedDeviceManifestError::Digest(error.to_string()))?;
        if expected_digest != self.digest {
            return Err(FederatedDeviceManifestError::InvalidOutput(
                "manifest digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

pub fn publish_glioma_federated_device_capability_manifest(
    request: &FederatedDeviceCapabilityRequest,
) -> Result<FederatedDeviceCapabilityManifest, FederatedDeviceManifestError> {
    validate_request(request)?;
    let in_window = request.availability.iter().any(|window| {
        request.current_tick >= window.start_tick && request.current_tick < window.end_tick
    });
    let mut reasons = Vec::new();
    let state = if request.revoked {
        reasons.push("manifest-is-revoked".into());
        ManifestAvailabilityState::Revoked
    } else if request.current_tick >= request.expires_tick {
        reasons.push("manifest-is-expired".into());
        ManifestAvailabilityState::Expired
    } else if request.current_tick >= request.calibration_valid_until_tick {
        reasons.push("calibration-is-stale".into());
        ManifestAvailabilityState::CalibrationStale
    } else if !request.secrets_excluded || !request.raw_sample_identifiers_excluded {
        if !request.secrets_excluded {
            reasons.push("secret-exclusion-is-not-attested".into());
        }
        if !request.raw_sample_identifiers_excluded {
            reasons.push("raw-sample-identifier-exclusion-is-not-attested".into());
        }
        ManifestAvailabilityState::Blocked
    } else if !in_window {
        reasons.push("device-is-outside-declared-availability-window".into());
        ManifestAvailabilityState::Blocked
    } else {
        ManifestAvailabilityState::Available
    };
    let schedulable = state == ManifestAvailabilityState::Available;
    let capability_order = request
        .capabilities
        .iter()
        .map(|capability| CapabilityCompatibility {
            assay_id: capability.assay_id.clone(),
            protocol_version: capability.protocol_version.clone(),
            schedulable,
            reason_order: if schedulable {
                Vec::new()
            } else {
                reasons.clone()
            },
        })
        .collect::<Vec<_>>();
    let mut output = FederatedDeviceCapabilityManifest {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        site_id: request.site_id.clone(),
        device_id: request.device_id.clone(),
        device_class: request.device_class.clone(),
        availability_state: state,
        issued_tick: request.issued_tick,
        expires_tick: request.expires_tick,
        calibration_valid_until_tick: request.calibration_valid_until_tick,
        data_locality: request.data_locality,
        capability_order,
        available_window_order: request.availability.clone(),
        reason_order: reasons,
        schedulable,
        federation_payload_digest: ContentHash::of_bytes(b"unsealed-glioma-device-payload"),
        digest: ContentHash::of_bytes(b"unsealed-glioma-device-manifest"),
    };
    output.federation_payload_digest = ContentHash::of_value(&payload_body(&output))
        .map_err(|error| FederatedDeviceManifestError::Digest(error.to_string()))?;
    output.digest = ContentHash::of_value(&output_body(&output))
        .map_err(|error| FederatedDeviceManifestError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn request() -> FederatedDeviceCapabilityRequest {
        let mut output = FederatedDeviceCapabilityRequest {
            site_id: "site-a".into(),
            device_id: "device-a".into(),
            device_class: "high-content-imager".into(),
            capabilities: vec![DeviceCapabilityClaim {
                assay_id: "invasion-imaging".into(),
                protocol_version: "2026.1".into(),
                max_parallel_units: 2,
                required_calibration_class: "cal-v2".into(),
            }],
            availability: vec![AvailabilityWindow {
                start_tick: 10,
                end_tick: 100,
            }],
            calibration_valid_until_tick: 80,
            calibration_digest: hash("calibration"),
            attestation_digest: hash("attestation"),
            signing_key_id: "site-key-1".into(),
            signature_digest: hash("placeholder"),
            policy_digest: hash("policy"),
            issued_tick: 1,
            expires_tick: 90,
            current_tick: 20,
            revoked: false,
            revocation_digest: None,
            data_locality: DataLocality::AggregateMetadata,
            secrets_excluded: true,
            raw_sample_identifiers_excluded: true,
        };
        output.signature_digest = ContentHash::of_value(&unsigned_body(&output)).unwrap();
        output
    }

    #[test]
    fn available_manifest_is_schedulable_and_metadata_only() {
        let output = publish_glioma_federated_device_capability_manifest(&request()).unwrap();
        assert_eq!(
            output.availability_state,
            ManifestAvailabilityState::Available
        );
        assert!(output.schedulable);
        assert_eq!(output.data_locality, DataLocality::AggregateMetadata);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn signature_tampering_is_rejected_before_publication() {
        let mut input = request();
        input.device_class = "different-device".into();
        assert!(matches!(
            publish_glioma_federated_device_capability_manifest(&input),
            Err(FederatedDeviceManifestError::InvalidRequest(message)) if message.contains("tampered")
        ));
    }

    #[test]
    fn expiry_and_calibration_staleness_never_schedule() {
        let mut expired = request();
        expired.current_tick = expired.expires_tick;
        expired.signature_digest = ContentHash::of_value(&unsigned_body(&expired)).unwrap();
        let expired_output = publish_glioma_federated_device_capability_manifest(&expired).unwrap();
        assert_eq!(
            expired_output.availability_state,
            ManifestAvailabilityState::Expired
        );
        assert!(!expired_output.schedulable);
        let mut stale = request();
        stale.current_tick = stale.calibration_valid_until_tick;
        stale.signature_digest = ContentHash::of_value(&unsigned_body(&stale)).unwrap();
        let stale_output = publish_glioma_federated_device_capability_manifest(&stale).unwrap();
        assert_eq!(
            stale_output.availability_state,
            ManifestAvailabilityState::CalibrationStale
        );
        assert!(!stale_output.schedulable);
    }

    #[test]
    fn revocation_and_locality_exclusion_are_explicit() {
        let mut revoked = request();
        revoked.revoked = true;
        revoked.revocation_digest = Some(hash("revocation"));
        revoked.signature_digest = ContentHash::of_value(&unsigned_body(&revoked)).unwrap();
        let revoked_output = publish_glioma_federated_device_capability_manifest(&revoked).unwrap();
        assert_eq!(
            revoked_output.availability_state,
            ManifestAvailabilityState::Revoked
        );
        let mut unsafe_payload = request();
        unsafe_payload.raw_sample_identifiers_excluded = false;
        unsafe_payload.signature_digest =
            ContentHash::of_value(&unsigned_body(&unsafe_payload)).unwrap();
        let unsafe_output =
            publish_glioma_federated_device_capability_manifest(&unsafe_payload).unwrap();
        assert_eq!(
            unsafe_output.availability_state,
            ManifestAvailabilityState::Blocked
        );
        assert!(unsafe_output
            .reason_order
            .iter()
            .any(|reason| reason.contains("raw-sample")));
    }

    #[test]
    fn outside_availability_window_is_blocked_without_disclosing_raw_data() {
        let mut input = request();
        input.current_tick = 5;
        input.signature_digest = ContentHash::of_value(&unsigned_body(&input)).unwrap();
        let output = publish_glioma_federated_device_capability_manifest(&input).unwrap();
        assert_eq!(
            output.availability_state,
            ManifestAvailabilityState::Blocked
        );
        assert!(!output.schedulable);
        let encoded = serde_json::to_string(&output).unwrap();
        assert!(!encoded.contains("sample_id"));
        assert!(!encoded.contains("credential"));
    }
}
