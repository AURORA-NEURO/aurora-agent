//! The WorldTape: an append-only, hash-chained record of everything a run did to the world.
//!
//! Blueprint 05.04 (WorldTape and state ledger). The module's whole claim is this: a tape holds
//! *enough* to replay the run that produced it, and *only* what a replay needs. Both halves matter.
//! Too little and a replay quietly reaches for the live world; too much and the tape becomes a log,
//! which nobody can replay and everybody trusts anyway.
//!
//! Each entry chains the previous entry's digest, so the digest after step *n* is a commitment to
//! the entire history up to *n*. That single property does most of the work here:
//!
//! - **Tamper evidence.** Editing step 3 changes every digest from 3 onward, and `verify_chain`
//!   finds it. A tape is only ever accepted through a verifying constructor, so a tampered tape
//!   fails to *load* rather than loading and misbehaving later.
//! - **State identity.** "The state after step *n*" needs no snapshot format — it is the digest at
//!   step *n*. Two runs are at the same state exactly when those digests agree, which is what
//!   05.05's matched counterfactuals are matched *on*.
//! - **Cheap divergence search.** Finding the first step at which two runs differ is a scan of two
//!   digest lists, not a diff of two worlds.
//!
//! Relationship to `bioprism-weave`'s ledger: the shapes rhyme, and that is not duplication. Weave
//! chains *communicative acts* between participants — who proposed, who accepted, who is owed what.
//! This chains *effects on the world* — what was read, written, fetched and spent. A run has both,
//! they are appended by different subsystems for different audiences, and a single merged log would
//! be replayable by neither.
//!
//! Deliberately **not** implemented: garbage collection and compaction (05.04's "retention"). The
//! blueprint wants unreferenced intermediate state compacted after producing a verifiable summary.
//! Compaction that preserves a hash chain needs a pruning-commitment scheme, and inventing one here
//! before there is a storage backend to prune would be guesswork. Tapes are kept whole.

use crate::effect::{Effect, EffectRequest, Provenance};
use crate::error::RuntimeError;
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// One effect, sealed into the chain.
///
/// `digest` covers `step`, `effect` and `previous` — and nothing else. In particular it does not
/// cover the tape's lineage, so a forked tape's inherited prefix keeps digests byte-identical to
/// its parent's. That is what makes "fork at exactly this state" checkable rather than asserted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TapeEntry {
    pub step: u64,
    pub effect: Effect,
    /// Digest of the previous entry, or the empty string for the first.
    pub previous: String,
    pub digest: String,
}

/// Where a tape came from, when it did not start from nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TapeLineage {
    pub parent_run: RunId,
    /// The number of steps inherited from the parent. Steps `0..forked_at_step` were performed by
    /// the parent's world-line, not by this run.
    pub forked_at_step: u64,
    /// The parent's state digest at the fork point, restated so ancestry is checkable without the
    /// parent tape in hand.
    pub parent_head: String,
}

/// What a provider would need in order to restore a checkpoint.
///
/// 05.03 requires state handles to carry a restoration declaration rather than an opaque blob,
/// because "we have a snapshot" and "we can still restore that snapshot on some other machine next
/// quarter" are different claims and only the second is useful for published evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RestorationDeclaration {
    /// True when the checkpoint can be restored from the tape alone.
    pub portable: bool,
    /// The provider whose native snapshot is needed, when it is not portable.
    pub requires_provider: Option<String>,
    pub notes: String,
}

impl RestorationDeclaration {
    /// The declaration an in-process run makes: the tape is the state.
    pub fn portable() -> Self {
        RestorationDeclaration {
            portable: true,
            requires_provider: None,
            notes: "the tape prefix is sufficient to restore this state".into(),
        }
    }

    pub fn provider_bound(provider: impl Into<String>, notes: impl Into<String>) -> Self {
        RestorationDeclaration {
            portable: false,
            requires_provider: Some(provider.into()),
            notes: notes.into(),
        }
    }
}

/// A named point in a tape's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub id: String,
    pub step: u64,
    /// The state digest at `step`. A checkpoint that cannot name the history it summarizes is not
    /// evidence of anything.
    pub tape_head: String,
    pub provider: String,
    pub restoration: RestorationDeclaration,
}

/// What a run read and what it produced.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Artifacts {
    /// Paths the run read.
    pub consumed: BTreeSet<String>,
    /// Paths the run wrote, mapped to the digest of the last content written.
    pub created: BTreeMap<String, String>,
}

