//! Federated aggregate-only instrument endpoint consensus for preclinical glioma research.
//!
//! Stable local endpoints still need cross-site comparison before a consortium promotes them to a
//! shared research surface. This feature consumes only site-level summaries, applies privacy and
//! federation permission gates, computes inverse-uncertainty weighted consensus, and reports
//! heterogeneity, leave-one-site-out sensitivity, and maximum site influence. Raw traces, samples,
//! human data, and instrument effects remain local; this route never executes hardware.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P08-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedInstrumentConsensus1@1";
pub const MAX_SITES: usize = 512;
pub const MAX_ENDPOINTS: usize = 4_096;
pub const SCORE_SCALE: u64 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentConsensusRequest {
    pub objective: String,
    pub instrument_family: String,
    pub min_sites: usize,
    pub min_privacy_count: u32,
    pub min_endpoint_coverage_milli: u16,
    pub max_heterogeneity_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub max_site_influence_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEndpointValue {
    pub endpoint_id: String,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub coverage_milli: u16,
    pub permitted_for_federation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentSite {
    pub site_id: String,
    pub instrument_family: String,
    pub privacy_count: u32,
    pub endpoints: Vec<FederatedEndpointValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEndpointDisposition {
    Qualified,
    PrivacyBlocked,
    CoverageBlocked,
    HeterogeneityBlocked,
    InfluenceBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEndpointConsensus {
    pub endpoint_id: String,
    pub eligible_site_order: Vec<String>,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub heterogeneity_milli: u64,
    pub leave_one_out_shift_milli: u64,
    pub max_site_influence_milli: u16,
    pub disposition: FederatedEndpointDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedConsensusDisposition {
    Qualified,
    Partial,
    NoEligibleSites,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInstrumentConsensus {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub instrument_family: String,
    pub site_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub endpoints: Vec<FederatedEndpointConsensus>,
    pub qualified_endpoint_order: Vec<String>,
    pub blocked_endpoint_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedConsensusDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedConsensusError {
    #[error("federated instrument consensus request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated instrument site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated instrument consensus output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated instrument consensus digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedInstrumentConsensus) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "instrument_family": output.instrument_family,
        "site_order": output.site_order,
        "endpoint_order": output.endpoint_order,
        "endpoints": output.endpoints,
        "qualified_endpoint_order": output.qualified_endpoint_order,
        "blocked_endpoint_order": output.blocked_endpoint_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &FederatedInstrumentConsensusRequest,
) -> Result<(), FederatedConsensusError> {
    if request.objective.trim().is_empty()
        || request.instrument_family.trim().is_empty()
        || request.min_sites < 2
        || request.min_sites > MAX_SITES
        || request.min_privacy_count == 0
        || request.min_endpoint_coverage_milli == 0
        || request.min_endpoint_coverage_milli > SCORE_SCALE as u16
        || request.max_heterogeneity_milli == 0
        || request.max_leave_one_out_shift_milli == 0
        || request.max_site_influence_milli == 0
        || request.max_site_influence_milli > SCORE_SCALE as u16
    {
        return Err(FederatedConsensusError::InvalidRequest(
            "objective, instrument family, site/privacy floors, coverage, heterogeneity, influence, and leave-one-out gates are required".into(),
        ));
    }
    Ok(())
}

fn validate_sites(
    request: &FederatedInstrumentConsensusRequest,
    sites: &[FederatedInstrumentSite],
) -> Result<(), FederatedConsensusError> {
    if sites.len() < request.min_sites || sites.len() > MAX_SITES {
        return Err(FederatedConsensusError::InvalidSite(
            "site count does not meet the configured bounded minimum or maximum".into(),
        ));
    }
    let mut site_ids = BTreeSet::new();
    for site in sites {
        if site.site_id.trim().is_empty()
            || !site_ids.insert(site.site_id.clone())
            || site.instrument_family != request.instrument_family
            || site.endpoints.is_empty()
            || site.endpoints.len() > MAX_ENDPOINTS
        {
            return Err(FederatedConsensusError::InvalidSite(
                "site identity, family binding, bounded non-empty endpoints, and uniqueness are required".into(),
            ));
        }
        let mut endpoint_ids = BTreeSet::new();
        for endpoint in &site.endpoints {
            if endpoint.endpoint_id.trim().is_empty()
                || !endpoint_ids.insert(endpoint.endpoint_id.clone())
                || endpoint.uncertainty_milli == 0
                || endpoint.coverage_milli > SCORE_SCALE as u16
            {
                return Err(FederatedConsensusError::InvalidSite(
                    "endpoint identity, positive uncertainty, coverage bounds, and uniqueness are required".into(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_output(output: &FederatedInstrumentConsensus) -> Result<(), FederatedConsensusError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.instrument_family.trim().is_empty()
        || !canonical(&output.site_order)
        || !canonical(&output.endpoint_order)
        || !canonical(&output.qualified_endpoint_order)
        || !canonical(&output.blocked_endpoint_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .endpoints
            .windows(2)
            .any(|pair| pair[0].endpoint_id >= pair[1].endpoint_id)
        || output.endpoints.iter().any(|endpoint| {
            endpoint.endpoint_id.trim().is_empty()
                || !canonical(&endpoint.eligible_site_order)
                || endpoint.max_site_influence_milli > SCORE_SCALE as u16
        })
    {
        return Err(FederatedConsensusError::InvalidOutput(
            "identity, canonical ordering, influence, or endpoint invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| FederatedConsensusError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(FederatedConsensusError::InvalidOutput(
            "digest is not bound to federated instrument consensus".into(),
        ));
    }
    Ok(())
}

impl FederatedInstrumentConsensus {
    pub fn validate(&self) -> Result<(), FederatedConsensusError> {
        validate_output(self)
    }
}

fn weighted_mean(values: &[(i64, u64)]) -> i64 {
    let total_weight = values.iter().map(|(_, weight)| *weight).sum::<u64>();
    if total_weight == 0 {
        return 0;
    }
    let weighted = values
        .iter()
        .map(|(value, weight)| i128::from(*value) * i128::from(*weight))
        .sum::<i128>();
    (weighted / i128::from(total_weight)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn endpoint_consensus(
    request: &FederatedInstrumentConsensusRequest,
    endpoint_id: &str,
    sites: &[&FederatedInstrumentSite],
) -> FederatedEndpointConsensus {
    let mut eligible = sites
        .iter()
        .filter_map(|site| {
            if site.privacy_count < request.min_privacy_count {
                return None;
            }
            let endpoint = site
                .endpoints
                .iter()
                .find(|endpoint| endpoint.endpoint_id == endpoint_id)?;
            if !endpoint.permitted_for_federation
                || endpoint.coverage_milli < request.min_endpoint_coverage_milli
            {
                return None;
            }
            Some((
                site.site_id.clone(),
                endpoint.effect_milli,
                endpoint.uncertainty_milli,
            ))
        })
        .collect::<Vec<_>>();
    eligible.sort_by(|left, right| left.0.cmp(&right.0));
    let eligible_site_order = eligible
        .iter()
        .map(|(site_id, _, _)| site_id.clone())
        .collect::<Vec<_>>();
    if eligible.is_empty() {
        return FederatedEndpointConsensus {
            endpoint_id: endpoint_id.into(),
            eligible_site_order,
            effect_milli: 0,
            uncertainty_milli: 0,
            heterogeneity_milli: 0,
            leave_one_out_shift_milli: 0,
            max_site_influence_milli: 0,
            disposition: FederatedEndpointDisposition::PrivacyBlocked,
        };
    }
    let values = eligible
        .iter()
        .map(|(_, effect, uncertainty)| (*effect, 1_000_000_u64 / uncertainty.saturating_add(1)))
        .collect::<Vec<_>>();
    let total_weight = values.iter().map(|(_, weight)| *weight).sum::<u64>().max(1);
    let effect_milli = weighted_mean(&values);
    let heterogeneity_milli = eligible
        .iter()
        .map(|(_, effect, _)| effect.abs_diff(effect_milli))
        .max()
        .unwrap_or(0);
    let mut leave_one_out_shift_milli = 0_u64;
    for index in 0..values.len() {
        if values.len() <= 2 {
            break;
        }
        let subset = values
            .iter()
            .enumerate()
            .filter(|(candidate, _)| *candidate != index)
            .map(|(_, value)| *value)
            .collect::<Vec<_>>();
        leave_one_out_shift_milli =
            leave_one_out_shift_milli.max(weighted_mean(&subset).abs_diff(effect_milli));
    }
    let max_site_influence_milli = values
        .iter()
        .map(|(_, weight)| weight.saturating_mul(SCORE_SCALE) / total_weight)
        .max()
        .unwrap_or(0) as u16;
    let uncertainty_milli = (1_000_000_u64 / total_weight.max(1)).min(u64::MAX / 2);
    let disposition = if eligible.len() < request.min_sites {
        FederatedEndpointDisposition::Unresolved
    } else if heterogeneity_milli > request.max_heterogeneity_milli {
        FederatedEndpointDisposition::HeterogeneityBlocked
    } else if leave_one_out_shift_milli > request.max_leave_one_out_shift_milli {
        FederatedEndpointDisposition::InfluenceBlocked
    } else if max_site_influence_milli > request.max_site_influence_milli {
        FederatedEndpointDisposition::InfluenceBlocked
    } else {
        FederatedEndpointDisposition::Qualified
    };
    FederatedEndpointConsensus {
        endpoint_id: endpoint_id.into(),
        eligible_site_order,
        effect_milli,
        uncertainty_milli,
        heterogeneity_milli,
        leave_one_out_shift_milli,
        max_site_influence_milli,
        disposition,
    }
}

/// Compute aggregate-only, privacy-gated instrument endpoint consensus across sites.
pub fn analyze_glioma_federated_instrument_consensus(
    request: &FederatedInstrumentConsensusRequest,
    sites: &[FederatedInstrumentSite],
) -> Result<FederatedInstrumentConsensus, FederatedConsensusError> {
    validate_request(request)?;
    validate_sites(request, sites)?;
    let mut ordered_sites = sites.iter().collect::<Vec<_>>();
    ordered_sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let site_order = ordered_sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let mut endpoint_ids = BTreeSet::new();
    for site in &ordered_sites {
        endpoint_ids.extend(
            site.endpoints
                .iter()
                .map(|endpoint| endpoint.endpoint_id.clone()),
        );
    }
    let endpoint_order = endpoint_ids.iter().cloned().collect::<Vec<_>>();
    let endpoints = endpoint_order
        .iter()
        .map(|endpoint_id| endpoint_consensus(request, endpoint_id, &ordered_sites))
        .collect::<Vec<_>>();
    let qualified_endpoint_order = endpoints
        .iter()
        .filter(|endpoint| endpoint.disposition == FederatedEndpointDisposition::Qualified)
        .map(|endpoint| endpoint.endpoint_id.clone())
        .collect::<Vec<_>>();
    let blocked_endpoint_order = endpoints
        .iter()
        .filter(|endpoint| endpoint.disposition != FederatedEndpointDisposition::Qualified)
        .map(|endpoint| endpoint.endpoint_id.clone())
        .collect::<Vec<_>>();
    let mut negative_evidence = endpoints
        .iter()
        .filter(|endpoint| endpoint.disposition != FederatedEndpointDisposition::Qualified)
        .map(|endpoint| {
            format!("{:?}:{}", endpoint.disposition, endpoint.endpoint_id).to_lowercase()
        })
        .collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = Vec::new();
    if !blocked_endpoint_order.is_empty() {
        uncertainty
            .push("one-or-more-endpoints-failed-federated-privacy-or-heterogeneity-gates".into());
    }
    if ordered_sites.len() < request.min_sites {
        uncertainty.push("federated-site-count-is-underpowered-for-configured-minimum".into());
    }
    uncertainty.sort();
    let disposition =
        if qualified_endpoint_order.len() == endpoint_order.len() && !endpoint_order.is_empty() {
            FederatedConsensusDisposition::Qualified
        } else if !qualified_endpoint_order.is_empty() {
            FederatedConsensusDisposition::Partial
        } else if ordered_sites
            .iter()
            .all(|site| site.privacy_count < request.min_privacy_count)
        {
            FederatedConsensusDisposition::NoEligibleSites
        } else {
            FederatedConsensusDisposition::Unresolved
        };
    let mut output = FederatedInstrumentConsensus {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        instrument_family: request.instrument_family.clone(),
        site_order,
        endpoint_order,
        endpoints,
        qualified_endpoint_order,
        blocked_endpoint_order,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedConsensusError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(site_id: &str, effect_milli: i64, uncertainty_milli: u64) -> FederatedInstrumentSite {
        FederatedInstrumentSite {
            site_id: site_id.into(),
            instrument_family: "imaging".into(),
            privacy_count: 12,
            endpoints: vec![FederatedEndpointValue {
                endpoint_id: "invasion-edge".into(),
                effect_milli,
                uncertainty_milli,
                coverage_milli: 950,
                permitted_for_federation: true,
            }],
        }
    }

    fn request() -> FederatedInstrumentConsensusRequest {
        FederatedInstrumentConsensusRequest {
            objective: "transport stable endpoint".into(),
            instrument_family: "imaging".into(),
            min_sites: 3,
            min_privacy_count: 5,
            min_endpoint_coverage_milli: 800,
            max_heterogeneity_milli: 100,
            max_leave_one_out_shift_milli: 80,
            max_site_influence_milli: 500,
        }
    }

    #[test]
    fn qualifies_replayable_consensus() {
        let sites = vec![
            site("site-a", 500, 40),
            site("site-b", 520, 45),
            site("site-c", 510, 50),
        ];
        let first =
            analyze_glioma_federated_instrument_consensus(&request(), &sites).expect("consensus");
        let second =
            analyze_glioma_federated_instrument_consensus(&request(), &sites).expect("consensus");
        assert_eq!(first, second);
        assert_eq!(first.disposition, FederatedConsensusDisposition::Qualified);
        first.validate().expect("valid consensus");
    }

    #[test]
    fn heterogeneity_remains_negative_evidence() {
        let sites = vec![
            site("site-a", 100, 40),
            site("site-b", 500, 40),
            site("site-c", 510, 50),
        ];
        let output =
            analyze_glioma_federated_instrument_consensus(&request(), &sites).expect("consensus");
        assert_eq!(
            output.disposition,
            FederatedConsensusDisposition::Unresolved
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("heterogeneityblocked")));
    }
}
