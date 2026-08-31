//! Exclusive task leases with TTL expiry and affine handles.
//!
//! The lease table is where "no duplicate assignment" stops being a promise and becomes a
//! structure: at most one live lease per task exists because [`LeaseTable::grant`] refuses while
//! one is held, and the returned [`LeaseHandle`] is deliberately **not `Clone`** — releasing or
//! renewing consumes it, so double-release and grant-then-forget are compile errors, not runtime
//! audits. Expiry is how the fabric detects crashed or silently-dead workers: the lease simply
//! outlives its TTL and the task returns to the retry path.

use crate::ids::{AgentId, LeaseEpoch, TaskId};
use std::collections::BTreeMap;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeaseError {
    /// Another agent holds a live lease on the task.
    HeldByOther {
        holder: AgentId,
    },
    /// The presented handle's epoch no longer matches — the lease expired and was re-granted.
    EpochMismatch,
    UnknownTask,
}

impl fmt::Display for LeaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LeaseError::HeldByOther { holder } => write!(f, "task already leased to {holder}"),
            LeaseError::EpochMismatch => write!(f, "lease epoch no longer current"),
            LeaseError::UnknownTask => write!(f, "no lease recorded for this task"),
        }
    }
}

impl std::error::Error for LeaseError {}

/// Ownership token for one live lease. Not `Clone`, not `Copy`: holding one *is* holding the
/// exclusive right, and rights that can be duplicated are not exclusive.
#[derive(Debug, PartialEq, Eq)]
pub struct LeaseHandle {
    pub(crate) task: TaskId,
    pub(crate) agent: AgentId,
    pub(crate) epoch: LeaseEpoch,
}

impl LeaseHandle {
    pub fn task(&self) -> TaskId {
        self.task
    }

    pub fn agent(&self) -> AgentId {
        self.agent
    }

    pub fn epoch(&self) -> LeaseEpoch {
        self.epoch
    }
}

#[derive(Debug)]
struct Active {
    agent: AgentId,
    epoch: LeaseEpoch,
    expires_at: u64,
}

/// A released or expired lease, reported back so the scheduler can retry with full information.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExpiredLease {
    pub task: TaskId,
    pub agent: AgentId,
    pub ended_at_tick: u64,
}

#[derive(Debug, Default)]
pub struct LeaseTable {
    by_task: BTreeMap<TaskId, Active>,
    next_epoch: u64,
}

impl LeaseTable {
    pub fn new() -> Self {
        Self::default()
    }

    /// Grants an exclusive lease or fails with the current holder. Monotone epochs mean a stale
    /// handle from an expired generation can never be mistaken for the current one.
    pub fn grant(
        &mut self,
        task: TaskId,
        agent: AgentId,
        now: u64,
        ttl_ticks: u64,
    ) -> Result<LeaseHandle, LeaseError> {
        assert!(ttl_ticks > 0, "a zero-ttl lease would expire before use");
        if let Some(active) = self.by_task.get(&task) {
            return Err(LeaseError::HeldByOther {
                holder: active.agent,
            });
        }
        self.next_epoch += 1;
        let epoch = LeaseEpoch::new(self.next_epoch);
        self.by_task.insert(
            task,
            Active {
                agent,
                epoch,
                expires_at: now + ttl_ticks,
            },
        );
        Ok(LeaseHandle { task, agent, epoch })
    }

    /// Extends a live lease; consumes and re-mints the handle so exactly one token remains.
    pub fn renew(
        &mut self,
        h: LeaseHandle,
        now: u64,
        ttl_ticks: u64,
    ) -> Result<LeaseHandle, LeaseError> {
        assert!(ttl_ticks > 0, "a zero-ttl lease would expire before use");
        let Some(active) = self.by_task.get_mut(&h.task) else {
            return Err(LeaseError::UnknownTask);
        };
        if active.epoch != h.epoch {
            return Err(LeaseError::EpochMismatch);
        }
        active.expires_at = now + ttl_ticks;
        Ok(LeaseHandle {
            task: h.task,
            agent: h.agent,
            epoch: h.epoch,
        })
    }

    /// Ends the lease. The handle is consumed; there is no way to release twice.
    pub fn release(&mut self, h: LeaseHandle) -> Result<(), LeaseError> {
        self.release_by(h.task(), h.epoch())
    }

