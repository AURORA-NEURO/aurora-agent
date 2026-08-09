//! Derived views over the log, and the checkpoints that stop them replaying from genesis.
//!
//! Blueprint 40.09 lists "updated projections" as an output and "projection checkpoints" as
//! persisted state, and puts "projection rebuild equality" in the verification plan. That last
//! item is the one that matters, because a checkpoint is a promise: *this state is what you
//! would have got by folding every entry from the beginning*. If the promise is false the
//! ledger is still append-only and still tamper-evident, and every number the platform reports
//! is still wrong.
//!
//! So [`Checkpoint`] commits to three things — the sequence it stopped at, the digest of the
//! entry at that sequence, and a digest of the state itself. Resuming re-derives all three
//! before applying anything, which turns "you gave me a checkpoint from a different log" from a
//! silently wrong answer into [`LedgerError::CheckpointDivergence`].
//!
//! ## What is checkpointable, and what is not
//!
//! Checkpoints apply to *sequence prefixes* only. As-of projections ([`EventLedger::project_cut`])
//! are a filter over the whole log, not a prefix of it, because record time is data supplied by
//! the caller and may go backwards (see [`EventLedger::clock_consistency`]). Pretending
//! otherwise would let a late-arriving backfill land behind a checkpoint and never be folded in
//! — a leak that would surface as a quietly stale number rather than as an error. On a log with
//! no clock anomalies a record-time cut *is* a prefix; that fact is reported but deliberately
//! not exploited, because the ledger cannot promise a log will stay anomaly-free.
//!
//! Deliberately not implemented: incremental invalidation, subscriptions, and an outbox worker.
//! Those need a scheduler, and this crate has no threads.

use crate::entry::LedgerEntry;
use crate::error::LedgerError;
use crate::event::{EventClass, SubjectKey};
use crate::ledger::EventLedger;
use crate::time::{TemporalCut, ValidTime};
use bioprism_ids::{ContentHash, EventId};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A fold over ledger entries.
///
/// `apply` takes entries in sequence order and must be a pure function of the state and the
/// entry — no clock, no randomness, no ambient state. That is what makes rebuild equality
/// provable rather than merely likely.
pub trait Projection {
    type State: Clone + PartialEq + Serialize + DeserializeOwned;

    /// Stable name, recorded in checkpoints so one cannot be resumed into a different fold.
    fn name(&self) -> &str;

    fn empty(&self) -> Self::State;

    fn apply(&self, state: &mut Self::State, entry: &LedgerEntry);
}

/// A projection state together with where in the log it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectionRun<S> {
    pub projection: String,
    pub state: S,
    /// Highest sequence folded in, or `None` if nothing was.
    pub through_seq: Option<u64>,
    /// Digest of the entry at `through_seq`, empty when nothing was folded.
    pub head_digest: String,
    pub applied: u64,
}

impl<S: Clone + PartialEq + Serialize + DeserializeOwned> ProjectionRun<S> {
    /// Freezes this run so a later rebuild can start here.
    ///
    /// Returns `None` for a run that folded nothing: a checkpoint at "before the beginning" is
    /// just an empty state, and handing one out invites a caller to treat it as progress.
    pub fn checkpoint(&self) -> Result<Option<Checkpoint<S>>, LedgerError> {
        let Some(through_seq) = self.through_seq else {
            return Ok(None);
        };
        let state_digest = digest_state(&self.projection, &self.state)?;
        Ok(Some(Checkpoint {
            projection: self.projection.clone(),
            through_seq,
            head_digest: self.head_digest.clone(),
            state: self.state.clone(),
            state_digest,
        }))
    }
}

/// A projection state pinned to a point in the log, with enough commitments to be checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint<S> {
    pub projection: String,
    pub through_seq: u64,
    /// Digest of the ledger entry at `through_seq`. Binds the checkpoint to one specific
    /// history, not merely to a sequence number that any log could supply.
    pub head_digest: String,
    pub state: S,
    pub state_digest: String,
}

