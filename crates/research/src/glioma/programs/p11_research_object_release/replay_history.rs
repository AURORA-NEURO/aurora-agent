//! Long-horizon reconciliation of independently produced archival replay campaigns.
//!
//! GAF-GLIOMA-P11-F12 compares bounded, already-validated replay reports across expected epochs
//! and site commitments. It never retrieves archived code or data, reruns a historical task, or
//! authenticates a site. Site commitments are caller-supplied opaque digests; callers should use
//! keyed commitments when site identity must resist dictionary attacks.

use super::replay::{MAX_ROUNDS, MAX_TASKS, ReplayCampaign, ReplayCampaignDisposition};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectReplayHistory1@1";
pub const MAX_EPOCHS: usize = 32;
pub const MAX_SITES_PER_EPOCH: usize = 64;
pub const MAX_CAMPAIGN_RECORDS: usize = 512;
pub const MAX_ID_BYTES: usize = 256;
pub const MAX_TEXT_BYTES: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHistoryRow {
    /// A caller-defined longitudinal epoch, ordered numerically in the request.
    pub epoch_index: u32,
    /// Opaque site identity commitment. Do not provide an unhashed site name here.
    pub site_commitment: ContentHash,
    /// An archival campaign that has already passed its internal digest validation.
    pub campaign: ReplayCampaign,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHistoryRequest {
    pub research_id: String,
    pub manifest_digest: ContentHash,
    /// Expected epochs, strictly increasing. Missing site reports remain visible as insufficiency.
    pub expected_epoch_order: Vec<u32>,
    /// Distinct site reports required for each expected epoch; must be at least two.
    pub min_sites_per_epoch: u16,
    pub rows: Vec<ReplayHistoryRow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayHistoryDisposition {
    Reproducible,
    Divergent,
    Insufficient,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayHistorySiteDisposition {
    Reproducible,
    Divergent,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHistoryEntry {
    pub epoch_index: u32,
    pub site_commitment: ContentHash,
    pub campaign_digest: ContentHash,
    pub disposition: ReplayHistorySiteDisposition,
    pub coverage_milli: u16,
    pub previous_digest: ContentHash,
    pub entry_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHistoryEpochSummary {
    pub epoch_index: u32,
    pub required_sites: u16,
    pub observed_sites: u16,
    pub reproducible_sites: u16,
    pub divergent_sites: u16,
    pub unresolved_sites: u16,
    pub disposition: ReplayHistoryDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayHistoryReport {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub manifest_digest: ContentHash,
    pub expected_epoch_order: Vec<u32>,
    pub min_sites_per_epoch: u16,
    /// Contains only opaque commitments and campaign digests, never campaign objectives,
    /// task identifiers, artifact names, or raw artifacts.
    pub entries: Vec<ReplayHistoryEntry>,
    pub epochs: Vec<ReplayHistoryEpochSummary>,
    pub disposition: ReplayHistoryDisposition,
    pub chain_head: ContentHash,
    pub report_digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayHistoryError {
    #[error("replay history request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replay history campaign is invalid: {0}")]
    InvalidCampaign(String),
    #[error("replay history report is invalid: {0}")]
    InvalidOutput(String),
    #[error("replay history digest failed: {0}")]
    Digest(String),
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, ReplayHistoryError> {
    ContentHash::of_serializable(value)
        .map_err(|error| ReplayHistoryError::Digest(error.to_string()))
}

fn validate_request(request: &ReplayHistoryRequest) -> Result<(), ReplayHistoryError> {
    if request.research_id.trim().is_empty()
        || request.research_id.len() > MAX_ID_BYTES
        || request.expected_epoch_order.len() < 2
        || request.expected_epoch_order.len() > MAX_EPOCHS
        || request
            .expected_epoch_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.min_sites_per_epoch < 2
        || usize::from(request.min_sites_per_epoch) > MAX_SITES_PER_EPOCH
        || request.rows.len() > MAX_CAMPAIGN_RECORDS
    {
        return Err(ReplayHistoryError::InvalidRequest(
            "research identity, at least two ordered epochs, site quorum, and bounded campaign records are required".into(),
        ));
    }

    let epochs = request
        .expected_epoch_order
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut sites_by_epoch = BTreeMap::<u32, BTreeSet<ContentHash>>::new();
    for row in &request.rows {
        if !epochs.contains(&row.epoch_index) {
            return Err(ReplayHistoryError::InvalidRequest(
                "every replay row must belong to an expected epoch".into(),
            ));
        }
        let sites = sites_by_epoch.entry(row.epoch_index).or_default();
        if !sites.insert(row.site_commitment.clone()) {
            return Err(ReplayHistoryError::InvalidRequest(
                "site commitments must be unique within each epoch".into(),
            ));
        }
        if sites.len() > MAX_SITES_PER_EPOCH {
            return Err(ReplayHistoryError::InvalidRequest(
                "site commitment count exceeds the per-epoch bound".into(),
            ));
        }
        if !campaign_within_bounds(&row.campaign) {
            return Err(ReplayHistoryError::InvalidRequest(
                "archival campaign fields exceed the bounded aggregation limits".into(),
            ));
        }
        row.campaign
            .validate()
            .map_err(|error| ReplayHistoryError::InvalidCampaign(error.to_string()))?;
        if row.campaign.research_id != request.research_id
            || row.campaign.manifest.manifest_digest != request.manifest_digest
        {
            return Err(ReplayHistoryError::InvalidRequest(
                "each archival campaign must match the requested research and manifest identity"
                    .into(),
            ));
        }
    }
    Ok(())
}

fn bounded_strings(values: &[String], max_items: usize) -> bool {
    values.len() <= max_items
        && values
            .iter()
            .all(|value| !value.is_empty() && value.len() <= MAX_TEXT_BYTES)
}

fn bounded_ids(values: &[String]) -> bool {
    values.len() <= MAX_TASKS
        && values
            .iter()
            .all(|value| !value.is_empty() && value.len() <= MAX_ID_BYTES)
}

fn campaign_within_bounds(campaign: &ReplayCampaign) -> bool {
    if campaign.research_id.len() > MAX_ID_BYTES
        || campaign.objective.len() > MAX_TEXT_BYTES
        || campaign.rounds.len() > MAX_ROUNDS as usize
        || campaign.observations.len() > MAX_TASKS
        || campaign.manifest.research_id.len() > MAX_ID_BYTES
        || campaign.manifest.study_id.len() > MAX_ID_BYTES
        || campaign.manifest.objective.len() > MAX_TEXT_BYTES
        || !bounded_ids(&campaign.manifest.program_order)
        || !bounded_ids(&campaign.manifest.artifact_order)
        || !bounded_strings(&campaign.manifest.negative_evidence, MAX_TASKS)
        || !bounded_strings(&campaign.manifest.limitations, MAX_TASKS)
        || !bounded_ids(&campaign.matched_order)
        || !bounded_ids(&campaign.mismatched_order)
        || !bounded_ids(&campaign.unavailable_order)
        || !bounded_ids(&campaign.failed_order)
        || !bounded_strings(&campaign.negative_evidence, MAX_TASKS)
        || !bounded_strings(&campaign.uncertainty, MAX_TASKS)
    {
        return false;
    }
    for round in &campaign.rounds {
        if !bounded_ids(&round.selected_order)
            || !bounded_ids(&round.matched_order)
            || !bounded_ids(&round.mismatched_order)
            || !bounded_ids(&round.unavailable_order)
            || !bounded_ids(&round.failed_order)
        {
            return false;
        }
    }
    campaign.observations.iter().all(|observation| {
        !observation.task_id.is_empty()
            && observation.task_id.len() <= MAX_ID_BYTES
            && !observation.note.is_empty()
            && observation.note.len() <= MAX_TEXT_BYTES
    })
}

fn seed_digest(
    research_id: &str,
    manifest_digest: &ContentHash,
    expected_epoch_order: &[u32],
    min_sites_per_epoch: u16,
) -> Result<ContentHash, ReplayHistoryError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        research_id,
        manifest_digest,
        expected_epoch_order,
        min_sites_per_epoch,
    ))
}

fn entry_digest(entry: &ReplayHistoryEntry) -> Result<ContentHash, ReplayHistoryError> {
    digest(&(
        FEATURE_ID,
        OUTPUT_SCHEMA,
        entry.epoch_index,
        &entry.site_commitment,
        &entry.campaign_digest,
        entry.disposition,
        entry.coverage_milli,
        &entry.previous_digest,
    ))
}

fn site_disposition(campaign: &ReplayCampaign) -> ReplayHistorySiteDisposition {
    match campaign.disposition {
        ReplayCampaignDisposition::NonReproducible => ReplayHistorySiteDisposition::Divergent,
        ReplayCampaignDisposition::Reproducible if campaign.exact_match => {
            ReplayHistorySiteDisposition::Reproducible
        }
        ReplayCampaignDisposition::Reproducible => ReplayHistorySiteDisposition::Divergent,
        ReplayCampaignDisposition::Partial
        | ReplayCampaignDisposition::Blocked
        | ReplayCampaignDisposition::Unresolved => ReplayHistorySiteDisposition::Unresolved,
    }
}

fn epoch_disposition(
    observed: u16,
    divergent: u16,
    unresolved: u16,
    required: u16,
) -> ReplayHistoryDisposition {
    if divergent > 0 {
        ReplayHistoryDisposition::Divergent
    } else if unresolved > 0 {
        ReplayHistoryDisposition::Unresolved
    } else if observed < required {
        ReplayHistoryDisposition::Insufficient
    } else {
        ReplayHistoryDisposition::Reproducible
    }
}

fn summarize_epochs(
    expected_epoch_order: &[u32],
    min_sites_per_epoch: u16,
    entries: &[ReplayHistoryEntry],
) -> Vec<ReplayHistoryEpochSummary> {
    expected_epoch_order
        .iter()
        .copied()
        .map(|epoch_index| {
            let rows = entries
                .iter()
                .filter(|entry| entry.epoch_index == epoch_index)
                .collect::<Vec<_>>();
            let reproducible_sites = rows
                .iter()
                .filter(|entry| entry.disposition == ReplayHistorySiteDisposition::Reproducible)
                .count() as u16;
            let divergent_sites = rows
                .iter()
                .filter(|entry| entry.disposition == ReplayHistorySiteDisposition::Divergent)
                .count() as u16;
            let unresolved_sites = rows
                .iter()
                .filter(|entry| entry.disposition == ReplayHistorySiteDisposition::Unresolved)
                .count() as u16;
            let observed_sites = rows.len() as u16;
            ReplayHistoryEpochSummary {
                epoch_index,
                required_sites: min_sites_per_epoch,
                observed_sites,
                reproducible_sites,
                divergent_sites,
                unresolved_sites,
                disposition: epoch_disposition(
                    observed_sites,
                    divergent_sites,
                    unresolved_sites,
                    min_sites_per_epoch,
                ),
            }
        })
        .collect()
}

fn overall_disposition(epochs: &[ReplayHistoryEpochSummary]) -> ReplayHistoryDisposition {
    if epochs
        .iter()
        .any(|epoch| epoch.disposition == ReplayHistoryDisposition::Divergent)
    {
        ReplayHistoryDisposition::Divergent
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == ReplayHistoryDisposition::Unresolved)
    {
        ReplayHistoryDisposition::Unresolved
    } else if epochs
        .iter()
        .any(|epoch| epoch.disposition == ReplayHistoryDisposition::Insufficient)
    {
        ReplayHistoryDisposition::Insufficient
    } else {
        ReplayHistoryDisposition::Reproducible
    }
}

fn report_digest(report: &ReplayHistoryReport) -> Result<ContentHash, ReplayHistoryError> {
    digest(&(
        &report.feature_id,
        &report.output_schema,
        &report.research_id,
        &report.manifest_digest,
        &report.expected_epoch_order,
        report.min_sites_per_epoch,
        &report.entries,
        &report.epochs,
        report.disposition,
        &report.chain_head,
    ))
}

/// Reconcile completed archival campaigns without replaying code or moving any study artifacts.
pub fn reconcile_glioma_replay_history(
    request: &ReplayHistoryRequest,
) -> Result<ReplayHistoryReport, ReplayHistoryError> {
    validate_request(request)?;
    let seed = seed_digest(
        &request.research_id,
        &request.manifest_digest,
        &request.expected_epoch_order,
        request.min_sites_per_epoch,
    )?;

    let mut rows = request.rows.iter().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        left.epoch_index
            .cmp(&right.epoch_index)
            .then_with(|| left.site_commitment.cmp(&right.site_commitment))
    });
    let mut previous_digest = seed;
    let mut entries = Vec::with_capacity(rows.len());
    for row in rows {
        let mut entry = ReplayHistoryEntry {
            epoch_index: row.epoch_index,
            site_commitment: row.site_commitment.clone(),
            campaign_digest: row.campaign.digest.clone(),
            disposition: site_disposition(&row.campaign),
            coverage_milli: row.campaign.coverage_milli,
            previous_digest: previous_digest.clone(),
            entry_digest: ContentHash::of_bytes(&[]),
        };
        entry.entry_digest = entry_digest(&entry)?;
        previous_digest = entry.entry_digest.clone();
        entries.push(entry);
    }

    let epochs = summarize_epochs(
        &request.expected_epoch_order,
        request.min_sites_per_epoch,
        &entries,
    );
    let mut report = ReplayHistoryReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.research_id.clone(),
        manifest_digest: request.manifest_digest.clone(),
        expected_epoch_order: request.expected_epoch_order.clone(),
        min_sites_per_epoch: request.min_sites_per_epoch,
        entries,
        disposition: overall_disposition(&epochs),
        epochs,
        chain_head: previous_digest,
        report_digest: ContentHash::of_bytes(&[]),
    };
    report.report_digest = report_digest(&report)?;
    report.validate()?;
    Ok(report)
}

impl ReplayHistoryReport {
    /// Check the canonical epoch summaries, digest chain, and content digest.
    pub fn validate(&self) -> Result<(), ReplayHistoryError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.expected_epoch_order.len() < 2
            || self.expected_epoch_order.len() > MAX_EPOCHS
            || self
                .expected_epoch_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.min_sites_per_epoch < 2
            || usize::from(self.min_sites_per_epoch) > MAX_SITES_PER_EPOCH
            || self.entries.len() > MAX_CAMPAIGN_RECORDS
        {
            return Err(ReplayHistoryError::InvalidOutput(
                "identity, epoch ordering, site quorum, or report bounds are invalid".into(),
            ));
        }
        let expected_epochs = self
            .expected_epoch_order
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut sites_by_epoch = BTreeMap::<u32, BTreeSet<ContentHash>>::new();
        let mut previous_key: Option<(u32, ContentHash)> = None;
        let seed = seed_digest(
            &self.research_id,
            &self.manifest_digest,
            &self.expected_epoch_order,
            self.min_sites_per_epoch,
        )?;
        let mut previous_digest = seed;
        for entry in &self.entries {
            let key = (entry.epoch_index, entry.site_commitment.clone());
            if !expected_epochs.contains(&entry.epoch_index)
                || previous_key
                    .as_ref()
                    .is_some_and(|previous| previous >= &key)
                || !sites_by_epoch
                    .entry(entry.epoch_index)
                    .or_default()
                    .insert(entry.site_commitment.clone())
                || entry.coverage_milli > 1_000
                || entry.previous_digest != previous_digest
                || entry.entry_digest != entry_digest(entry)?
            {
                return Err(ReplayHistoryError::InvalidOutput(
                    "entry ordering, identity, coverage, or digest chain is invalid".into(),
                ));
            }
            if sites_by_epoch[&entry.epoch_index].len() > MAX_SITES_PER_EPOCH {
                return Err(ReplayHistoryError::InvalidOutput(
                    "entry count exceeds the per-epoch bound".into(),
                ));
            }
            previous_key = Some(key);
            previous_digest = entry.entry_digest.clone();
        }
        let epochs = summarize_epochs(
            &self.expected_epoch_order,
            self.min_sites_per_epoch,
            &self.entries,
        );
        if self.epochs != epochs
            || self.disposition != overall_disposition(&epochs)
            || self.chain_head != previous_digest
            || self.report_digest != report_digest(self)?
        {
            return Err(ReplayHistoryError::InvalidOutput(
                "epoch summaries, overall disposition, chain head, or report digest are inconsistent".into(),
            ));
        }
        Ok(())
    }
}
