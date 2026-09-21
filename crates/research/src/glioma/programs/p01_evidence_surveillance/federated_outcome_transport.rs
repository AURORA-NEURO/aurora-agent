//! Policy-gated transport of aggregate glioma outcomes between research sites.
//!
//! The transport boundary is intentionally narrower than a data lake or a remote execution
//! client.  A site contributes a typed, content-addressed outcome summary; raw measurements,
//! specimens, human data, and instrument commands remain at that site.  The compiler checks the
//! consortium contract, claim/scope identity, capability and key revocation, freshness, quality,
//! and independent-site quorum before producing a deterministic export plan.  Negative,
//! contradictory, unknown, and deferred outcomes remain visible instead of being collapsed into
//! a positive consensus.

use crate::glioma::evidence::EvidenceState;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F29";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedOutcomeTransport1@1";
pub const MAX_BUNDLES: usize = 1_024;
pub const MAX_ROUTES: usize = 64;

/// A bounded decision for one site contribution.  `Accepted` means only the aggregate summary
/// may cross the federation boundary; it does not claim the underlying result is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedOutcomeBundleDecision {
    Accepted,
    Deferred,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedOutcomeTransportDisposition {
    Ready,
    Partial,
    Blocked,
}

/// Site-local aggregate evidence.  The attestation is a content hash over the fields that a
/// consortium signer would sign; a transport adapter may replace it with an external signature
/// envelope while retaining the same canonical payload and validation rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOutcomeBundle {
    pub bundle_id: String,
    pub site_id: String,
    pub consortium_id: String,
    pub independence_group: String,
    pub claim_key: String,
    pub scope_key: String,
    pub source_epoch: u32,
    pub modality: Option<GliomaModality>,
    pub model_system: Option<GliomaModelSystem>,
    pub outcome_state: EvidenceState,
    pub support_milli: u16,
    pub uncertainty_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub outcome_digest: ContentHash,
    pub calibration_digest: ContentHash,
    pub reconciliation_digest: ContentHash,
    pub capability_version: String,
    pub purpose: String,
    pub key_id: String,
    pub attestation_digest: ContentHash,
    pub raw_data_local: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub contains_clinical_decision: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOutcomeTransportRequest {
    pub objective: String,
    pub consortium_id: String,
    pub claim_key: String,
    pub scope_key: String,
    pub allowed_purpose: String,
    pub required_capability_version: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub minimum_sites: usize,
    pub minimum_independent_groups: usize,
    pub current_epoch: u32,
    pub maximum_age_epochs: u32,
    pub min_quality_milli: u16,
    pub min_reproducibility_milli: u16,
    pub max_bundles: usize,
    pub require_local_raw_data: bool,
    pub revoked_site_ids: BTreeSet<String>,
    pub revoked_key_ids: BTreeSet<String>,
    pub bundles: Vec<FederatedOutcomeBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOutcomeBundleDecisionRecord {
    pub bundle_id: String,
    pub site_id: String,
    pub independence_group: String,
    pub decision: FederatedOutcomeBundleDecision,
    pub reason: String,
    pub outcome_state: EvidenceState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOutcomeTransportReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub consortium_id: String,
    pub claim_key: String,
    pub scope_key: String,
    pub bundle_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub decisions: Vec<FederatedOutcomeBundleDecisionRecord>,
    pub accepted_site_order: Vec<String>,
    pub accepted_independent_group_order: Vec<String>,
    pub accepted_modality_order: Vec<GliomaModality>,
    pub accepted_model_system_order: Vec<GliomaModelSystem>,
    pub required_modalities_missing: Vec<GliomaModality>,
    pub required_model_systems_missing: Vec<GliomaModelSystem>,
    pub accepted_count: usize,
    pub independent_group_count: usize,
    pub site_quorum_satisfied: bool,
    pub independent_quorum_satisfied: bool,
    pub raw_data_local_only: bool,
    pub omission_order: Vec<String>,
    pub disposition: FederatedOutcomeTransportDisposition,
    pub next_routes: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedOutcomeTransportError {
    #[error("federated outcome transport request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated outcome transport output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated outcome transport digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_hash(hash: &ContentHash) -> bool {
    hash.as_str().len() == 64
}

fn attestation_payload(bundle: &FederatedOutcomeBundle) -> serde_json::Value {
    serde_json::json!({
        "bundle_id": bundle.bundle_id,
        "site_id": bundle.site_id,
        "consortium_id": bundle.consortium_id,
        "independence_group": bundle.independence_group,
        "claim_key": bundle.claim_key,
        "scope_key": bundle.scope_key,
        "source_epoch": bundle.source_epoch,
        "modality": bundle.modality,
        "model_system": bundle.model_system,
        "outcome_state": bundle.outcome_state,
        "support_milli": bundle.support_milli,
        "uncertainty_milli": bundle.uncertainty_milli,
        "quality_milli": bundle.quality_milli,
        "reproducibility_milli": bundle.reproducibility_milli,
        "outcome_digest": bundle.outcome_digest,
        "calibration_digest": bundle.calibration_digest,
        "reconciliation_digest": bundle.reconciliation_digest,
        "capability_version": bundle.capability_version,
        "purpose": bundle.purpose,
        "key_id": bundle.key_id,
        "raw_data_local": bundle.raw_data_local,
        "contains_human_data": bundle.contains_human_data,
        "contains_direct_identifiers": bundle.contains_direct_identifiers,
        "contains_clinical_decision": bundle.contains_clinical_decision,
    })
}

fn expected_attestation(
    bundle: &FederatedOutcomeBundle,
) -> Result<ContentHash, FederatedOutcomeTransportError> {
    ContentHash::of_value(&attestation_payload(bundle))
        .map_err(|error| FederatedOutcomeTransportError::Digest(error.to_string()))
}

fn digest_input(report: &FederatedOutcomeTransportReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "objective": report.objective,
        "consortium_id": report.consortium_id,
        "claim_key": report.claim_key,
        "scope_key": report.scope_key,
        "bundle_order": report.bundle_order,
        "accepted_order": report.accepted_order,
        "deferred_order": report.deferred_order,
        "denied_order": report.denied_order,
        "decisions": report.decisions,
        "accepted_site_order": report.accepted_site_order,
        "accepted_independent_group_order": report.accepted_independent_group_order,
        "accepted_modality_order": report.accepted_modality_order,
        "accepted_model_system_order": report.accepted_model_system_order,
        "required_modalities_missing": report.required_modalities_missing,
        "required_model_systems_missing": report.required_model_systems_missing,
        "accepted_count": report.accepted_count,
        "independent_group_count": report.independent_group_count,
        "site_quorum_satisfied": report.site_quorum_satisfied,
        "independent_quorum_satisfied": report.independent_quorum_satisfied,
        "raw_data_local_only": report.raw_data_local_only,
        "omission_order": report.omission_order,
        "disposition": report.disposition,
        "next_routes": report.next_routes,
    })
}