impl<S: Clone + PartialEq + Serialize + DeserializeOwned> Checkpoint<S> {
    /// Confirms the carried state still hashes to the digest recorded beside it.
    pub fn verify_state(&self) -> Result<(), LedgerError> {
        let recomputed = digest_state(&self.projection, &self.state)?;
        if recomputed == self.state_digest {
            Ok(())
        } else {
            Err(LedgerError::CheckpointStateMismatch {
                carried: self.state_digest.clone(),
                recomputed,
            })
        }
    }
}

fn digest_state<S: Serialize>(projection: &str, state: &S) -> Result<String, LedgerError> {
    let value = serde_json::to_value(state).map_err(|error| LedgerError::StateNotSerializable {
        projection: projection.to_string(),
        detail: error.to_string(),
    })?;
    Ok(ContentHash::of_value(&value)?.as_str().to_string())
}

impl EventLedger {
    /// Folds the whole log from genesis.
    pub fn project<P: Projection>(&self, projection: &P) -> ProjectionRun<P::State> {
        let mut run = ProjectionRun {
            projection: projection.name().to_string(),
            state: projection.empty(),
            through_seq: None,
            head_digest: String::new(),
            applied: 0,
        };
        for entry in self.entries() {
            projection.apply(&mut run.state, entry);
            run.through_seq = Some(entry.seq);
            run.head_digest = entry.digest.clone();
            run.applied += 1;
        }
        run
    }

    /// Folds the log up to and including a chosen sequence number.
    ///
    /// Checkpointing at the head is the common case, but an operator restoring a projection to
    /// a known-good point needs to freeze it somewhere behind the head, and a rebuild-equality
    /// test needs to compare two folds that stopped in different places.
    pub fn project_through<P: Projection>(
        &self,
        projection: &P,
        through_seq: u64,
    ) -> ProjectionRun<P::State> {
        let mut run = ProjectionRun {
            projection: projection.name().to_string(),
            state: projection.empty(),
            through_seq: None,
            head_digest: String::new(),
            applied: 0,
        };
        for entry in self
            .entries()
            .iter()
            .filter(|entry| entry.seq <= through_seq)
        {
            projection.apply(&mut run.state, entry);
            run.through_seq = Some(entry.seq);
            run.head_digest = entry.digest.clone();
            run.applied += 1;
        }
        run
    }

    /// Folds only the entries after a checkpoint, having first proved the checkpoint belongs to
    /// this log.
    ///
    /// The equality this is supposed to preserve — resume equals genesis rebuild — holds on a
    /// log that has not been compacted since the checkpoint was taken. After compaction it does
    /// not, and cannot: a genesis rebuild of a compacted log folds fewer entries than the
    /// checkpoint already absorbed. The resumed state is the *more* complete of the two, which
    /// is why this is allowed rather than refused, but a caller comparing the two will see a
    /// difference and should not be surprised by it.
    pub fn resume<P: Projection>(
        &self,
        projection: &P,
        checkpoint: &Checkpoint<P::State>,
    ) -> Result<ProjectionRun<P::State>, LedgerError> {
        if checkpoint.projection != projection.name() {
            return Err(LedgerError::CheckpointDivergence {
                seq: checkpoint.through_seq,
                expected: checkpoint.projection.clone(),
                found: projection.name().to_string(),
            });
        }
        checkpoint.verify_state()?;

        let anchor = self
            .entries()
            .iter()
            .find(|entry| entry.seq == checkpoint.through_seq);
        match anchor {
            Some(entry) if entry.digest == checkpoint.head_digest => {}
            Some(entry) => {
                return Err(LedgerError::CheckpointDivergence {
                    seq: checkpoint.through_seq,
                    expected: checkpoint.head_digest.clone(),
                    found: entry.digest.clone(),
                });
            }
            None if checkpoint.through_seq < self.window().earliest_retained_seq => {
                return Err(LedgerError::CheckpointOutsideRetention {
                    seq: checkpoint.through_seq,
                    earliest: self.window().earliest_retained_seq,
                });
            }
            None => {
                return Err(LedgerError::CheckpointDivergence {
                    seq: checkpoint.through_seq,
                    expected: checkpoint.head_digest.clone(),
                    found: String::new(),
                });
            }
        }

        let mut run = ProjectionRun {
            projection: checkpoint.projection.clone(),
            state: checkpoint.state.clone(),
            through_seq: Some(checkpoint.through_seq),
            head_digest: checkpoint.head_digest.clone(),
            applied: 0,
        };
        for entry in self
            .entries()
            .iter()
            .filter(|entry| entry.seq > checkpoint.through_seq)
        {
            projection.apply(&mut run.state, entry);
            run.through_seq = Some(entry.seq);
            run.head_digest = entry.digest.clone();
            run.applied += 1;
        }
        Ok(run)
    }