/// The serialized shape of a tape.
///
/// A tape is deserialized *through* this type so the chain is verified before a `WorldTape` exists.
/// Deriving `Deserialize` directly on `WorldTape` would let a hand-edited file become a live tape
/// with a broken chain, and every later check would be verifying an already-lost argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TapeRepr {
    run: RunId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    lineage: Option<TapeLineage>,
    entries: Vec<TapeEntry>,
    #[serde(default)]
    checkpoints: Vec<Checkpoint>,
}

/// An append-only record of one run's effects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "TapeRepr", into = "TapeRepr")]
pub struct WorldTape {
    run: RunId,
    lineage: Option<TapeLineage>,
    entries: Vec<TapeEntry>,
    checkpoints: Vec<Checkpoint>,
}

impl WorldTape {
    pub fn new(run: RunId) -> Self {
        WorldTape {
            run,
            lineage: None,
            entries: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    /// Builds a tape that inherits a parent's prefix verbatim.
    ///
    /// Used by 05.05's fork. The entries are *copies*, not re-performances: the child did not read
    /// those files or send those requests, its world-line simply begins after they happened.
    pub(crate) fn forked(
        run: RunId,
        lineage: TapeLineage,
        entries: Vec<TapeEntry>,
    ) -> Self {
        WorldTape {
            run,
            lineage: Some(lineage),
            entries,
            checkpoints: Vec::new(),
        }
    }

    pub fn run(&self) -> &RunId {
        &self.run
    }

    pub fn lineage(&self) -> Option<&TapeLineage> {
        self.lineage.as_ref()
    }

    /// The number of leading steps this run inherited rather than performed.
    pub fn inherited_steps(&self) -> u64 {
        self.lineage.as_ref().map_or(0, |l| l.forked_at_step)
    }

    pub fn entries(&self) -> &[TapeEntry] {
        &self.entries
    }

    pub fn len(&self) -> u64 {
        self.entries.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn checkpoints(&self) -> &[Checkpoint] {
        &self.checkpoints
    }

    /// The digest committing to the whole history so far.
    pub fn head(&self) -> &str {
        self.entries.last().map_or("", |entry| entry.digest.as_str())
    }

    /// Seals an effect into the chain.
    pub fn append(&mut self, effect: Effect) -> Result<&TapeEntry, RuntimeError> {
        let step = self.len();
        let previous = self.head().to_string();
        let digest = Self::digest_of(step, &effect, &previous)?;
        self.entries.push(TapeEntry {
            step,
            effect,
            previous,
            digest,
        });
        Ok(self.entries.last().expect("just pushed"))
    }

    fn digest_of(step: u64, effect: &Effect, previous: &str) -> Result<String, RuntimeError> {
        let body = json!({
            "step": step,
            "effect": effect,
            "previous": previous,
        });
        ContentHash::of_value(&body)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| RuntimeError::Uncanonical(error.to_string()))
    }

    /// The state digest after exactly `step` effects.
    ///
    /// `state_digest_at(0)` is the empty string: a run that has done nothing has committed to
    /// nothing, and every such run is at the same state.
    pub fn state_digest_at(&self, step: u64) -> Result<&str, RuntimeError> {
        if step > self.len() {
            return Err(RuntimeError::StepOutOfRange {
                step,
                length: self.len(),
            });
        }
        if step == 0 {
            return Ok("");
        }
        Ok(self.entries[(step - 1) as usize].digest.as_str())
    }

    /// The first `step` entries, for handing to a fork.
    pub fn prefix(&self, step: u64) -> Result<Vec<TapeEntry>, RuntimeError> {
        if step > self.len() {
            return Err(RuntimeError::StepOutOfRange {
                step,
                length: self.len(),
            });
        }
        Ok(self.entries[..step as usize].to_vec())
    }

    /// Recomputes the chain from scratch, the way an auditor must before believing any of it.
    pub fn verify_chain(&self) -> Result<(), RuntimeError> {
        let mut expected_previous = String::new();
        for (index, entry) in self.entries.iter().enumerate() {
            let step = index as u64;
            if entry.step != step {
                return Err(RuntimeError::BrokenChain {
                    step,
                    reason: format!("entry claims step {} but sits at {step}", entry.step),
                });
            }
            if entry.previous != expected_previous {
                return Err(RuntimeError::BrokenChain {
                    step,
                    reason: "previous digest does not match the prior entry".into(),
                });
            }
            let recomputed = Self::digest_of(entry.step, &entry.effect, &entry.previous)?;
            if recomputed != entry.digest {
                return Err(RuntimeError::BrokenChain {
                    step,
                    reason: "entry digest does not match its contents".into(),
                });
            }
            expected_previous = entry.digest.clone();
        }
        Ok(())
    }

    /// Records a checkpoint at the current head.
    pub fn checkpoint(
        &mut self,
        provider: impl Into<String>,
        restoration: RestorationDeclaration,
    ) -> Checkpoint {
        let checkpoint = Checkpoint {
            id: format!("ckpt-{}-{:06}", self.run.as_str(), self.checkpoints.len()),
            step: self.len(),
            tape_head: self.head().to_string(),
            provider: provider.into(),
            restoration,
        };
        self.checkpoints.push(checkpoint.clone());
        checkpoint
    }

    /// Confirms a checkpoint still describes this tape.
    ///
    /// The interesting failure is not corruption on disk but a checkpoint taken against a different
    /// world-line and offered as if it belonged here. 05.03 lists "corrupted checkpoint" as its own
    /// error class for exactly that reason.
    pub fn verify_checkpoint(&self, checkpoint: &Checkpoint) -> Result<(), RuntimeError> {
        let found = self.state_digest_at(checkpoint.step)?;
        if found == checkpoint.tape_head {
            Ok(())
        } else {
            Err(RuntimeError::CorruptCheckpoint {
                id: checkpoint.id.clone(),
                expected: checkpoint.tape_head.clone(),
                found: found.to_string(),
            })
        }
    }

    /// The earliest step at which two tapes stop agreeing, if they ever do.
    ///
    /// Compares digests, so a difference anywhere in an effect — request, outcome or provenance —
    /// surfaces at the step it happened, and everything after it is downstream of that one cause.
    pub fn first_divergence(&self, other: &WorldTape) -> Option<u64> {
        let shared = self.entries.len().min(other.entries.len());
        for index in 0..shared {
            if self.entries[index].digest != other.entries[index].digest {
                return Some(index as u64);
            }
        }
        if self.entries.len() == other.entries.len() {
            None
        } else {
            Some(shared as u64)
        }
    }

    /// What the run read and what it left behind.
    pub fn artifacts(&self) -> Artifacts {
        let mut artifacts = Artifacts::default();
        for entry in &self.entries {
            match &entry.effect.request {
                EffectRequest::FileRead { path } => {
                    artifacts.consumed.insert(path.clone());
                }
                EffectRequest::FileWrite { path, content } => {
                    let digest = ContentHash::of_bytes(content.as_bytes());
                    artifacts
                        .created
                        .insert(path.clone(), digest.as_str().to_string());
                }
                _ => {}
            }
        }
        artifacts
    }

    /// The steps whose outcome the runtime invented rather than observed.
    ///
    /// A caller aggregating a tape into a score needs this to avoid crediting simulated work.
    pub fn simulated_steps(&self) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|entry| entry.effect.provenance == Provenance::Simulated)
            .map(|entry| entry.step)
            .collect()
    }

    pub fn to_json(&self) -> Result<String, RuntimeError> {
        serde_json::to_string(self).map_err(|error| RuntimeError::Uncanonical(error.to_string()))
    }

    /// Loads a tape, verifying the chain before returning one.
    pub fn from_json(raw: &str) -> Result<Self, RuntimeError> {
        serde_json::from_str(raw).map_err(|error| RuntimeError::BrokenChain {
            step: 0,
            reason: error.to_string(),
        })
    }
}

impl TryFrom<TapeRepr> for WorldTape {
    type Error = RuntimeError;

    fn try_from(repr: TapeRepr) -> Result<Self, Self::Error> {
        let tape = WorldTape {
            run: repr.run,
            lineage: repr.lineage,
            entries: repr.entries,
            checkpoints: repr.checkpoints,
        };
        tape.verify_chain()?;
        for checkpoint in &tape.checkpoints {
            tape.verify_checkpoint(checkpoint)?;
        }
        Ok(tape)
    }
}

impl From<WorldTape> for TapeRepr {
    fn from(tape: WorldTape) -> Self {
        TapeRepr {
            run: tape.run,
            lineage: tape.lineage,
            entries: tape.entries,
            checkpoints: tape.checkpoints,
        }
    }
}
