//! Prospective escrow and the blind-reveal vault.
//!
//! Blueprint 35.05: "create the strongest evaluation tier by freezing systems before later
//! outcomes, experiments, or replication data are revealed." It is the strongest tier because it is
//! the only one where the answer provably did not exist inside the evaluated system, and the whole
//! construction lives or dies on the word *provably*.
//!
//! ## The invariant
//!
//! A reveal must prove the committed value was fixed **before** the run. Two pieces make that
//! checkable by someone who was not in the room:
//!
//! 1. **A hash commitment.** [`EscrowVault::seal`] publishes `sha256(canonical{salt, payload})` and
//!    keeps the payload private. The salt is mandatory and not optional, because escrowed outcomes
//!    are frequently low-entropy — "responder" / "non-responder" is one bit — and an unsalted
//!    commitment to one bit is a commitment to nothing at all: anyone can enumerate both hashes.
//! 2. **An ordering witness.** The vault issues a monotone [`Sequence`] on every state change and
//!    records the sequence at which the escrow was sealed and at which each run was registered.
//!    Revealing against a run that started *before* the seal is [`EscrowError::CommitmentNotPriorToRun`].
//!
//! Wall-clock timestamps are deliberately absent. A timestamp is supplied by the party being
//! audited and proves nothing; a vault-issued sequence is at least internally consistent, and the
//! whole sequence is in the audit trail so an external notary can anchor it.
//!
//! ## The payload has no serializable form until it is revealed
//!
//! The sealed value is a private field with no accessor and no `Serialize`. The only serializable
//! carriers are [`EscrowRecord`], which publishes the commitment and never the payload, and
//! [`Reveal`], which the vault only mints once every condition holds. Writing a vault to JSON
//! cannot leak the answer, because there is no code path that puts it there.
//!
//! ## Not implemented
//!
//! No cryptographic timestamping, no external notary, no threshold custody, no signature over the
//! commitment. In-process and single-party: this proves ordering *within* one vault's own history,
//! which is what makes an audit trail checkable, not what makes it trustless. 35.05's
//! pre-registration and adjudication documents are governance artefacts and live elsewhere.

use crate::error::EscrowError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// A vault-issued logical instant. Monotone, gapless, and the only ordering this crate trusts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Sequence(pub u64);

impl Sequence {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// When an escrow becomes revealable. Stated at seal time and immutable afterwards.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevealCondition {
    /// Revealable once the vault clock reaches this sequence. The blunt instrument.
    AtOrAfter(Sequence),
    /// Revealable once the named run has been marked complete — the usual prospective design:
    /// freeze the system, run it blind, then reveal.
    WhenRunCompleted { run: String },
    /// Revealable once the named external outcome has arrived: follow-up data, a replication, a
    /// clinical endpoint. The case 35.05 calls "later outcomes".
    WhenOutcomeRecorded { outcome: String },
}

impl RevealCondition {
    pub fn describe(&self) -> String {
        match self {
            RevealCondition::AtOrAfter(sequence) => {
                format!("revealable at or after sequence {}", sequence.get())
            }
            RevealCondition::WhenRunCompleted { run } => {
                format!("revealable once run {run:?} completes")
            }
            RevealCondition::WhenOutcomeRecorded { outcome } => {
                format!("revealable once outcome {outcome:?} is recorded")
            }
        }
    }
}

/// The lifecycle of one escrow, for the public audit trail.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscrowState {
    Sealed,
    Revealed,
    /// 35.05's "missing follow-up handling". The outcome never arrived; the payload is permanently
    /// unrevealable and the commitment stays published. Nothing is deleted.
    Voided,
}

/// The public half of an escrow. Everything here may be published at seal time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscrowRecord {
    pub escrow_id: String,
    /// `sha256(canonical{salt, payload})`.
    pub commitment: String,
    pub sealed_at: Sequence,
    pub condition: RevealCondition,
    /// Digest of the frozen system under evaluation. 35.05's "system and architecture freeze".
    pub frozen_system: String,
    pub state: EscrowState,
    pub void_reason: Option<String>,
}

/// A sealed payload. No `Serialize`, no accessor, no `Clone` into anything public.
#[derive(Debug)]
struct SealedValue {
    payload: Value,
    salt: String,
}

/// A revealed escrow, in a form a third party can check without access to the vault.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reveal {
    pub escrow_id: String,
    pub payload: Value,
    pub salt: String,
    pub commitment: String,
    pub sealed_at: Sequence,
    pub run: String,
    pub run_registered_at: Sequence,
    pub revealed_at: Sequence,
    pub frozen_system: String,
}

