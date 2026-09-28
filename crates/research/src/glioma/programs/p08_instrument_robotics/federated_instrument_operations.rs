//! Aggregate-only federated instrument operations for preclinical glioma research.
//!
//! Sites exchange only signed capability/conformance digests and privacy-bounded service-capacity
//! aggregates.  The exchange filters revoked, stale, unsafe, and small-cell sites before summing
//! capacity, so the consortium scheduler receives useful availability without credentials, raw
//! samples, device traces, or reconstructable site-level observations.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedInstrumentOperations1@1";
pub const MAX_SITES: usize = 512;
pub const MAX_TEXT_LEN: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentSiteSummary {
    pub site_id: String,
    pub capability_manifest_digest: ContentHash,
    pub protocol_conformance_digest: ContentHash,
    pub calibration_class: String,
    pub service_capacity_units: u64,
    pub available_capacity_units: u64,
    pub observed_tick: u64,
    pub expires_tick: u64,
    pub privacy_count: u32,
    pub revoked: bool,
    pub raw_data_excluded: bool,
    pub credentials_excluded: bool,
    pub approved_for_exchange: bool,
    pub summary_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentOperationsRequest {
    pub sites: Vec<FederatedInstrumentSiteSummary>,
    pub current_tick: u64,
    pub minimum_privacy_count: u32,
    pub maximum_staleness_ticks: u64,
    pub minimum_eligible_sites: usize,
    pub allow_metadata_exchange: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteOperationsState {
    Eligible,
    Stale,
    Revoked,
    PrivacyBlocked,
    Unsafe,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteOperationsResult {
    pub site_id: String,
    pub state: SiteOperationsState,
    pub observed_tick: u64,
    pub expires_tick: u64,
    pub reason_order: Vec<String>,
    pub result_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentOperationsSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub current_tick: u64,
    pub site_order: Vec<String>,
    pub results: Vec<SiteOperationsResult>,
    pub eligible_site_order: Vec<String>,
    pub excluded_site_order: Vec<String>,
    pub aggregate_service_capacity_units: u64,
    pub aggregate_available_capacity_units: u64,
    pub reconstruction_risk_milli: u16,
    pub sharing_permitted: bool,
    pub negative_evidence_order: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedInstrumentOperationsError {
    #[error("federated instrument operations request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated instrument operations output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated instrument operations digest failed: {0}")]
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

fn site_body(site: &FederatedInstrumentSiteSummary) -> serde_json::Value {
    serde_json::json!({
        "site_id": site.site_id,
        "capability_manifest_digest": site.capability_manifest_digest,
        "protocol_conformance_digest": site.protocol_conformance_digest,
        "calibration_class": site.calibration_class,
        "service_capacity_units": site.service_capacity_units,
        "available_capacity_units": site.available_capacity_units,
        "observed_tick": site.observed_tick,
        "expires_tick": site.expires_tick,
        "privacy_count": site.privacy_count,
        "revoked": site.revoked,
        "raw_data_excluded": site.raw_data_excluded,
        "credentials_excluded": site.credentials_excluded,
        "approved_for_exchange": site.approved_for_exchange,
    })
}

fn result_body(result: &SiteOperationsResult) -> serde_json::Value {
    serde_json::json!({
        "site_id": result.site_id,
        "state": result.state,
        "observed_tick": result.observed_tick,
        "expires_tick": result.expires_tick,
        "reason_order": result.reason_order,
    })
}

fn snapshot_body(snapshot: &FederatedInstrumentOperationsSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "current_tick": snapshot.current_tick,
        "site_order": snapshot.site_order,
        "results": snapshot.results,
        "eligible_site_order": snapshot.eligible_site_order,
        "excluded_site_order": snapshot.excluded_site_order,
        "aggregate_service_capacity_units": snapshot.aggregate_service_capacity_units,
        "aggregate_available_capacity_units": snapshot.aggregate_available_capacity_units,
        "reconstruction_risk_milli": snapshot.reconstruction_risk_milli,
        "sharing_permitted": snapshot.sharing_permitted,
        "negative_evidence_order": snapshot.negative_evidence_order,
    })
}

fn validate_request(
    request: &FederatedInstrumentOperationsRequest,
) -> Result<(), FederatedInstrumentOperationsError> {
    if request.sites.is_empty()
        || request.sites.len() > MAX_SITES
        || request.current_tick == 0
        || request.minimum_privacy_count == 0
        || request.maximum_staleness_ticks == 0
        || request.minimum_eligible_sites == 0
        || request.minimum_eligible_sites > request.sites.len()
    {
        return Err(FederatedInstrumentOperationsError::InvalidRequest(
            "bounded site set, current epoch, privacy floor, freshness bound, and eligible-site quorum are required".into(),
        ));
    }
    let mut previous = None;
    for site in &request.sites {
        if !safe_text(&site.site_id)
            || previous.is_some_and(|value: &str| value >= site.site_id.as_str())
            || !safe_text(&site.calibration_class)
            || site.service_capacity_units == 0
            || site.available_capacity_units > site.service_capacity_units
            || site.observed_tick == 0
            || site.expires_tick <= site.observed_tick
            || site.privacy_count == 0
            || !valid_hash(&site.capability_manifest_digest)
            || !valid_hash(&site.protocol_conformance_digest)
            || !valid_hash(&site.summary_digest)
        {
            return Err(FederatedInstrumentOperationsError::InvalidRequest(format!(
                "site {} has invalid identity, capacity, freshness, privacy, or digest bounds",
                site.site_id
            )));
        }
        let expected = ContentHash::of_value(&site_body(site))
            .map_err(|error| FederatedInstrumentOperationsError::Digest(error.to_string()))?;
        if expected != site.summary_digest {
            return Err(FederatedInstrumentOperationsError::InvalidRequest(format!(
                "site {} summary signature is tampered",
                site.site_id
            )));
        }
        previous = Some(site.site_id.as_str());
    }
    Ok(())
}

impl FederatedInstrumentOperationsSnapshot {
    pub fn validate(&self) -> Result<(), FederatedInstrumentOperationsError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.current_tick == 0
            || self.site_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .results
                .windows(2)
                .any(|pair| pair[0].site_id >= pair[1].site_id)
            || self
                .eligible_site_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .excluded_site_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.reconstruction_risk_milli > 1_000
        {
            return Err(FederatedInstrumentOperationsError::InvalidOutput(
                "snapshot identity, ordering, quorum, or reconstruction-risk bounds are invalid"
                    .into(),
            ));
        }
        for result in &self.results {
            let expected = ContentHash::of_value(&result_body(result))
                .map_err(|error| FederatedInstrumentOperationsError::Digest(error.to_string()))?;
            if expected != result.result_digest {
                return Err(FederatedInstrumentOperationsError::InvalidOutput(format!(
                    "site result digest is not bound for {}",
                    result.site_id
                )));
            }
        }
        let expected = ContentHash::of_value(&snapshot_body(self))
            .map_err(|error| FederatedInstrumentOperationsError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedInstrumentOperationsError::InvalidOutput(
                "snapshot digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

pub fn exchange_glioma_federated_instrument_operations(
    request: &FederatedInstrumentOperationsRequest,
) -> Result<FederatedInstrumentOperationsSnapshot, FederatedInstrumentOperationsError> {
    validate_request(request)?;
    let mut results = Vec::with_capacity(request.sites.len());
    let mut eligible_site_order = Vec::new();
    let mut excluded_site_order = Vec::new();
    let mut aggregate_service_capacity_units = 0u64;
    let mut aggregate_available_capacity_units = 0u64;
    let mut negative_evidence_order = Vec::new();
    for site in &request.sites {
        let mut reasons = Vec::new();
        let state =
            if !site.approved_for_exchange || !site.raw_data_excluded || !site.credentials_excluded
            {
                if !site.approved_for_exchange {
                    reasons.push("site-sharing-policy-denied".into());
                }
                if !site.raw_data_excluded {
                    reasons.push("raw-data-locality-proof-missing".into());
                }
                if !site.credentials_excluded {
                    reasons.push("credential-exclusion-proof-missing".into());
                }
                SiteOperationsState::Unsafe
            } else if site.revoked {
                reasons.push("site-membership-is-revoked".into());
                SiteOperationsState::Revoked
            } else if request.current_tick >= site.expires_tick
                || request.current_tick.saturating_sub(site.observed_tick)
                    > request.maximum_staleness_ticks
            {
                reasons.push("site-summary-is-stale-or-expired".into());
                SiteOperationsState::Stale
            } else if site.privacy_count < request.minimum_privacy_count {
                reasons.push("site-is-below-privacy-floor".into());
                SiteOperationsState::PrivacyBlocked
            } else {
                SiteOperationsState::Eligible
            };
        if state == SiteOperationsState::Eligible {
            eligible_site_order.push(site.site_id.clone());
            aggregate_service_capacity_units =
                aggregate_service_capacity_units.saturating_add(site.service_capacity_units);
            aggregate_available_capacity_units =
                aggregate_available_capacity_units.saturating_add(site.available_capacity_units);
        } else {
            excluded_site_order.push(site.site_id.clone());
            for reason in &reasons {
                negative_evidence_order.push(format!("{}:{}", site.site_id, reason));
            }
        }
        let mut result = SiteOperationsResult {
            site_id: site.site_id.clone(),
            state,
            observed_tick: site.observed_tick,
            expires_tick: site.expires_tick,
            reason_order: reasons,
            result_digest: ContentHash::of_bytes(b"unsealed-glioma-site-operation-result"),
        };
        result.result_digest = ContentHash::of_value(&result_body(&result))
            .map_err(|error| FederatedInstrumentOperationsError::Digest(error.to_string()))?;
        results.push(result);
    }
    let sharing_permitted = request.allow_metadata_exchange
        && eligible_site_order.len() >= request.minimum_eligible_sites;
    let reconstruction_risk_milli = if sharing_permitted {
        ((request.minimum_privacy_count as u64 * 1_000)
            / (eligible_site_order.len() as u64 + request.minimum_privacy_count as u64))
            .min(1_000) as u16
    } else {
        1_000
    };
    let snapshot = FederatedInstrumentOperationsSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        current_tick: request.current_tick,
        site_order: request
            .sites
            .iter()
            .map(|site| site.site_id.clone())
            .collect(),
        results,
        eligible_site_order,
        excluded_site_order,
        aggregate_service_capacity_units,
        aggregate_available_capacity_units,
        reconstruction_risk_milli,
        sharing_permitted,
        negative_evidence_order,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-operations"),
    };
    let mut sealed = snapshot;
    sealed.digest = ContentHash::of_value(&snapshot_body(&sealed))
        .map_err(|error| FederatedInstrumentOperationsError::Digest(error.to_string()))?;
    sealed.validate()?;
    Ok(sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn site(id: &str) -> FederatedInstrumentSiteSummary {
        let mut value = FederatedInstrumentSiteSummary {
            site_id: id.into(),
            capability_manifest_digest: hash("manifest"),
            protocol_conformance_digest: hash("protocol"),
            calibration_class: "cal-v2".into(),
            service_capacity_units: 100,
            available_capacity_units: 60,
            observed_tick: 90,
            expires_tick: 150,
            privacy_count: 8,
            revoked: false,
            raw_data_excluded: true,
            credentials_excluded: true,
            approved_for_exchange: true,
            summary_digest: hash("placeholder"),
        };
        value.summary_digest = ContentHash::of_value(&site_body(&value)).unwrap();
        value
    }

    fn request(sites: Vec<FederatedInstrumentSiteSummary>) -> FederatedInstrumentOperationsRequest {
        FederatedInstrumentOperationsRequest {
            sites,
            current_tick: 100,
            minimum_privacy_count: 5,
            maximum_staleness_ticks: 20,
            minimum_eligible_sites: 2,
            allow_metadata_exchange: true,
        }
    }

    #[test]
    fn eligible_sites_share_only_aggregate_capacity() {
        let output = exchange_glioma_federated_instrument_operations(&request(vec![
            site("site-a"),
            site("site-b"),
        ]))
        .unwrap();
        assert!(output.sharing_permitted);
        assert_eq!(output.aggregate_service_capacity_units, 200);
        assert_eq!(output.aggregate_available_capacity_units, 120);
        assert!(output.validate().is_ok());
    }

    #[test]
    fn stale_revoked_and_privacy_blocked_sites_are_excluded() {
        let mut stale = site("site-b");
        stale.observed_tick = 1;
        stale.summary_digest = ContentHash::of_value(&site_body(&stale)).unwrap();
        let mut revoked = site("site-c");
        revoked.revoked = true;
        revoked.summary_digest = ContentHash::of_value(&site_body(&revoked)).unwrap();
        let mut privacy = site("site-d");
        privacy.privacy_count = 1;
        privacy.summary_digest = ContentHash::of_value(&site_body(&privacy)).unwrap();
        let output = exchange_glioma_federated_instrument_operations(&request(vec![
            site("site-a"),
            stale,
            revoked,
            privacy,
        ]))
        .unwrap();
        assert_eq!(output.eligible_site_order, vec!["site-a"]);
        assert!(!output.sharing_permitted);
        assert_eq!(output.aggregate_available_capacity_units, 60);
        assert_eq!(output.excluded_site_order.len(), 3);
    }

    #[test]
    fn locality_and_policy_failures_are_not_silent() {
        let mut unsafe_site = site("site-a");
        unsafe_site.raw_data_excluded = false;
        unsafe_site.credentials_excluded = false;
        unsafe_site.approved_for_exchange = false;
        unsafe_site.summary_digest = ContentHash::of_value(&site_body(&unsafe_site)).unwrap();
        let output = exchange_glioma_federated_instrument_operations(&request(vec![
            unsafe_site,
            site("site-b"),
        ]))
        .unwrap();
        assert_eq!(output.results[0].state, SiteOperationsState::Unsafe);
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("raw-data")));
        assert!(output
            .negative_evidence_order
            .iter()
            .any(|item| item.contains("credential")));
    }

    #[test]
    fn tampered_site_summary_is_rejected() {
        let mut tampered = site("site-a");
        tampered.available_capacity_units = 99;
        assert!(matches!(
            exchange_glioma_federated_instrument_operations(&request(vec![tampered, site("site-b")])),
            Err(FederatedInstrumentOperationsError::InvalidRequest(message)) if message.contains("tampered")
        ));
    }

    #[test]
    fn metadata_exchange_can_be_denied_without_losing_local_status() {
        let mut input = request(vec![site("site-a"), site("site-b")]);
        input.allow_metadata_exchange = false;
        let output = exchange_glioma_federated_instrument_operations(&input).unwrap();
        assert!(!output.sharing_permitted);
        assert_eq!(output.eligible_site_order.len(), 2);
        assert_eq!(output.reconstruction_risk_milli, 1_000);
    }
}
