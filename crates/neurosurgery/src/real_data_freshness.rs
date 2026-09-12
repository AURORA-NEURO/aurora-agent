//! Deterministic freshness posture for caller-supplied public evidence snapshots.
//!
//! A real-data agent must be able to say how old its source material is without turning age into a
//! quality, applicability, or clinical-relevance score. This module compares source retrieval
//! timestamps with an explicit caller-supplied UTC `as_of` time, reports stale and future-dated
//! rows separately, and binds the result to the validated bundle digest. It never uses the host
//! clock, fetches a URL, or changes the underlying snapshot.

use crate::{NeurosurgeryError, PublicLiteratureBundle, RealGliomaBundle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const REAL_DATA_FRESHNESS_SCHEMA_VERSION: &str =
    "bioprism-neurosurgery-real-data-freshness/0.1";
const MAX_MAX_AGE_DAYS: u32 = 3_650;

/// Explicit as-of policy for a freshness audit. The caller supplies the clock so the report is
/// replayable and cannot drift because a machine was run on a different day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataFreshnessQuery {
    pub as_of: String,
    /// A source is current when its retrieval timestamp is no older than this many whole UTC
    /// days. This is a review-policy bound, not a study-quality threshold.
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u32,
    /// Optional exact source facet. Omitting it audits every source in the validated bundle.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

fn default_max_age_days() -> u32 {
    365
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataFreshnessState {
    Current,
    Stale,
    FutureDated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RealDataFreshnessStatus {
    Current,
    Stale,
    RequiresReview,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataFreshnessSource {
    pub source_id: String,
    pub retrieved_at: String,
    pub declared_record_count: usize,
    pub age_days: Option<u64>,
    pub state: RealDataFreshnessState,
}

/// Digest-bound source-age report for one validated snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealDataFreshnessReport {
    pub schema_version: String,
    pub bundle_digest: String,
    pub generated_at: String,
    pub query: RealDataFreshnessQuery,
    pub status: RealDataFreshnessStatus,
    pub source_count: usize,
    pub current_source_count: usize,
    pub stale_source_count: usize,
    pub future_dated_source_count: usize,
    pub sources: Vec<RealDataFreshnessSource>,
    pub freshness_digest: String,
    pub provenance_bound: bool,
    pub synthetic_data: bool,
    pub human_review_required: bool,
    pub provider: String,
    pub network: bool,
    pub effect: String,
    pub limitations: Vec<String>,
}

impl RealDataFreshnessReport {
    /// Validate a persisted source-age posture without consulting the host clock.
    pub fn validate_integrity(&self) -> Result<(), NeurosurgeryError> {
        if self.schema_version != REAL_DATA_FRESHNESS_SCHEMA_VERSION
            || !is_sha256_hex(&self.freshness_digest)
            || !is_sha256_hex(&self.bundle_digest)
            || !is_utc_timestamp(&self.generated_at)
            || !is_utc_timestamp(&self.query.as_of)
            || self.query.max_age_days > MAX_MAX_AGE_DAYS
            || self.query.source_id.as_deref().is_some_and(str::is_empty)
            || self.source_count != self.sources.len()
            || self
                .current_source_count
                .saturating_add(self.stale_source_count)
                .saturating_add(self.future_dated_source_count)
                != self.source_count
            || self
                .sources
                .windows(2)
                .any(|window| window[0].source_id >= window[1].source_id)
            || self.sources.iter().any(|source| {
                source.source_id.trim().is_empty()
                    || !is_utc_timestamp(&source.retrieved_at)
                    || self
                        .query
                        .source_id
                        .as_deref()
                        .is_some_and(|id| id != source.source_id)
                    || !freshness_state_matches(source, &self.query.as_of, self.query.max_age_days)
            })
            || self.status
                != expected_status(self.future_dated_source_count, self.stale_source_count)
            || !self.provenance_bound
            || self.synthetic_data
            || !self.human_review_required
            || self.provider != "none"
            || self.network
            || self.effect != "read_only"
            || self.limitations.is_empty()
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data freshness report is invalid".to_string(),
            });
        }
        if self.freshness_digest
            != digest_report(
                &self.bundle_digest,
                &self.generated_at,
                &self.query,
                self.status,
                &self.sources,
            )?
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data freshness digest does not match its contents".to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild a freshness report from an exact real-glioma snapshot.
    pub fn validate_for_real_inputs(
        &self,
        bundle: &RealGliomaBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.freshness_report(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: "real-data freshness report does not replay to the exact supplied snapshot"
                    .to_string(),
            });
        }
        Ok(())
    }

    /// Rebuild a freshness report from an exact cross-specialty public-literature snapshot.
    pub fn validate_for_public_inputs(
        &self,
        bundle: &PublicLiteratureBundle,
    ) -> Result<(), NeurosurgeryError> {
        self.validate_integrity()?;
        let expected = bundle.freshness_report(&self.query)?;
        if &expected != self {
            return Err(NeurosurgeryError::RealDataRejected {
                reason:
                    "public-literature freshness report does not replay to the exact supplied snapshot"
                        .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FreshnessDigestInput<'a> {
    bundle_digest: &'a str,
    generated_at: &'a str,
    query: &'a RealDataFreshnessQuery,
    status: RealDataFreshnessStatus,
    sources: &'a [RealDataFreshnessSource],
}

#[derive(Debug, Clone, Copy)]
struct SourceTimestamp<'a> {
    source_id: &'a str,
    retrieved_at: &'a str,
    declared_record_count: usize,
}

impl RealGliomaBundle {
    /// Audit source ages in a validated real-glioma bundle using an explicit caller-owned clock.
    pub fn freshness_report(
        &self,
        query: &RealDataFreshnessQuery,
    ) -> Result<RealDataFreshnessReport, NeurosurgeryError> {
        self.validate()?;
        build_report(
            self.summary()?.bundle_digest,
            &self.generated_at,
            query,
            self.sources.iter().map(|source| SourceTimestamp {
                source_id: &source.source_id,
                retrieved_at: &source.retrieved_at,
                declared_record_count: source.record_count,
            }),
            "real glioma",
        )
    }
}

impl PublicLiteratureBundle {
    /// Audit source ages in a validated cross-specialty PubMed bundle using an explicit clock.
    pub fn freshness_report(
        &self,
        query: &RealDataFreshnessQuery,
    ) -> Result<RealDataFreshnessReport, NeurosurgeryError> {
        self.validate()?;
        build_report(
            self.summary()?.bundle_digest,
            &self.generated_at,
            query,
            self.sources.iter().map(|source| SourceTimestamp {
                source_id: &source.source_id,
                retrieved_at: &source.retrieved_at,
                declared_record_count: source.record_count,
            }),
            "public literature",
        )
    }
}

fn build_report<'a, I>(
    bundle_digest: String,
    generated_at: &str,
    query: &RealDataFreshnessQuery,
    sources: I,
    bundle_label: &str,
) -> Result<RealDataFreshnessReport, NeurosurgeryError>
where
    I: IntoIterator<Item = SourceTimestamp<'a>>,
{
    let source_rows = sources.into_iter().collect::<Vec<_>>();
    let available_source_ids = source_rows
        .iter()
        .map(|source| source.source_id)
        .collect::<Vec<_>>();
    validate_query(query, generated_at, &available_source_ids, bundle_label)?;
    let as_of_seconds =
        timestamp_seconds(&query.as_of).ok_or_else(|| NeurosurgeryError::RealDataRejected {
            reason: "freshness as_of must be a valid UTC timestamp".to_string(),
        })?;
    let mut rows = source_rows
        .into_iter()
        .filter(|source| {
            query
                .source_id
                .as_deref()
                .is_none_or(|source_id| source.source_id == source_id)
        })
        .map(|source| {
            let retrieved_seconds = timestamp_seconds(source.retrieved_at)
                .expect("validated source timestamps use the shared UTC grammar");
            let delta_seconds = as_of_seconds - retrieved_seconds;
            let (age_days, state) = if delta_seconds < 0 {
                (None, RealDataFreshnessState::FutureDated)
            } else {
                let age_days = (delta_seconds / 86_400) as u64;
                let state = if age_days <= u64::from(query.max_age_days) {
                    RealDataFreshnessState::Current
                } else {
                    RealDataFreshnessState::Stale
                };
                (Some(age_days), state)
            };
            RealDataFreshnessSource {
                source_id: source.source_id.to_string(),
                retrieved_at: source.retrieved_at.to_string(),
                declared_record_count: source.declared_record_count,
                age_days,
                state,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| left.source_id.cmp(&right.source_id));
    if rows.is_empty() {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "freshness query selected no sources".to_string(),
        });
    }
    let current_source_count = rows
        .iter()
        .filter(|source| source.state == RealDataFreshnessState::Current)
        .count();
    let stale_source_count = rows
        .iter()
        .filter(|source| source.state == RealDataFreshnessState::Stale)
        .count();
    let future_dated_source_count = rows
        .iter()
        .filter(|source| source.state == RealDataFreshnessState::FutureDated)
        .count();
    let status = if future_dated_source_count > 0 {
        RealDataFreshnessStatus::RequiresReview
    } else if stale_source_count > 0 {
        RealDataFreshnessStatus::Stale
    } else {
        RealDataFreshnessStatus::Current
    };
    let freshness_digest =
        digest_report(bundle_digest.as_str(), generated_at, query, status, &rows)?;
    let report = RealDataFreshnessReport {
        schema_version: REAL_DATA_FRESHNESS_SCHEMA_VERSION.to_string(),
        bundle_digest,
        generated_at: generated_at.to_string(),
        query: query.clone(),
        status,
        source_count: rows.len(),
        current_source_count,
        stale_source_count,
        future_dated_source_count,
        sources: rows,
        freshness_digest,
        provenance_bound: true,
        synthetic_data: false,
        human_review_required: true,
        provider: "none".to_string(),
        network: false,
        effect: "read_only".to_string(),
        limitations: vec![
            "freshness compares source retrieval timestamps with the caller's explicit as_of time; it does not assess evidence quality, applicability, completeness, or clinical relevance".to_string(),
            "max_age_days is a caller-owned review policy and is not a medical or regulatory threshold".to_string(),
            "future-dated source metadata is retained as requires_review rather than coerced into current".to_string(),
            "the report never fetches URLs, invokes a model, opens credentials, exposes patient/sample values, or writes durable state".to_string(),
        ],
    };
    report.validate_integrity()?;
    Ok(report)
}

fn validate_query(
    query: &RealDataFreshnessQuery,
    generated_at: &str,
    source_ids: &[&str],
    bundle_label: &str,
) -> Result<(), NeurosurgeryError> {
    if !is_utc_timestamp(&query.as_of) {
        return Err(NeurosurgeryError::RealDataRejected {
            reason: "freshness as_of must use YYYY-MM-DDTHH:MM:SSZ".to_string(),
        });
    }
    if query.max_age_days > MAX_MAX_AGE_DAYS {
        return Err(NeurosurgeryError::TooMany {
            field: "freshness.max_age_days",
            found: query.max_age_days as usize,
            max: MAX_MAX_AGE_DAYS as usize,
        });
    }
    if let Some(source_id) = query.source_id.as_deref() {
        if source_id.is_empty()
            || source_id.len() > 512
            || source_id.chars().any(char::is_control)
            || !source_ids.contains(&source_id)
        {
            return Err(NeurosurgeryError::RealDataRejected {
                reason: format!(
                    "freshness source_id {source_id:?} is invalid or absent from the {bundle_label} bundle"
                ),
            });
        }
    }
    // Historical as-of queries are allowed: a source retrieved after that time is explicitly
    // reported as future_dated/requires_review rather than silently treated as current.
    let _ = timestamp_seconds(&query.as_of).ok_or_else(|| NeurosurgeryError::RealDataRejected {
        reason: "freshness as_of must be a valid UTC timestamp".to_string(),
    })?;
    let _ = timestamp_seconds(generated_at).ok_or_else(|| NeurosurgeryError::RealDataRejected {
        reason: "bundle generated_at must be a valid UTC timestamp".to_string(),
    })?;
    Ok(())
}

fn is_utc_timestamp(value: &str) -> bool {
    crate::temporal::is_utc_timestamp(value)
}

fn timestamp_seconds(value: &str) -> Option<i64> {
    if !is_utc_timestamp(value) {
        return None;
    }
    let bytes = value.as_bytes();
    let year = i64::from(
        u16::from(bytes[0] - b'0') * 1_000
            + u16::from(bytes[1] - b'0') * 100
            + u16::from(bytes[2] - b'0') * 10
            + u16::from(bytes[3] - b'0'),
    );
    let month = i64::from((bytes[5] - b'0') * 10 + (bytes[6] - b'0'));
    let day = i64::from((bytes[8] - b'0') * 10 + (bytes[9] - b'0'));
    let hour = i64::from((bytes[11] - b'0') * 10 + (bytes[12] - b'0'));
    let minute = i64::from((bytes[14] - b'0') * 10 + (bytes[15] - b'0'));
    let second = i64::from((bytes[17] - b'0') * 10 + (bytes[18] - b'0'));
    Some(days_from_civil(year, month, day) * 86_400 + hour * 3_600 + minute * 60 + second)
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = year - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn digest_report(
    bundle_digest: &str,
    generated_at: &str,
    query: &RealDataFreshnessQuery,
    status: RealDataFreshnessStatus,
    sources: &[RealDataFreshnessSource],
) -> Result<String, NeurosurgeryError> {
    let bytes = serde_json::to_vec(&FreshnessDigestInput {
        bundle_digest,
        generated_at,
        query,
        status,
        sources,
    })
    .map_err(|error| NeurosurgeryError::Digest(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn expected_status(
    future_dated_source_count: usize,
    stale_source_count: usize,
) -> RealDataFreshnessStatus {
    if future_dated_source_count > 0 {
        RealDataFreshnessStatus::RequiresReview
    } else if stale_source_count > 0 {
        RealDataFreshnessStatus::Stale
    } else {
        RealDataFreshnessStatus::Current
    }
}

fn freshness_state_matches(
    source: &RealDataFreshnessSource,
    as_of: &str,
    max_age_days: u32,
) -> bool {
    let Some(as_of_seconds) = timestamp_seconds(as_of) else {
        return false;
    };
    let Some(retrieved_seconds) = timestamp_seconds(&source.retrieved_at) else {
        return false;
    };
    let delta_seconds = as_of_seconds - retrieved_seconds;
    if delta_seconds < 0 {
        source.age_days.is_none() && source.state == RealDataFreshnessState::FutureDated
    } else {
        let age_days = (delta_seconds / 86_400) as u64;
        let state = if age_days <= u64::from(max_age_days) {
            RealDataFreshnessState::Current
        } else {
            RealDataFreshnessState::Stale
        };
        source.age_days == Some(age_days) && source.state == state
    }
}

fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
}