impl Reveal {
    /// Recomputes the commitment and rechecks the ordering.
    ///
    /// This is the whole audit, and it needs nothing but the [`Reveal`] itself: recompute the hash
    /// from the disclosed salt and payload, compare it to the published commitment, and confirm the
    /// seal preceded the run. A vault that lied about either fails here.
    pub fn verify(&self) -> Result<(), EscrowError> {
        let recomputed = commit_digest(&self.payload, &self.salt)?;
        if recomputed != self.commitment {
            return Err(EscrowError::CommitmentMismatch {
                recomputed,
                recorded: self.commitment.clone(),
            });
        }
        if self.sealed_at >= self.run_registered_at {
            return Err(EscrowError::CommitmentNotPriorToRun {
                escrow: self.escrow_id.clone(),
                sealed_at: self.sealed_at.get(),
                run: self.run.clone(),
                run_at: self.run_registered_at.get(),
            });
        }
        Ok(())
    }
}

/// Computes the published commitment. Salt first so a payload cannot be extended into a different
/// salt's preimage.
pub fn commit_digest(payload: &Value, salt: &str) -> Result<String, EscrowError> {
    let document = serde_json::json!({ "salt": salt, "payload": payload });
    ContentHash::of_value(&document)
        .map(|digest| digest.as_str().to_string())
        .map_err(|error| EscrowError::CommitmentMismatch {
            recomputed: error.to_string(),
            recorded: String::new(),
        })
}

#[derive(Debug)]
struct Run {
    registered_at: Sequence,
    complete: bool,
}

/// The vault. Owns the logical clock, so it is the only thing that can order seals against runs.
#[derive(Debug, Default)]
pub struct EscrowVault {
    clock: u64,
    records: BTreeMap<String, EscrowRecord>,
    sealed: BTreeMap<String, SealedValue>,
    runs: BTreeMap<String, Run>,
    outcomes: BTreeSet<String>,
}

impl EscrowVault {
    pub fn new() -> Self {
        EscrowVault::default()
    }

    fn tick(&mut self) -> Sequence {
        self.clock += 1;
        Sequence(self.clock)
    }

    /// The vault's current logical instant.
    pub fn now(&self) -> Sequence {
        Sequence(self.clock)
    }

    /// Advances the clock without any other effect, for [`RevealCondition::AtOrAfter`].
    pub fn advance(&mut self) -> Sequence {
        self.tick()
    }

    /// Commits to `payload` and returns the public record.
    ///
    /// The payload is retained privately; the returned [`EscrowRecord`] is the artefact to
    /// publish. Re-sealing an existing id is refused, because a commitment that can be replaced is
    /// not a commitment.
    pub fn seal(
        &mut self,
        escrow_id: impl Into<String>,
        payload: Value,
        salt: impl Into<String>,
        frozen_system: impl Into<String>,
        condition: RevealCondition,
    ) -> Result<EscrowRecord, EscrowError> {
        let escrow_id = escrow_id.into();
        if self.records.contains_key(&escrow_id) {
            return Err(EscrowError::AlreadySealed(escrow_id));
        }
        let salt = salt.into();
        let commitment = commit_digest(&payload, &salt)?;
        let sealed_at = self.tick();
        let record = EscrowRecord {
            escrow_id: escrow_id.clone(),
            commitment,
            sealed_at,
            condition,
            frozen_system: frozen_system.into(),
            state: EscrowState::Sealed,
            void_reason: None,
        };
        self.sealed.insert(escrow_id.clone(), SealedValue { payload, salt });
        self.records.insert(escrow_id, record.clone());
        Ok(record)
    }

    /// Registers that a run has begun, taking a sequence number as its ordering witness.
    ///
    /// Anything sealed after this point cannot be revealed against this run.
    pub fn register_run(&mut self, run: impl Into<String>) -> Sequence {
        let registered_at = self.tick();
        self.runs.insert(
            run.into(),
            Run {
                registered_at,
                complete: false,
            },
        );
        registered_at
    }

    pub fn complete_run(&mut self, run: &str) -> Result<Sequence, EscrowError> {
        let completed_at = self.tick();
        let entry = self
            .runs
            .get_mut(run)
            .ok_or_else(|| EscrowError::UnknownRun(run.to_string()))?;
        entry.complete = true;
        Ok(completed_at)
    }