impl FederatedOutcomeTransportReport {
    pub fn validate(&self) -> Result<(), FederatedOutcomeTransportError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.consortium_id.trim().is_empty()
            || self.claim_key.trim().is_empty()
            || self.scope_key.trim().is_empty()
            || !canonical(&self.bundle_order)
            || !canonical(&self.accepted_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.denied_order)
            || !canonical(&self.accepted_site_order)
            || !canonical(&self.accepted_independent_group_order)
            || !canonical(&self.accepted_modality_order)
            || !canonical(&self.accepted_model_system_order)
            || !canonical(&self.required_modalities_missing)
            || !canonical(&self.required_model_systems_missing)
            || !canonical(&self.omission_order)
            || !canonical(&self.next_routes)
            || !canonical(
                &self
                    .decisions
                    .iter()
                    .map(|decision| decision.bundle_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.bundle_order.len() != self.decisions.len()
            || self.accepted_count != self.accepted_order.len()
            || self.independent_group_count != self.accepted_independent_group_order.len()
            || !self.raw_data_local_only
            || self.digest.as_str().len() != 64
            || self.decisions.iter().any(|decision| {
                decision.bundle_id.trim().is_empty()
                    || decision.site_id.trim().is_empty()
                    || decision.independence_group.trim().is_empty()
                    || decision.reason.trim().is_empty()
            })
        {
            return Err(FederatedOutcomeTransportError::InvalidOutput(
                "identity, canonical ordering, decision coverage, locality, or digest fields are invalid".into(),
            ));
        }
        let bundle_set = self.bundle_order.iter().cloned().collect::<BTreeSet<_>>();
        let decision_set = self
            .decisions
            .iter()
            .map(|decision| decision.bundle_id.clone())
            .collect::<BTreeSet<_>>();
        let accepted_set = self.accepted_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred_set = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let denied_set = self.denied_order.iter().cloned().collect::<BTreeSet<_>>();
        let classified = accepted_set
            .union(&deferred_set)
            .chain(denied_set.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_sites = self
            .decisions
            .iter()
            .filter(|decision| decision.decision == FederatedOutcomeBundleDecision::Accepted)
            .map(|decision| decision.site_id.clone())
            .collect::<BTreeSet<_>>();
        let expected_groups = self
            .decisions
            .iter()
            .filter(|decision| decision.decision == FederatedOutcomeBundleDecision::Accepted)
            .map(|decision| decision.independence_group.clone())
            .collect::<BTreeSet<_>>();
        if bundle_set.len() != self.bundle_order.len()
            || bundle_set != decision_set
            || classified != bundle_set
            || accepted_set.len() != self.accepted_order.len()
            || deferred_set.len() != self.deferred_order.len()
            || denied_set.len() != self.denied_order.len()
            || accepted_set.intersection(&deferred_set).next().is_some()
            || accepted_set.intersection(&denied_set).next().is_some()
            || deferred_set.intersection(&denied_set).next().is_some()
            || expected_sites
                != self
                    .accepted_site_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || expected_groups
                != self
                    .accepted_independent_group_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || self.site_quorum_satisfied && self.accepted_count == 0
            || self.independent_quorum_satisfied && self.independent_group_count == 0
        {
            return Err(FederatedOutcomeTransportError::InvalidOutput(
                "bundle partitions, quorum flags, or decision coverage are inconsistent".into(),
            ));
        }
        for decision in &self.decisions {
            let in_partition = match decision.decision {
                FederatedOutcomeBundleDecision::Accepted => {
                    accepted_set.contains(&decision.bundle_id)
                }
                FederatedOutcomeBundleDecision::Deferred => {
                    deferred_set.contains(&decision.bundle_id)
                }
                FederatedOutcomeBundleDecision::Denied => denied_set.contains(&decision.bundle_id),
            };
            if !in_partition {
                return Err(FederatedOutcomeTransportError::InvalidOutput(
                    "decision does not match its partition".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedOutcomeTransportError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedOutcomeTransportError::Digest(
                "transport digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn invalid_bundle_reason(
    request: &FederatedOutcomeTransportRequest,
    bundle: &FederatedOutcomeBundle,
) -> Option<String> {
    if bundle.consortium_id != request.consortium_id {
        return Some("consortium_mismatch".into());
    }
    if bundle.claim_key != request.claim_key || bundle.scope_key != request.scope_key {
        return Some("claim_or_scope_mismatch".into());
    }
    if bundle.purpose != request.allowed_purpose {
        return Some("purpose_not_allowed".into());
    }
    if bundle.capability_version != request.required_capability_version {
        return Some("capability_version_mismatch".into());
    }
    if request.revoked_site_ids.contains(&bundle.site_id) {
        return Some("site_revoked".into());
    }
    if request.revoked_key_ids.contains(&bundle.key_id) {
        return Some("attestation_key_revoked".into());
    }
    if !request.require_local_raw_data || bundle.raw_data_local {
        // The first disjunct is a compatibility mode for an explicit local-only false policy.
    } else {
        return Some("raw_data_locality_not_proven".into());
    }
    if bundle.contains_human_data || bundle.contains_direct_identifiers {
        return Some("human_or_direct_identifier_data_forbidden".into());
    }
    if bundle.contains_clinical_decision {
        return Some("clinical_decision_payload_forbidden".into());
    }
    if !valid_hash(&bundle.outcome_digest)
        || !valid_hash(&bundle.calibration_digest)
        || !valid_hash(&bundle.reconciliation_digest)
        || !valid_hash(&bundle.attestation_digest)
    {
        return Some("content_hash_missing_or_malformed".into());
    }
    if bundle.key_id.trim().is_empty() || bundle.site_id.trim().is_empty() {
        return Some("identity_missing".into());
    }
    if bundle.source_epoch > request.current_epoch {
        return Some("future_epoch".into());
    }
    if request.current_epoch.saturating_sub(bundle.source_epoch) > request.maximum_age_epochs {
        return Some("stale_epoch".into());
    }
    if bundle.quality_milli < request.min_quality_milli {
        return Some("quality_below_floor".into());
    }
    if bundle.reproducibility_milli < request.min_reproducibility_milli {
        return Some("reproducibility_below_floor".into());
    }
    None
}

/// Compile a deterministic aggregate-only federation export.  This function does not contact a
/// site, copy bytes, run an assay, or decide whether a biological claim is true.
pub fn compile_glioma_federated_outcome_transport(
    request: &FederatedOutcomeTransportRequest,
) -> Result<FederatedOutcomeTransportReport, FederatedOutcomeTransportError> {
    if request.objective.trim().is_empty()
        || request.consortium_id.trim().is_empty()
        || request.claim_key.trim().is_empty()
        || request.scope_key.trim().is_empty()
        || request.allowed_purpose.trim().is_empty()
        || request.required_capability_version.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_independent_groups == 0
        || request.current_epoch == 0
        || request.maximum_age_epochs > request.current_epoch
        || request.max_bundles == 0
        || request.max_bundles > MAX_BUNDLES
        || request.bundles.len() > MAX_BUNDLES
        || request.max_bundles < request.bundles.len()
        || !request.require_local_raw_data
    {
        return Err(FederatedOutcomeTransportError::InvalidRequest(
            "objective, consortium contract, quorum, epoch, bundle bounds, or local-only policy is invalid".into(),
        ));
    }
    let mut bundles = request.bundles.clone();
    bundles.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    if bundles
        .windows(2)
        .any(|pair| pair[0].bundle_id == pair[1].bundle_id)
    {
        return Err(FederatedOutcomeTransportError::InvalidRequest(
            "bundle identifiers must be unique".into(),
        ));
    }

    let mut decisions = Vec::with_capacity(bundles.len());
    for bundle in &bundles {
        if bundle.bundle_id.trim().is_empty()
            || bundle.independence_group.trim().is_empty()
            || bundle.attestation_digest != expected_attestation(bundle)?
        {
            decisions.push(FederatedOutcomeBundleDecisionRecord {
                bundle_id: bundle.bundle_id.clone(),
                site_id: bundle.site_id.clone(),
                independence_group: bundle.independence_group.clone(),
                decision: FederatedOutcomeBundleDecision::Denied,
                reason: if bundle.bundle_id.trim().is_empty() {
                    "identity_missing".into()
                } else if bundle.independence_group.trim().is_empty() {
                    "independence_group_missing".into()
                } else {
                    "attestation_mismatch".into()
                },
                outcome_state: bundle.outcome_state,
            });
            continue;
        }
        let decision = if let Some(reason) = invalid_bundle_reason(request, bundle) {
            if matches!(
                reason.as_str(),
                "stale_epoch" | "quality_below_floor" | "reproducibility_below_floor"
            ) || matches!(
                bundle.outcome_state,
                EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Stale
            ) {
                FederatedOutcomeBundleDecision::Deferred
            } else {
                FederatedOutcomeBundleDecision::Denied
            }
        } else if matches!(
            bundle.outcome_state,
            EvidenceState::Unknown | EvidenceState::Unmeasured | EvidenceState::Stale
        ) {
            FederatedOutcomeBundleDecision::Deferred
        } else {
            FederatedOutcomeBundleDecision::Accepted
        };
        let reason = invalid_bundle_reason(request, bundle).unwrap_or_else(|| match decision {
            FederatedOutcomeBundleDecision::Accepted => "aggregate_summary_ready".into(),
            FederatedOutcomeBundleDecision::Deferred => "outcome_requires_review_or_refresh".into(),
            FederatedOutcomeBundleDecision::Denied => "policy_gate_failed".into(),
        });
        decisions.push(FederatedOutcomeBundleDecisionRecord {
            bundle_id: bundle.bundle_id.clone(),
            site_id: bundle.site_id.clone(),
            independence_group: bundle.independence_group.clone(),
            decision,
            reason,
            outcome_state: bundle.outcome_state,
        });
    }
    decisions.sort_by(|left, right| left.bundle_id.cmp(&right.bundle_id));
    let bundle_order = decisions
        .iter()
        .map(|row| row.bundle_id.clone())
        .collect::<Vec<_>>();
    let accepted_order = decisions
        .iter()
        .filter(|row| row.decision == FederatedOutcomeBundleDecision::Accepted)
        .map(|row| row.bundle_id.clone())
        .collect::<Vec<_>>();
    let deferred_order = decisions
        .iter()
        .filter(|row| row.decision == FederatedOutcomeBundleDecision::Deferred)
        .map(|row| row.bundle_id.clone())
        .collect::<Vec<_>>();
    let denied_order = decisions
        .iter()
        .filter(|row| row.decision == FederatedOutcomeBundleDecision::Denied)
        .map(|row| row.bundle_id.clone())
        .collect::<Vec<_>>();
    let accepted = bundles
        .iter()
        .filter(|bundle| accepted_order.binary_search(&bundle.bundle_id).is_ok())
        .collect::<Vec<_>>();
    let accepted_site_order = accepted
        .iter()
        .map(|bundle| bundle.site_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let accepted_independent_group_order = accepted
        .iter()
        .map(|bundle| bundle.independence_group.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let accepted_modality_order = accepted
        .iter()
        .filter_map(|bundle| bundle.modality)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let accepted_model_system_order = accepted
        .iter()
        .filter_map(|bundle| bundle.model_system)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let required_modalities_missing = request
        .required_modalities
        .difference(&accepted_modality_order.iter().copied().collect())
        .copied()
        .collect::<Vec<_>>();
    let required_model_systems_missing = request
        .required_model_systems
        .difference(&accepted_model_system_order.iter().copied().collect())
        .copied()
        .collect::<Vec<_>>();
    let site_quorum_satisfied = accepted_site_order.len() >= request.minimum_sites;
    let independent_quorum_satisfied =
        accepted_independent_group_order.len() >= request.minimum_independent_groups;
    let independent_group_count = accepted_independent_group_order.len();
    let raw_data_local_only = accepted.iter().all(|bundle| bundle.raw_data_local);
    let mut omission_order = decisions
        .iter()
        .filter(|row| row.decision != FederatedOutcomeBundleDecision::Accepted)
        .map(|row| format!("{}:{}", row.bundle_id, row.reason))
        .collect::<Vec<_>>();
    for modality in &required_modalities_missing {
        omission_order.push(format!("missing_modality:{modality:?}"));
    }
    for model in &required_model_systems_missing {
        omission_order.push(format!("missing_model_system:{model:?}"));
    }
    omission_order.sort();
    let disposition = if site_quorum_satisfied
        && independent_quorum_satisfied
        && required_modalities_missing.is_empty()
        && required_model_systems_missing.is_empty()
    {
        FederatedOutcomeTransportDisposition::Ready
    } else if accepted.is_empty() {
        FederatedOutcomeTransportDisposition::Blocked
    } else {
        FederatedOutcomeTransportDisposition::Partial
    };
    let mut next_routes = match disposition {
        FederatedOutcomeTransportDisposition::Ready => vec![
            "glioma_multisite_outcome_reconciliation".into(),
            "glioma_evidence_knowledge_bridge".into(),
        ],
        FederatedOutcomeTransportDisposition::Partial => vec![
            "glioma_federated_evidence_acquisition_policy".into(),
            "glioma_evidence_prospective_triage".into(),
        ],
        FederatedOutcomeTransportDisposition::Blocked => vec![
            "glioma_federated_evidence_acquisition_policy".into(),
            "glioma_evidence_verification_gate".into(),
        ],
    };
    next_routes.sort();
    next_routes.truncate(MAX_ROUTES);
    let mut report = FederatedOutcomeTransportReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        consortium_id: request.consortium_id.clone(),
        claim_key: request.claim_key.clone(),
        scope_key: request.scope_key.clone(),
        bundle_order,
        accepted_order,
        deferred_order,
        denied_order,
        decisions,
        accepted_site_order,
        accepted_independent_group_order,
        accepted_modality_order,
        accepted_model_system_order,
        required_modalities_missing,
        required_model_systems_missing,
        accepted_count: accepted.len(),
        independent_group_count,
        site_quorum_satisfied,
        independent_quorum_satisfied,
        raw_data_local_only,
        omission_order,
        disposition,
        next_routes,
        digest: ContentHash::of_bytes(b"pending"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| FederatedOutcomeTransportError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::evidence::EvidenceState;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn bundle(
        id: &str,
        site: &str,
        group: &str,
        epoch: u32,
        state: EvidenceState,
    ) -> FederatedOutcomeBundle {
        let mut value = FederatedOutcomeBundle {
            bundle_id: id.into(),
            site_id: site.into(),
            consortium_id: "consortium-a".into(),
            independence_group: group.into(),
            claim_key: "egfr-state".into(),
            scope_key: "preclinical-gbm".into(),
            source_epoch: epoch,
            modality: Some(GliomaModality::Transcriptomics),
            model_system: Some(GliomaModelSystem::Organoid),
            outcome_state: state,
            support_milli: 700,
            uncertainty_milli: 200,
            quality_milli: 900,
            reproducibility_milli: 850,
            outcome_digest: hash("outcome"),
            calibration_digest: hash("calibration"),
            reconciliation_digest: hash("reconciliation"),
            capability_version: "glioma-outcome/1".into(),
            purpose: "benchmarking".into(),
            key_id: "key-a".into(),
            attestation_digest: hash("pending"),
            raw_data_local: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            contains_clinical_decision: false,
        };
        value.attestation_digest = expected_attestation(&value).unwrap();
        value
    }

    fn request(bundles: Vec<FederatedOutcomeBundle>) -> FederatedOutcomeTransportRequest {
        FederatedOutcomeTransportRequest {
            objective: "transport bounded outcomes".into(),
            consortium_id: "consortium-a".into(),
            claim_key: "egfr-state".into(),
            scope_key: "preclinical-gbm".into(),
            allowed_purpose: "benchmarking".into(),
            required_capability_version: "glioma-outcome/1".into(),
            required_modalities: [GliomaModality::Transcriptomics].into_iter().collect(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            minimum_sites: 2,
            minimum_independent_groups: 2,
            current_epoch: 10,
            maximum_age_epochs: 3,
            min_quality_milli: 700,
            min_reproducibility_milli: 700,
            max_bundles: 16,
            require_local_raw_data: true,
            revoked_site_ids: BTreeSet::new(),
            revoked_key_ids: BTreeSet::new(),
            bundles,
        }
    }

    #[test]
    fn ready_transport_keeps_negative_outcome_visible() {
        let report = compile_glioma_federated_outcome_transport(&request(vec![
            bundle("b-02", "site-2", "group-2", 9, EvidenceState::Negative),
            bundle("b-01", "site-1", "group-1", 10, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            report.disposition,
            FederatedOutcomeTransportDisposition::Ready
        );
        assert_eq!(report.accepted_count, 2);
        assert_eq!(report.decisions[0].outcome_state, EvidenceState::Supported);
        assert_eq!(report.decisions[1].outcome_state, EvidenceState::Negative);
        report.validate().unwrap();
    }

    #[test]
    fn stale_and_unknown_bundles_are_deferred() {
        let report = compile_glioma_federated_outcome_transport(&request(vec![
            bundle("b-01", "site-1", "group-1", 1, EvidenceState::Supported),
            bundle("b-02", "site-2", "group-2", 10, EvidenceState::Unknown),
        ]))
        .unwrap();
        assert_eq!(report.deferred_order, vec!["b-01", "b-02"]);
        assert_eq!(
            report.disposition,
            FederatedOutcomeTransportDisposition::Blocked
        );
    }

    #[test]
    fn revoked_site_is_denied_without_moving_data() {
        let mut request = request(vec![bundle(
            "b-01",
            "site-1",
            "group-1",
            10,
            EvidenceState::Supported,
        )]);
        request.revoked_site_ids.insert("site-1".into());
        let report = compile_glioma_federated_outcome_transport(&request).unwrap();
        assert_eq!(report.denied_order, vec!["b-01"]);
        assert!(report.raw_data_local_only);
    }

    #[test]
    fn permutation_has_same_digest() {
        let first = compile_glioma_federated_outcome_transport(&request(vec![
            bundle("b-02", "site-2", "group-2", 9, EvidenceState::Supported),
            bundle("b-01", "site-1", "group-1", 10, EvidenceState::Supported),
        ]))
        .unwrap();
        let second = compile_glioma_federated_outcome_transport(&request(vec![
            bundle("b-01", "site-1", "group-1", 10, EvidenceState::Supported),
            bundle("b-02", "site-2", "group-2", 9, EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(first.digest, second.digest);
    }
}
