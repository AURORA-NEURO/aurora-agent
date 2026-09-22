//! Replication-to-federation handoff for preclinical glioma research.
//!
//! This feature is the workflow bridge after independent-site replication: it converts only
//! typed, aggregate study summaries into the P12 mechanism-transport campaign, refuses blocked
//! or negative replication states, and keeps the institution-local raw observations behind the
//! federation boundary.  It is deliberately an executable handoff rather than a receipt or a
//! passive export format.

use super::transport_campaign::{
    execute_federated_mechanism_transport_campaign, FederatedMechanismTransportAction,
    FederatedMechanismTransportCampaign, FederatedMechanismTransportCampaignDisposition,
    FederatedMechanismTransportCampaignError, FederatedMechanismTransportCampaignRequest,
    FederatedMechanismTransportExecutor,
};
use crate::glioma::programs::p10_interpretation_replication::validation_replication_campaign::{
    ValidationReplicationCampaignDisposition, ValidationReplicationCampaignRun,
};
use crate::glioma::programs::p12_federated_benchmarking::mechanism_transport::{
    FederatedMechanismSite, FederatedMechanismTransportRequest,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaReplicationFederationTransport1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationTransportRequest {
    pub replication: ValidationReplicationCampaignRun,
    pub transport: FederatedMechanismTransportRequest,
    pub aggregate_quality: Vec<ReplicationAggregateQuality>,
    pub actions: Vec<FederatedMechanismTransportAction>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub stop_on_qualified: bool,
    pub stop_on_negative: bool,
}

/// Institution-local QC summary required when a P10 replication study is promoted to an
/// aggregate federation site. The raw QC trace remains local; only this bounded score crosses
/// the bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplicationAggregateQuality {
    pub study_id: String,
    pub quality_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationReplicationTransportDisposition {
    BlockedByReplication,
    HoldAggregateSites,
    Executed,
    Qualified,
    Negative,
    Heterogeneous,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReplicationTransportRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub source_replication_digest: ContentHash,
    pub source_replication_disposition: ValidationReplicationCampaignDisposition,
    pub aggregate_site_order: Vec<String>,
    pub campaign: Option<FederatedMechanismTransportCampaign>,
    pub disposition: ValidationReplicationTransportDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationReplicationTransportError {
    #[error("source validation-replication run is invalid: {0}")]
    Replication(String),
    #[error("replication-to-federation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated transport campaign failed: {0}")]
    Campaign(#[from] FederatedMechanismTransportCampaignError),
    #[error("replication-to-federation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replication-to-federation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(output: &ValidationReplicationTransportRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "source_replication_digest": output.source_replication_digest,
        "source_replication_disposition": output.source_replication_disposition,
        "aggregate_site_order": output.aggregate_site_order,
        "campaign": output.campaign,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn source_sites(
    replication: &ValidationReplicationCampaignRun,
    transport: &FederatedMechanismTransportRequest,
    aggregate_quality: &[ReplicationAggregateQuality],
) -> Result<Vec<FederatedMechanismSite>, ValidationReplicationTransportError> {
    let campaign = replication.campaign.as_ref().ok_or_else(|| {
        ValidationReplicationTransportError::InvalidRequest(
            "an executed replication campaign is required before federation transport".into(),
        )
    })?;
    let mut studies = BTreeSet::new();
    let quality_by_study = aggregate_quality
        .iter()
        .map(|entry| (entry.study_id.as_str(), entry.quality_milli))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut sites = Vec::new();
    for study in &campaign.studies {
        if study.site_id == replication.origin_site_id
            || study.site_id.trim().is_empty()
            || !studies.insert(study.study_id.clone())
            || !study.artifact.local_only
            || study.artifact.contains_human_data
            || study.artifact.contains_direct_identifiers
        {
            return Err(ValidationReplicationTransportError::InvalidRequest(
                "replication studies must be non-origin, unique, local-only, and non-human aggregates".into(),
            ));
        }
        let quality_milli = quality_by_study
            .get(study.study_id.as_str())
            .copied()
            .ok_or_else(|| {
                ValidationReplicationTransportError::InvalidRequest(format!(
                    "missing aggregate QC score for replication study {}",
                    study.study_id
                ))
            })?;
        sites.push(FederatedMechanismSite {
            site_id: study.site_id.clone(),
            study_id: study.study_id.clone(),
            mechanism_id: transport.mechanism_id.clone(),
            model_system: study.model_system,
            effect_milli: study.effect_milli,
            uncertainty_milli: study.uncertainty_milli,
            quality_milli,
            replicate_count: study.replicate_count,
            population_signature: transport.target_signature.clone(),
            artifact: study.artifact.clone(),
        });
    }
    for study in &campaign.transport_studies {
        if !studies.insert(study.study_id.clone()) {
            continue;
        }
        if study.study_id.trim().is_empty()
            || !study.artifact.local_only
            || study.artifact.contains_human_data
            || study.artifact.contains_direct_identifiers
        {
            return Err(ValidationReplicationTransportError::InvalidRequest(
                "transport studies must be unique, local-only, and non-human aggregates".into(),
            ));
        }
        sites.push(FederatedMechanismSite {
            site_id: format!("replication-transport:{}", study.study_id),
            study_id: study.study_id.clone(),
            mechanism_id: transport.mechanism_id.clone(),
            model_system: study.model_system,
            effect_milli: study.effect_milli,
            uncertainty_milli: study.uncertainty_milli,
            quality_milli: study.quality_milli,
            replicate_count: study.replicates,
            population_signature: study.population_signature.clone(),
            artifact: study.artifact.clone(),
        });
    }
    sites.sort_by(|left, right| {
        left.site_id
            .cmp(&right.site_id)
            .then_with(|| left.study_id.cmp(&right.study_id))
    });
    Ok(sites)
}

fn finish(
    request: &ValidationReplicationTransportRequest,
    sites: &[FederatedMechanismSite],
    campaign: Option<FederatedMechanismTransportCampaign>,
    disposition: ValidationReplicationTransportDisposition,
    negative_evidence: Vec<String>,
    uncertainty: Vec<String>,
    next_action: &str,
) -> Result<ValidationReplicationTransportRun, ValidationReplicationTransportError> {
    let mut aggregate_site_order = sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<BTreeSet<_>>();
    if let Some(campaign) = campaign.as_ref() {
        aggregate_site_order.extend(campaign.sites.iter().map(|site| site.site_id.clone()));
    }
    let mut output = ValidationReplicationTransportRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.transport.objective.clone(),
        model_system: request.transport.target_model_system,
        source_replication_digest: request.replication.digest.clone(),
        source_replication_disposition: request.replication.disposition,
        aggregate_site_order: aggregate_site_order.into_iter().collect(),
        campaign,
        disposition,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-replication-federation-transport"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ValidationReplicationTransportError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

fn validate_request(
    request: &ValidationReplicationTransportRequest,
) -> Result<(), ValidationReplicationTransportError> {
    request
        .replication
        .validate()
        .map_err(|error| ValidationReplicationTransportError::Replication(error.to_string()))?;
    if request.replication.objective != request.transport.objective
        || request.replication.model_system != request.transport.target_model_system
        || request.transport.objective.trim().is_empty()
        || request.transport.mechanism_id.trim().is_empty()
        || request.actions.is_empty()
        || request.budget_units == 0
        || request.max_rounds == 0
        || request.max_retries > 6
    {
        return Err(ValidationReplicationTransportError::InvalidRequest(
            "replication and transport objectives/model systems must match with bounded campaign settings".into(),
        ));
    }
    if request
        .actions
        .iter()
        .any(|action| action.model_system != request.transport.target_model_system)
    {
        return Err(ValidationReplicationTransportError::InvalidRequest(
            "every aggregate follow-up action must target the declared transport model system"
                .into(),
        ));
    }
    let mut quality_ids = BTreeSet::new();
    if request.aggregate_quality.iter().any(|entry| {
        entry.study_id.trim().is_empty()
            || entry.quality_milli > 1_000
            || !quality_ids.insert(entry.study_id.clone())
    }) {
        return Err(ValidationReplicationTransportError::InvalidRequest(
            "aggregate QC scores must have unique study identities and bounded quality values"
                .into(),
        ));
    }
    Ok(())
}

impl ValidationReplicationTransportRun {
    pub fn validate(&self) -> Result<(), ValidationReplicationTransportError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.aggregate_site_order)
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.next_action.trim().is_empty()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self
                .campaign
                .as_ref()
                .is_some_and(|campaign| campaign.validate().is_err())
        {
            return Err(ValidationReplicationTransportError::InvalidOutput(
                "identity, boundary, ordering, campaign, or next-action invariants are invalid"
                    .into(),
            ));
        }
        if self.campaign.as_ref().is_some_and(|campaign| {
            campaign.objective != self.objective
                || !campaign
                    .sites
                    .iter()
                    .all(|site| self.aggregate_site_order.contains(&site.site_id))
        }) {
            return Err(ValidationReplicationTransportError::InvalidOutput(
                "federated campaign sites are not bound to the aggregate source order".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ValidationReplicationTransportError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ValidationReplicationTransportError::InvalidOutput(
                "replication-to-federation output is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Continue an eligible independent-site replication result into aggregate-only federation.
pub fn execute_validation_replication_transport<E: FederatedMechanismTransportExecutor>(
    request: &ValidationReplicationTransportRequest,
    executor: &mut E,
) -> Result<ValidationReplicationTransportRun, ValidationReplicationTransportError> {
    validate_request(request)?;
    let mut negative_evidence = request.replication.negative_evidence.clone();
    let mut uncertainty = request.replication.uncertainty.clone();
    if !matches!(
        request.replication.disposition,
        ValidationReplicationCampaignDisposition::Executed
            | ValidationReplicationCampaignDisposition::Qualified
    ) {
        negative_evidence.push("replication-not-eligible-for-federation".into());
        uncertainty.push(
            "blocked, held, or negative replication cannot be promoted into consortium transport"
                .into(),
        );
        return finish(
            request,
            &[],
            None,
            ValidationReplicationTransportDisposition::BlockedByReplication,
            negative_evidence,
            uncertainty,
            "resolve the independent-site replication disposition before federation transport",
        );
    }
    let sites = source_sites(
        &request.replication,
        &request.transport,
        &request.aggregate_quality,
    )?;
    if sites.len() < request.transport.min_sites {
        negative_evidence.push(format!(
            "aggregate-sites:{}<{}",
            sites.len(),
            request.transport.min_sites
        ));
        uncertainty.push(
            "federation transport requires the declared minimum independent aggregate sites".into(),
        );
        return finish(
            request,
            &sites,
            None,
            ValidationReplicationTransportDisposition::HoldAggregateSites,
            negative_evidence,
            uncertainty,
            "collect additional independent aggregate site summaries before federation transport",
        );
    }
    let campaign_request = FederatedMechanismTransportCampaignRequest {
        transport: request.transport.clone(),
        initial_sites: sites.clone(),
        actions: request.actions.clone(),
        budget_units: request.budget_units,
        max_rounds: request.max_rounds,
        max_retries: request.max_retries,
        stop_on_qualified: request.stop_on_qualified,
        stop_on_negative: request.stop_on_negative,
    };
    let campaign = execute_federated_mechanism_transport_campaign(&campaign_request, executor)?;
    negative_evidence.extend(campaign.negative_evidence.clone());
    uncertainty.extend(campaign.uncertainty.clone());
    let (disposition, next_action) = match campaign.disposition {
        FederatedMechanismTransportCampaignDisposition::Qualified => (
            ValidationReplicationTransportDisposition::Qualified,
            "hold the qualified aggregate transport result for independent methods review and signed release evidence",
        ),
        FederatedMechanismTransportCampaignDisposition::Negative => (
            ValidationReplicationTransportDisposition::Negative,
            "publish the negative or null federated transport result and preserve its boundary conditions",
        ),
        FederatedMechanismTransportCampaignDisposition::Heterogeneous => (
            ValidationReplicationTransportDisposition::Heterogeneous,
            "stratify the mechanism by model and site signature before another aggregate transport wave",
        ),
        FederatedMechanismTransportCampaignDisposition::Partial => (
            ValidationReplicationTransportDisposition::Executed,
            "continue only with the returned aggregate action partitions and an explicit federation budget",
        ),
        FederatedMechanismTransportCampaignDisposition::BudgetBlocked
        | FederatedMechanismTransportCampaignDisposition::Failed => (
            ValidationReplicationTransportDisposition::Blocked,
            "repair the aggregate worker or allocate a bounded federation budget before retrying",
        ),
        FederatedMechanismTransportCampaignDisposition::Unresolved => (
            ValidationReplicationTransportDisposition::Unresolved,
            "resolve missing, contradictory, or transportability evidence before another federation action",
        ),
    };
    finish(
        request,
        &sites,
        Some(campaign),
        disposition,
        negative_evidence,
        uncertainty,
        next_action,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::validation_replication_gate::{
        ValidationReplicationGate, ValidationReplicationGateDisposition,
    };

    fn blocked_request() -> ValidationReplicationTransportRequest {
        let mut gate = ValidationReplicationGate {
            feature_id: "GAF-GLIOMA-P10-F22".into(),
            output_schema: "GliomaValidationReplicationGate1@1".into(),
            objective: "replicate invasion mechanism".into(),
            model_system: GliomaModelSystem::Organoid,
            validation_campaign_digest: ContentHash::of_bytes(b"validation"),
            local_site_id: "origin".into(),
            independent_site_order: Vec::new(),
            validation_eligible: false,
            replication_plan: None,
            continuation: None,
            protocol: None,
            disposition: ValidationReplicationGateDisposition::HoldValidation,
            negative_evidence: vec!["awaiting-efficacy-stop".into()],
            uncertainty: vec!["local validation is not qualified".into()],
            next_action: "complete validation".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        gate.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": gate.feature_id,
            "output_schema": gate.output_schema,
            "objective": gate.objective,
            "model_system": gate.model_system,
            "validation_campaign_digest": gate.validation_campaign_digest,
            "local_site_id": gate.local_site_id,
            "independent_site_order": gate.independent_site_order,
            "validation_eligible": gate.validation_eligible,
            "replication_plan": gate.replication_plan,
            "continuation": gate.continuation,
            "protocol": gate.protocol,
            "disposition": gate.disposition,
            "negative_evidence": gate.negative_evidence,
            "uncertainty": gate.uncertainty,
            "next_action": gate.next_action,
            "boundary": gate.boundary,
        }))
        .unwrap();
        let replication = ValidationReplicationCampaignRun {
            feature_id: "GAF-GLIOMA-P10-F23".into(),
            output_schema: "GliomaValidationReplicationCampaign1@1".into(),
            objective: gate.objective.clone(),
            model_system: gate.model_system,
            validation_campaign_digest: gate.validation_campaign_digest.clone(),
            origin_site_id: gate.local_site_id.clone(),
            independent_site_order: Vec::new(),
            gate_disposition: gate.disposition,
            campaign: None,
            disposition: ValidationReplicationCampaignDisposition::BlockedByValidation,
            stop_reason: None,
            negative_evidence: vec!["validation-not-eligible".into()],
            uncertainty: vec!["awaiting local validation".into()],
            next_action: "complete validation".into(),
            boundary: PRECLINICAL_BOUNDARY.into(),
            digest: ContentHash::of_bytes(b"unsealed"),
        };
        let mut replication = replication;
        replication.digest = ContentHash::of_value(&serde_json::json!({
            "feature_id": replication.feature_id,
            "output_schema": replication.output_schema,
            "objective": replication.objective,
            "model_system": replication.model_system,
            "validation_campaign_digest": replication.validation_campaign_digest,
            "origin_site_id": replication.origin_site_id,
            "independent_site_order": replication.independent_site_order,
            "gate_disposition": replication.gate_disposition,
            "campaign": replication.campaign,
            "disposition": replication.disposition,
            "stop_reason": replication.stop_reason,
            "negative_evidence": replication.negative_evidence,
            "uncertainty": replication.uncertainty,
            "next_action": replication.next_action,
            "boundary": replication.boundary,
        }))
        .unwrap();
        ValidationReplicationTransportRequest {
            replication,
            transport: FederatedMechanismTransportRequest {
                objective: gate.objective,
                mechanism_id: "invasion".into(),
                target_model_system: GliomaModelSystem::Organoid,
                target_signature: vec![0, 0],
                min_sites: 1,
                min_replicates_per_site: 1,
                min_quality_milli: 500,
                similarity_scale_milli: 1_000,
                effect_threshold_milli: 10,
                min_signal_to_noise_milli: 1,
                max_heterogeneity_milli: 900,
                max_site_spread_milli: 1_000,
                max_leave_one_out_shift_milli: 1_000,
                require_target_model: true,
            },
            aggregate_quality: Vec::new(),
            actions: vec![FederatedMechanismTransportAction {
                action_id: "site-follow-up".into(),
                target_site_id: Some("site-b".into()),
                model_system: GliomaModelSystem::Organoid,
                population_signature: vec![0, 0],
                cost_units: 1,
                expected_information_milli: 100,
                expected_effect_milli: 100,
                expected_heterogeneity_reduction_milli: 100,
                feasibility_milli: 900,
                risk_milli: 10,
                requested_replicates: 1,
            }],
            budget_units: 1,
            max_rounds: 1,
            max_retries: 0,
            stop_on_qualified: false,
            stop_on_negative: false,
        }
    }

    #[test]
    fn blocked_replication_never_enters_federation() {
        let request = blocked_request();
        let mut executor =
            super::super::transport_campaign::DryRunFederatedMechanismTransportExecutor;
        let output = execute_validation_replication_transport(&request, &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ValidationReplicationTransportDisposition::BlockedByReplication
        );
        assert!(output.campaign.is_none());
        output.validate().unwrap();
    }
}