    /// Records the arrival of an external outcome, for [`RevealCondition::WhenOutcomeRecorded`].
    pub fn record_outcome(&mut self, outcome: impl Into<String>) -> Sequence {
        self.outcomes.insert(outcome.into());
        self.tick()
    }

    /// Voids an escrow whose follow-up never arrived.
    ///
    /// The payload becomes permanently unrevealable and the commitment stays in the audit trail.
    /// 35's failure containment is explicit that invalidated artefacts are marked, not deleted.
    pub fn void(&mut self, escrow_id: &str, reason: impl Into<String>) -> Result<(), EscrowError> {
        let record = self
            .records
            .get_mut(escrow_id)
            .ok_or_else(|| EscrowError::UnknownEscrow(escrow_id.to_string()))?;
        if record.state == EscrowState::Revealed {
            return Err(EscrowError::AlreadyRevealed(escrow_id.to_string()));
        }
        record.state = EscrowState::Voided;
        record.void_reason = Some(reason.into());
        self.sealed.remove(escrow_id);
        Ok(())
    }

    /// Reveals `escrow_id` against `run`, if every guard holds.
    ///
    /// The guards run in the order an auditor would ask about them: does the escrow exist, is it
    /// still sealed, does the run exist, did the commitment precede the run, is the presented
    /// system the frozen one, and only then has the stated condition been met.
    pub fn reveal(
        &mut self,
        escrow_id: &str,
        run: &str,
        presented_system: &str,
    ) -> Result<Reveal, EscrowError> {
        let record = self
            .records
            .get(escrow_id)
            .ok_or_else(|| EscrowError::UnknownEscrow(escrow_id.to_string()))?
            .clone();
        match record.state {
            EscrowState::Revealed => return Err(EscrowError::AlreadyRevealed(escrow_id.to_string())),
            EscrowState::Voided => {
                return Err(EscrowError::Voided {
                    escrow: escrow_id.to_string(),
                    reason: record.void_reason.unwrap_or_default(),
                })
            }
            EscrowState::Sealed => {}
        }

        let run_entry = self
            .runs
            .get(run)
            .ok_or_else(|| EscrowError::UnknownRun(run.to_string()))?;
        if record.sealed_at >= run_entry.registered_at {
            return Err(EscrowError::CommitmentNotPriorToRun {
                escrow: escrow_id.to_string(),
                sealed_at: record.sealed_at.get(),
                run: run.to_string(),
                run_at: run_entry.registered_at.get(),
            });
        }
        if record.frozen_system != presented_system {
            return Err(EscrowError::SystemNotFrozen {
                escrow: escrow_id.to_string(),
                frozen: record.frozen_system.clone(),
                presented: presented_system.to_string(),
            });
        }
        if !self.condition_met(&record.condition, run_entry) {
            return Err(EscrowError::ConditionNotMet {
                escrow: escrow_id.to_string(),
                condition: record.condition.describe(),
                now: self.clock,
            });
        }

        let run_registered_at = run_entry.registered_at;
        let sealed = self
            .sealed
            .remove(escrow_id)
            .ok_or_else(|| EscrowError::UnknownEscrow(escrow_id.to_string()))?;
        let revealed_at = self.tick();
        if let Some(record) = self.records.get_mut(escrow_id) {
            record.state = EscrowState::Revealed;
        }

        Ok(Reveal {
            escrow_id: escrow_id.to_string(),
            payload: sealed.payload,
            salt: sealed.salt,
            commitment: record.commitment,
            sealed_at: record.sealed_at,
            run: run.to_string(),
            run_registered_at,
            revealed_at,
            frozen_system: record.frozen_system,
        })
    }

    fn condition_met(&self, condition: &RevealCondition, run: &Run) -> bool {
        match condition {
            RevealCondition::AtOrAfter(sequence) => self.clock >= sequence.get(),
            RevealCondition::WhenRunCompleted { run: named } => {
                self.runs.get(named).map(|entry| entry.complete).unwrap_or(false) && run.complete
            }
            RevealCondition::WhenOutcomeRecorded { outcome } => self.outcomes.contains(outcome),
        }
    }

    pub fn record(&self, escrow_id: &str) -> Option<&EscrowRecord> {
        self.records.get(escrow_id)
    }

    /// The public audit trail: every escrow's commitment, seal sequence, condition and state, and
    /// no payloads. Safe to publish at any point in the lifecycle, including before any reveal.
    pub fn audit_trail(&self) -> Vec<EscrowRecord> {
        self.records.values().cloned().collect()
    }
}
