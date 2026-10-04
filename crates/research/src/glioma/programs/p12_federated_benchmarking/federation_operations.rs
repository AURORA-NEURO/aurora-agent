//! Consortium federation operations exchange for aggregate-only glioma research.
//!
//! This feature gives the research director a deterministic view of gateway availability,
//! standards compatibility, heartbeat freshness, maintenance, incidents, and policy-bounded
//! failover candidates. It exchanges operational metadata only: research payloads, credentials,
//! and local policy decisions never cross this boundary.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaFederationOperationsSnapshot1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_STANDARDS: usize = 64;
pub const MAX_INCIDENTS: usize = 128;
pub const MAX_TEXT: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationSiteOperationalState {
    Ready,
    Degraded,
    Offline,
    Maintenance,
    Revoked,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederationOperationalFindingKind {
    StaleHeartbeat,
    StandardsMismatch,
    RevokedSite,
    MaintenanceWindow,
    ServiceUnavailable,
    OpenIncident,
    QueuePressure,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationSiteOperationsInput {
    pub site_id: String,
    pub capability_manifest_digest: ContentHash,
    pub standards_order: Vec<String>,
    pub service_available: bool,
    pub heartbeat_tick: u64,
    pub revoked: bool,
    pub maintenance: bool,
    pub local_policy_failover_allowed: bool,
    pub open_incident_order: Vec<String>,
    pub incident_severity_milli: u16,
    pub queue_depth: u32,
    pub queue_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationOperationsRequest {
    pub federation_id: String,
    pub current_tick: u64,
    pub heartbeat_max_age_ticks: u64,
    pub queue_pressure_threshold_milli: u16,
    pub required_standard_order: Vec<String>,
    pub sites: Vec<FederationSiteOperationsInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationSiteOperationsView {
    pub site_id: String,
    pub state: FederationSiteOperationalState,
    pub heartbeat_age_ticks: Option<u64>,
    pub standards_gap_order: Vec<String>,
    pub finding_order: Vec<FederationOperationalFindingKind>,
    pub open_incident_order: Vec<String>,
    pub queue_pressure_milli: u16,
    pub local_policy_failover_allowed: bool,
    pub capability_manifest_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationOperationsSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub federation_id: String,
    pub site_order: Vec<String>,
    pub sites: Vec<FederationSiteOperationsView>,
    pub ready_site_order: Vec<String>,
    pub degraded_site_order: Vec<String>,
    pub offline_site_order: Vec<String>,
    pub maintenance_site_order: Vec<String>,
    pub revoked_site_order: Vec<String>,
    pub incompatible_site_order: Vec<String>,
    pub failover_candidate_order: Vec<String>,
    pub standards_gap_order: Vec<String>,
    pub incident_order: Vec<String>,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
    pub local_payloads_exchanged: bool,
    pub credentials_exchanged: bool,
    pub failover_requires_local_policy: bool,
    pub dispatch_permitted: bool,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederationOperationsError {
    #[error("federation operations request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federation operations output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federation operations digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= MAX_TEXT && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(snapshot: &FederationOperationsSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "federation_id": snapshot.federation_id,
        "site_order": snapshot.site_order,
        "sites": snapshot.sites,
        "ready_site_order": snapshot.ready_site_order,
        "degraded_site_order": snapshot.degraded_site_order,
        "offline_site_order": snapshot.offline_site_order,
        "maintenance_site_order": snapshot.maintenance_site_order,
        "revoked_site_order": snapshot.revoked_site_order,
        "incompatible_site_order": snapshot.incompatible_site_order,
        "failover_candidate_order": snapshot.failover_candidate_order,
        "standards_gap_order": snapshot.standards_gap_order,
        "incident_order": snapshot.incident_order,
        "blockers": snapshot.blockers,
        "warnings": snapshot.warnings,
        "local_payloads_exchanged": snapshot.local_payloads_exchanged,
        "credentials_exchanged": snapshot.credentials_exchanged,
        "failover_requires_local_policy": snapshot.failover_requires_local_policy,
        "dispatch_permitted": snapshot.dispatch_permitted,
    })
}

fn validate_request(
    request: &FederationOperationsRequest,
) -> Result<(), FederationOperationsError> {
    if !safe_text(&request.federation_id)
        || request.heartbeat_max_age_ticks == 0
        || request.queue_pressure_threshold_milli > 1_000
        || request.required_standard_order.is_empty()
        || request.required_standard_order.len() > MAX_STANDARDS
        || !canonical(&request.required_standard_order)
        || request.sites.is_empty()
        || request.sites.len() > MAX_SITES
    {
        return Err(FederationOperationsError::InvalidRequest(
            "bounded federation, heartbeat, pressure, canonical standards, and site inputs are required".into(),
        ));
    }
    let mut sites = BTreeSet::new();
    for site in &request.sites {
        if !safe_text(&site.site_id)
            || !sites.insert(site.site_id.clone())
            || site.capability_manifest_digest.as_str().len() != 64
            || !canonical(&site.standards_order)
            || site.standards_order.len() > MAX_STANDARDS
            || !canonical(&site.open_incident_order)
            || site.open_incident_order.len() > MAX_INCIDENTS
            || site.incident_severity_milli > 1_000
            || site.queue_capacity == 0
            || site.queue_depth > site.queue_capacity
        {
            return Err(FederationOperationsError::InvalidRequest(format!(
                "site {} has invalid identity, standards, incidents, severity, queue, or digest fields",
                site.site_id
            )));
        }
    }
    Ok(())
}

impl FederationOperationsSnapshot {
    pub fn validate(&self) -> Result<(), FederationOperationsError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.federation_id)
            || !canonical(&self.site_order)
            || !canonical(&self.ready_site_order)
            || !canonical(&self.degraded_site_order)
            || !canonical(&self.offline_site_order)
            || !canonical(&self.maintenance_site_order)
            || !canonical(&self.revoked_site_order)
            || !canonical(&self.incompatible_site_order)
            || !canonical(&self.failover_candidate_order)
            || !canonical(&self.standards_gap_order)
            || !canonical(&self.incident_order)
            || !canonical(&self.blockers)
            || !canonical(&self.warnings)
            || self.digest.as_str().len() != 64
            || self.local_payloads_exchanged
            || self.credentials_exchanged
            || !self.failover_requires_local_policy
            || self.dispatch_permitted
        {
            return Err(FederationOperationsError::InvalidOutput(
                "operations identity, canonical ordering, locality, policy, dispatch, or digest invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| FederationOperationsError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederationOperationsError::InvalidOutput(
                "federation operations digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Build a deterministic consortium operations snapshot from bounded site metadata.
pub fn build_glioma_federation_operations_snapshot(
    request: &FederationOperationsRequest,
) -> Result<FederationOperationsSnapshot, FederationOperationsError> {
    validate_request(request)?;
    let mut sites = request.sites.clone();
    sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let required_standards = request
        .required_standard_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut views = Vec::with_capacity(sites.len());
    let mut ready = BTreeSet::new();
    let mut degraded = BTreeSet::new();
    let mut offline = BTreeSet::new();
    let mut maintenance = BTreeSet::new();
    let mut revoked = BTreeSet::new();
    let mut incompatible = BTreeSet::new();
    let mut failover = BTreeSet::new();
    let mut standards_gaps = BTreeSet::new();
    let mut incidents = BTreeSet::new();
    let mut blockers = Vec::new();
    let mut warnings = Vec::new();

    for site in &sites {
        let provided = site
            .standards_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let gaps = required_standards
            .difference(&provided)
            .cloned()
            .collect::<Vec<_>>();
        standards_gaps.extend(
            gaps.iter()
                .map(|standard| format!("{}:{standard}", site.site_id)),
        );
        incidents.extend(
            site.open_incident_order
                .iter()
                .map(|incident| format!("{}:{incident}", site.site_id)),
        );
        let heartbeat_age = if site.heartbeat_tick > request.current_tick {
            None
        } else {
            Some(request.current_tick - site.heartbeat_tick)
        };
        let stale = heartbeat_age.is_none_or(|age| age > request.heartbeat_max_age_ticks);
        let pressure = ((u64::from(site.queue_depth) * 1_000) / u64::from(site.queue_capacity))
            .min(1_000) as u16;
        let pressure_high = pressure >= request.queue_pressure_threshold_milli;
        let mut findings = Vec::new();
        if stale {
            findings.push(FederationOperationalFindingKind::StaleHeartbeat);
        }
        if !gaps.is_empty() {
            findings.push(FederationOperationalFindingKind::StandardsMismatch);
        }
        if site.revoked {
            findings.push(FederationOperationalFindingKind::RevokedSite);
        }
        if site.maintenance {
            findings.push(FederationOperationalFindingKind::MaintenanceWindow);
        }
        if !site.service_available {
            findings.push(FederationOperationalFindingKind::ServiceUnavailable);
        }
        if !site.open_incident_order.is_empty() {
            findings.push(FederationOperationalFindingKind::OpenIncident);
        }
        if pressure_high {
            findings.push(FederationOperationalFindingKind::QueuePressure);
        }
        findings.sort();
        let state = if site.revoked {
            revoked.insert(site.site_id.clone());
            FederationSiteOperationalState::Revoked
        } else if !gaps.is_empty() {
            incompatible.insert(site.site_id.clone());
            FederationSiteOperationalState::Incompatible
        } else if site.maintenance {
            maintenance.insert(site.site_id.clone());
            FederationSiteOperationalState::Maintenance
        } else if !site.service_available || heartbeat_age.is_none() {
            offline.insert(site.site_id.clone());
            FederationSiteOperationalState::Offline
        } else if stale || pressure_high || site.incident_severity_milli >= 800 {
            degraded.insert(site.site_id.clone());
            FederationSiteOperationalState::Degraded
        } else {
            ready.insert(site.site_id.clone());
            if site.local_policy_failover_allowed {
                failover.insert(site.site_id.clone());
            }
            FederationSiteOperationalState::Ready
        };
        if state != FederationSiteOperationalState::Ready && !site.local_policy_failover_allowed {
            warnings.push(format!(
                "{} cannot be considered for failover without local policy permission",
                site.site_id
            ));
        }
        views.push(FederationSiteOperationsView {
            site_id: site.site_id.clone(),
            state,
            heartbeat_age_ticks: heartbeat_age,
            standards_gap_order: gaps,
            finding_order: findings,
            open_incident_order: site.open_incident_order.clone(),
            queue_pressure_milli: pressure,
            local_policy_failover_allowed: site.local_policy_failover_allowed,
            capability_manifest_digest: site.capability_manifest_digest.clone(),
        });
    }
    if ready.is_empty() {
        blockers.push("no ready compatible site is available for a new federated action".into());
    }
    if !incompatible.is_empty() {
        blockers.push(format!(
            "{} site(s) are incompatible with the required standards",
            incompatible.len()
        ));
    }
    if !offline.is_empty() {
        warnings.push(format!(
            "{} site(s) are offline or lack a current heartbeat",
            offline.len()
        ));
    }
    blockers.sort();
    warnings.sort();
    let mut snapshot = FederationOperationsSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        federation_id: request.federation_id.clone(),
        site_order: sites.iter().map(|site| site.site_id.clone()).collect(),
        sites: views,
        ready_site_order: ready.into_iter().collect(),
        degraded_site_order: degraded.into_iter().collect(),
        offline_site_order: offline.into_iter().collect(),
        maintenance_site_order: maintenance.into_iter().collect(),
        revoked_site_order: revoked.into_iter().collect(),
        incompatible_site_order: incompatible.into_iter().collect(),
        failover_candidate_order: failover.into_iter().collect(),
        standards_gap_order: standards_gaps.into_iter().collect(),
        incident_order: incidents.into_iter().collect(),
        blockers,
        warnings,
        local_payloads_exchanged: false,
        credentials_exchanged: false,
        failover_requires_local_policy: true,
        dispatch_permitted: false,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federation-operations"),
    };
    snapshot.digest = ContentHash::of_value(&digest_body(&snapshot))
        .map_err(|error| FederationOperationsError::Digest(error.to_string()))?;
    snapshot.validate()?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn site(id: &str) -> FederationSiteOperationsInput {
        FederationSiteOperationsInput {
            site_id: id.into(),
            capability_manifest_digest: digest(&format!("manifest-{id}")),
            standards_order: vec!["cwl-1.2".into(), "prov-o-2013".into()],
            service_available: true,
            heartbeat_tick: 95,
            revoked: false,
            maintenance: false,
            local_policy_failover_allowed: true,
            open_incident_order: Vec::new(),
            incident_severity_milli: 0,
            queue_depth: 10,
            queue_capacity: 100,
        }
    }

    fn request(sites: Vec<FederationSiteOperationsInput>) -> FederationOperationsRequest {
        FederationOperationsRequest {
            federation_id: "consortium-1".into(),
            current_tick: 100,
            heartbeat_max_age_ticks: 10,
            queue_pressure_threshold_milli: 800,
            required_standard_order: vec!["cwl-1.2".into(), "prov-o-2013".into()],
            sites,
        }
    }

    #[test]
    fn ready_sites_are_operational_failover_candidates() {
        let snapshot =
            build_glioma_federation_operations_snapshot(&request(vec![site("site-a")])).unwrap();
        assert_eq!(snapshot.ready_site_order, vec!["site-a"]);
        assert_eq!(snapshot.failover_candidate_order, vec!["site-a"]);
        assert!(!snapshot.local_payloads_exchanged);
        assert!(!snapshot.dispatch_permitted);
    }

    #[test]
    fn revoked_maintenance_and_incompatible_sites_are_partitioned() {
        let mut revoked = site("site-a");
        revoked.revoked = true;
        let mut maintenance = site("site-b");
        maintenance.maintenance = true;
        let mut incompatible = site("site-c");
        incompatible.standards_order = vec!["cwl-1.2".into()];
        let snapshot = build_glioma_federation_operations_snapshot(&request(vec![
            revoked,
            maintenance,
            incompatible,
        ]))
        .unwrap();
        assert_eq!(snapshot.revoked_site_order, vec!["site-a"]);
        assert_eq!(snapshot.maintenance_site_order, vec!["site-b"]);
        assert_eq!(snapshot.incompatible_site_order, vec!["site-c"]);
        assert!(!snapshot.blockers.is_empty());
    }

    #[test]
    fn stale_heartbeats_and_queue_pressure_are_degraded() {
        let mut stale = site("site-a");
        stale.heartbeat_tick = 50;
        let mut pressure = site("site-b");
        pressure.queue_depth = 90;
        let snapshot =
            build_glioma_federation_operations_snapshot(&request(vec![stale, pressure])).unwrap();
        assert_eq!(snapshot.degraded_site_order, vec!["site-a", "site-b"]);
        assert!(snapshot
            .sites
            .iter()
            .all(|view| !view.finding_order.is_empty()));
    }

    #[test]
    fn incident_and_failover_policy_remain_visible() {
        let mut incident = site("site-a");
        incident.open_incident_order = vec!["gateway-latency".into()];
        incident.incident_severity_milli = 900;
        incident.local_policy_failover_allowed = false;
        let snapshot =
            build_glioma_federation_operations_snapshot(&request(vec![incident])).unwrap();
        assert_eq!(snapshot.degraded_site_order, vec!["site-a"]);
        assert_eq!(snapshot.incident_order, vec!["site-a:gateway-latency"]);
        assert!(snapshot
            .warnings
            .iter()
            .any(|warning| warning.contains("local policy")));
        assert!(snapshot.failover_candidate_order.is_empty());
    }

    #[test]
    fn replay_is_content_deterministic() {
        let req = request(vec![site("site-b"), site("site-a")]);
        let first = build_glioma_federation_operations_snapshot(&req).unwrap();
        let second = build_glioma_federation_operations_snapshot(&req).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.digest, second.digest);
    }
}
