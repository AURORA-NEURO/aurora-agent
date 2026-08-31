//! Deterministic exponential backoff.
//!
//! No jitter: simulation determinism is worth more than thundering-herd smoothing in a fabric
//! whose whole point is reproducible scheduling. Delay doubles per attempt and saturates at
//! `cap_ticks`; once attempts are exhausted there is no next delay and the task settles as
//! failed — an exhausted policy must be visible as a terminal receipt, not an infinite queue.

use std::fmt;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total attempts allowed *including* the first. 1 means no retries at all.
    pub max_attempts: u32,
    pub base_ticks: u64,
    pub cap_ticks: u64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_ticks: 4,
            cap_ticks: 64,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryDecision {
    /// Wait this many ticks before the next attempt (numbered `attempt + 1`).
    RetryAfter(u64),
    /// The final attempt failed; settle the task now.
    Exhausted,
}

impl RetryPolicy {
    /// Decision after `attempt` (1-based) finished unsuccessfully. Attempt numbers beyond
    /// `max_attempts` report `Exhausted` rather than panicking — callers upstream may drift.
    pub fn after_failure(&self, attempt: u32) -> RetryDecision {
        if attempt >= self.max_attempts {
            return RetryDecision::Exhausted;
        }
        let shift = (attempt - 1).min(63);
        let delay = self
            .base_ticks
            .checked_shl(shift)
            .unwrap_or(self.cap_ticks)
            .min(self.cap_ticks);
        RetryDecision::RetryAfter(delay.max(1))
    }

    pub fn allows_attempt(&self, attempt: u32) -> bool {
        attempt <= self.max_attempts && attempt >= 1
    }

    pub fn to_duration_hint(&self, ticks_are_ms: bool) -> Duration {
        Duration::from_millis(if ticks_are_ms {
            self.base_ticks
        } else {
            self.base_ticks * 1000
        })
    }
}

impl fmt::Display for RetryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "max {} attempts, backoff {}..{} ticks",
            self.max_attempts, self.base_ticks, self.cap_ticks
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_saturates_at_the_cap() {
        let p = RetryPolicy {
            max_attempts: 10,
            base_ticks: 4,
            cap_ticks: 32,
        };
        assert_eq!(p.after_failure(1), RetryDecision::RetryAfter(4));
        assert_eq!(p.after_failure(2), RetryDecision::RetryAfter(8));
        assert_eq!(p.after_failure(3), RetryDecision::RetryAfter(16));
        assert_eq!(p.after_failure(4), RetryDecision::RetryAfter(32));
        assert_eq!(
            p.after_failure(5),
            RetryDecision::RetryAfter(32),
            "saturated"
        );
    }

    #[test]
    fn exhausting_the_budget_is_a_terminal_decision_not_another_delay() {
        let p = RetryPolicy {
            max_attempts: 3,
            ..Default::default()
        };
        assert_eq!(p.after_failure(2), RetryDecision::RetryAfter(8));
        assert_eq!(p.after_failure(3), RetryDecision::Exhausted);
        assert_eq!(
            p.after_failure(99),
            RetryDecision::Exhausted,
            "drift tolerated"
        );
    }

    #[test]
    fn a_no_retry_policy_never_schedules_a_second_attempt() {
        let p = RetryPolicy {
            max_attempts: 1,
            ..Default::default()
        };
        assert_eq!(p.after_failure(1), RetryDecision::Exhausted);
        assert!(!p.allows_attempt(2));
    }

    #[test]
    fn shifts_cannot_overflow_into_garbage_delays() {
        let p = RetryPolicy {
            max_attempts: u32::MAX,
            base_ticks: u64::MAX / 2,
            cap_ticks: 7,
        };
        assert_eq!(p.after_failure(40), RetryDecision::RetryAfter(7));
    }
}
