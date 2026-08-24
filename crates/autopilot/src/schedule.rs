//! Deterministic retry timing for the grant-authorised autopilot.
//!
//! The kernel does not own a clock, thread, executor, or wall-clock unit. A grant may authorize
//! a bounded exponential delay for repair dispatches, and the caller supplies the wait seam. This
//! keeps scheduling testable and makes a restart resume the same delay sequence from the retained
//! attempt count rather than from process-local timers.

use crate::error::GrantError;
use serde::{Deserialize, Serialize};

/// Maximum delay represented by one schedule, in caller-defined logical clock ticks.
pub const MAX_RETRY_DELAY_TICKS: u64 = 31_536_000;

fn default_zero() -> u64 {
    0
}

/// Serde-facing retry timing policy. Zero values preserve the historical immediate-retry
/// behavior; a non-zero base requires a non-zero ceiling at least as large as the base.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetryScheduleDocument {
    /// Delay before the first repair dispatch, in caller-defined logical ticks.
    #[serde(default = "default_zero")]
    pub retry_base_delay: u64,
    /// Exponential-backoff ceiling, in the same logical ticks.
    #[serde(default = "default_zero")]
    pub retry_max_delay: u64,
}

impl Default for RetryScheduleDocument {
    fn default() -> Self {
        Self {
            retry_base_delay: 0,
            retry_max_delay: 0,
        }
    }
}

/// Validated immutable retry timing policy carried by an [`AutonomyGrant`](crate::AutonomyGrant).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetrySchedule {
    retry_base_delay: u64,
    retry_max_delay: u64,
}

impl RetrySchedule {
    pub fn retry_base_delay(&self) -> u64 {
        self.retry_base_delay
    }

    pub fn retry_max_delay(&self) -> u64 {
        self.retry_max_delay
    }

    /// Return the delay before a repair whose one-based retry index is supplied. Index `1` is
    /// the first repair after the initial full dispatch. Saturating arithmetic and the validated
    /// ceiling make this safe for arbitrarily large history indexes.
    pub fn delay_for_retry(&self, retry_index: usize) -> u64 {
        if retry_index == 0 || self.retry_base_delay == 0 {
            return 0;
        }
        let shift = retry_index.saturating_sub(1).min(63) as u32;
        let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
        self.retry_base_delay
            .saturating_mul(multiplier)
            .min(self.retry_max_delay)
    }
}

impl TryFrom<RetryScheduleDocument> for RetrySchedule {
    type Error = GrantError;

    fn try_from(document: RetryScheduleDocument) -> Result<Self, Self::Error> {
        if document.retry_base_delay > MAX_RETRY_DELAY_TICKS
            || document.retry_max_delay > MAX_RETRY_DELAY_TICKS
        {
            return Err(GrantError::InvalidRetrySchedule {
                base: document.retry_base_delay,
                maximum: document.retry_max_delay,
                ceiling: MAX_RETRY_DELAY_TICKS,
            });
        }
        if document.retry_base_delay == 0 && document.retry_max_delay != 0 {
            return Err(GrantError::InvalidRetrySchedule {
                base: document.retry_base_delay,
                maximum: document.retry_max_delay,
                ceiling: MAX_RETRY_DELAY_TICKS,
            });
        }
        if document.retry_base_delay > 0 && document.retry_max_delay < document.retry_base_delay {
            return Err(GrantError::InvalidRetrySchedule {
                base: document.retry_base_delay,
                maximum: document.retry_max_delay,
                ceiling: MAX_RETRY_DELAY_TICKS,
            });
        }
        Ok(Self {
            retry_base_delay: document.retry_base_delay,
            retry_max_delay: document.retry_max_delay,
        })
    }
}

impl From<RetrySchedule> for RetryScheduleDocument {
    fn from(schedule: RetrySchedule) -> Self {
        Self {
            retry_base_delay: schedule.retry_base_delay,
            retry_max_delay: schedule.retry_max_delay,
        }
    }
}

/// Caller-owned waiting boundary. The implementation may sleep, enqueue work for a future
/// worker tick, or record a deterministic virtual-clock event. It must not mutate the grant or
/// dispatch a mission itself.
pub trait AutopilotWait {
    fn wait_for(&mut self, delay_ticks: u64) -> Result<(), String>;
}

impl<F> AutopilotWait for F
where
    F: FnMut(u64) -> Result<(), String>,
{
    fn wait_for(&mut self, delay_ticks: u64) -> Result<(), String> {
        self(delay_ticks)
    }
}