    /// Folds the entries visible at a temporal cut.
    ///
    /// The resulting run carries no usable resume point, because a cut is a filter and the next
    /// entry to arrive may fall inside it. `through_seq` is therefore `None` regardless of how
    /// much was folded, and [`ProjectionRun::checkpoint`] on it yields nothing.
    pub fn project_cut<P: Projection>(
        &self,
        projection: &P,
        cut: &TemporalCut,
    ) -> Result<ProjectionRun<P::State>, LedgerError> {
        let visible = self.cut(cut)?;
        let mut run = ProjectionRun {
            projection: projection.name().to_string(),
            state: projection.empty(),
            through_seq: None,
            head_digest: String::new(),
            applied: 0,
        };
        for entry in visible {
            projection.apply(&mut run.state, entry);
            run.applied += 1;
        }
        Ok(run)
    }

    /// How many entries a checkpoint has yet to absorb — 40.09's "projection lag" metric.
    pub fn projection_lag<S>(&self, checkpoint: &Checkpoint<S>) -> u64 {
        self.entries()
            .iter()
            .filter(|entry| entry.seq > checkpoint.through_seq)
            .count() as u64
    }
}

/// What the ledger currently says about each subject.
///
/// Ordered by valid time with the sequence number breaking ties, matching
/// [`EventLedger::latest_by_subject`]. The two are different code paths on purpose: the query
/// answers a cut by filtering, the projection answers "now" by folding, and a test that they
/// agree is a real check on both.
#[derive(Debug, Clone, Copy, Default)]
pub struct SubjectLatest;

/// One subject's current entry, reduced to what a view needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestFact {
    pub event: EventId,
    pub seq: u64,
    pub valid: ValidTime,
    /// The payload digest rather than the payload: a projection that copies bodies around
    /// duplicates the thing 12.22 may later be required to destroy.
    pub payload_digest: String,
}

impl Projection for SubjectLatest {
    type State = BTreeMap<SubjectKey, LatestFact>;

    fn name(&self) -> &str {
        "subject_latest"
    }

    fn empty(&self) -> Self::State {
        BTreeMap::new()
    }

    fn apply(&self, state: &mut Self::State, entry: &LedgerEntry) {
        let candidate = LatestFact {
            event: entry.id.clone(),
            seq: entry.seq,
            valid: entry.event.times.valid,
            payload_digest: entry.event.payload.digest().as_str().to_string(),
        };
        let subject = entry.event.subject.clone();
        let wins = state
            .get(&subject)
            .is_none_or(|current| (candidate.valid, candidate.seq) > (current.valid, current.seq));
        if wins {
            state.insert(subject, candidate);
        }
    }
}

/// Counts per transition family, the cheapest useful projection and a good rebuild-equality
/// subject because its fold is obviously associative.
#[derive(Debug, Clone, Copy, Default)]
pub struct ClassCounts;

impl Projection for ClassCounts {
    type State = BTreeMap<EventClass, u64>;

    fn name(&self) -> &str {
        "class_counts"
    }

    fn empty(&self) -> Self::State {
        BTreeMap::new()
    }

    fn apply(&self, state: &mut Self::State, entry: &LedgerEntry) {
        *state.entry(entry.event.class).or_insert(0) += 1;
    }
}