    /// Epoch-keyed release for callers that tracked the epoch without retaining the affine
    /// handle (the settlement path). A stale epoch is rejected, so a late completion cannot
    /// free a lease it no longer owns.
    pub fn release_by(&mut self, task: TaskId, epoch: LeaseEpoch) -> Result<(), LeaseError> {
        match self.by_task.get(&task) {
            None => Err(LeaseError::UnknownTask),
            Some(a) if a.epoch != epoch => Err(LeaseError::EpochMismatch),
            Some(_) => {
                self.by_task.remove(&task);
                Ok(())
            }
        }
    }

    /// Drops every lease whose TTL has passed by `now` and reports them for retry handling.
    pub fn expire_before(&mut self, now: u64) -> Vec<ExpiredLease> {
        let due: Vec<TaskId> = self
            .by_task
            .iter()
            .filter(|(_, a)| a.expires_at <= now)
            .map(|(t, _)| *t)
            .collect();
        due.into_iter()
            .filter_map(|t| {
                let a = self.by_task.remove(&t)?;
                Some(ExpiredLease {
                    task: t,
                    agent: a.agent,
                    ended_at_tick: a.expires_at,
                })
            })
            .collect()
    }

    pub fn holder(&self, task: TaskId) -> Option<AgentId> {
        self.by_task.get(&task).map(|a| a.agent)
    }

    pub fn live(&self) -> usize {
        self.by_task.len()
    }

    pub fn expiry_of(&self, task: TaskId) -> Option<u64> {
        self.by_task.get(&task).map(|a| a.expires_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(t: u64, a: u64) -> (TaskId, AgentId) {
        (
            TaskId::from_raw(t).expect("nonzero"),
            AgentId::from_raw(a).expect("nonzero"),
        )
    }

    #[test]
    fn a_second_grant_while_held_names_the_holder_rather_than_succeeding() {
        let mut t = LeaseTable::new();
        let (task, x, y) = (ids(1, 1).0, ids(1, 1).1, ids(1, 2).1);
        let h = t.grant(task, x, 0, 10).expect("first grant");
        assert_eq!(
            t.grant(task, y, 1, 10),
            Err(LeaseError::HeldByOther { holder: x })
        );
        t.release(h).expect("release");
        assert!(t.grant(task, y, 2, 10).is_ok(), "free after release");
    }

    #[test]
    fn a_consumed_handle_cannot_release_twice_because_ownership_moves() {
        let mut t = LeaseTable::new();
        let (task, x) = ids(5, 5);
        let h = t.grant(task, x, 0, 10).expect("grant");
        t.release(h).expect("first release");
        // `h` has been moved into release(); a second release(h) would not compile. The
        // observable consequence is asserted instead: nothing is left to release.
        assert_eq!(t.live(), 0);
        assert_eq!(t.holder(task), None);
    }

    #[test]
    fn renewal_consumes_the_old_handle_and_keeps_exactly_one_live_token() {
        let mut t = LeaseTable::new();
        let (task, x) = ids(7, 7);
        let h1 = t.grant(task, x, 0, 10).expect("grant");
        let h2 = t.renew(h1, 5, 20).expect("renew");
        assert_eq!(t.expiry_of(task), Some(25));
        t.release(h2).expect("release renewed");
        assert_eq!(t.live(), 0);
    }

    #[test]
    fn an_expired_generation_handle_is_rejected_not_treated_as_current() {
        let mut t = LeaseTable::new();
        let (task, x) = ids(9, 9);
        let h1 = t.grant(task, x, 0, 1).expect("grant");
        let old_epoch = h1.epoch();
        let expired = t.expire_before(5);
        assert_eq!(expired.len(), 1);
        assert_eq!(t.renew(h1, 6, 10), Err(LeaseError::UnknownTask));
        // Re-grant after expiry mints a new epoch; the old handle stays invalid forever.
        let h2 = t.grant(task, x, 6, 10).expect("re-grant");
        assert_ne!(old_epoch, h2.epoch());
    }

    #[test]
    fn expiry_reports_only_what_actually_lapsed_with_its_end_tick() {
        let mut t = LeaseTable::new();
        let (t1, a1) = ids(11, 11);
        let (t2, a2) = ids(12, 12);
        t.grant(t1, a1, 0, 10).expect("g1");
        t.grant(t2, a2, 0, 100).expect("g2");
        let out = t.expire_before(10);
        assert_eq!(
            out,
            vec![ExpiredLease {
                task: t1,
                agent: a1,
                ended_at_tick: 10
            }]
        );
        assert_eq!(t.live(), 1);
    }
}
