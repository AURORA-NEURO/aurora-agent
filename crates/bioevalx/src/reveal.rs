//! Seal, commit, reveal — and the rubric that cannot move in between (26.16).
//!
//! 26.16's protocol is a state machine and its scoring step ends "without retrospective rubric
//! changes". Its second failure mode is "benchmark authors tune rubric after seeing results". Both
//! describe the same thing: prospective evaluation is worth what it is worth only because the
//! rules were fixed before the answer was known, and every way of losing that is a way of turning
//! a strong result into a retrospective one wearing its clothes.
//!
//! The state machine is the type. [`Registration`] is the open state and accepts commitments;
//! [`Registration::seal`] consumes it and returns a [`Sealed`], which accepts none; [`Sealed::
//! reveal`] consumes *that* and returns a [`Revealed`]. Each transition takes `self` by value, so
//! the earlier state is gone — there is no `unseal`, and a caller holding a `Sealed` cannot reach
//! a `Registration` to add a commitment to.
//!
//! # The rubric digest is the mechanism
//!
//! [`Registration::seal`] takes the rubric's content hash and stores it. [`Revealed::score_under`]
//! takes the rubric the scorer is actually holding, hashes it, and returns
//! [`RevealError::RubricChanged`] if it differs. A rubric edited between seal and score cannot be
//! used, and the error names both digests so the change is auditable rather than merely blocked.
//!
//! This is `bioprism-bioeval`'s credit rule in a different currency — there, credit is a function
//! application carrying the rule id that produced it; here, a score is admissible only under the
//! rule that was sealed.
//!
//! # An uncommitted outcome cannot be scored
//!
//! 26.16's third failure mode is "agent retries after reveal". [`Revealed::score_under`] refuses
//! an outcome whose target was never committed, so a system cannot answer a question after seeing
//! the answer and have that answer counted. Symmetrically, a committed prediction with no revealed
//! outcome is reported as [`Scoring::unrevealed`] rather than dropped — 26.16's "selective
//! publication" failure mode is exactly the case where the unscored commitments quietly vanish.
//!
//! # Not implemented
//!
//! No signing and no timestamping authority. 26.16 step 4 is "timestamp and sign submission";
//! this crate has no key material and no clock, and a self-asserted timestamp proves nothing.
//! [`Sealed::sealed_at`] carries a caller-supplied instant and the documentation says it is an
//! assertion, not an attestation. No leakage audit against public preprints (26.16's first failure
//! mode) — that needs a corpus and a search, and [`crate::worldline`] handles the leakage that is
//! checkable from timestamps alone. No calibration or replication-rate estimator.

use std::collections::BTreeMap;

use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::RevealError;

/// What a system committed to before the outcome was known.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Commitment {
    /// What this predicts about. Matched by equality against a revealed outcome's target.
    pub target: String,
    /// The prediction itself, as opaque structured data. Hashed into the seal, so its content
    /// cannot change after the fact either.
    pub prediction: Value,
    /// The analysis plan this commitment will be scored under. 26.16 lists "analysis-plan
    /// adherence" as a metric; carrying the plan with the commitment is what makes adherence
    /// checkable at all.
    pub analysis_plan: String,
}

impl Commitment {
    /// Commit a prediction under a stated analysis plan.
    pub fn new(
        target: impl Into<String>,
        prediction: Value,
        analysis_plan: impl Into<String>,
    ) -> Self {
        Commitment {
            target: target.into(),
            prediction,
            analysis_plan: analysis_plan.into(),
        }
    }
}

/// The open state: commitments may be added, nothing may be scored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Registration {
    pub study: String,
    commitments: BTreeMap<String, Commitment>,
}

impl Registration {
    /// Open a registration.
    pub fn open(study: impl Into<String>) -> Self {
        Registration {
            study: study.into(),
            commitments: BTreeMap::new(),
        }
    }

    /// Add a commitment.
    pub fn commit(&mut self, commitment: Commitment) -> Result<(), RevealError> {
        if self.commitments.contains_key(&commitment.target) {
            return Err(RevealError::DuplicateCommitment(commitment.target));
        }
        self.commitments
            .insert(commitment.target.clone(), commitment);
        Ok(())
    }

    /// Seal the registration under a rubric, consuming it.
    ///
    /// Takes `self` by value. That is the whole mechanism: after this call the open state does not
    /// exist, so there is no object on which `commit` could be called.
    ///
    /// `sealed_at` is a caller assertion about when this happened. This crate reads no clock and
    /// verifies no attestation; the field exists so the assertion is recorded and can be checked
    /// against an external record, not because holding it makes it true.
    pub fn seal(self, rubric: &Value, sealed_at: Timestamp) -> Result<Sealed, RevealError> {
        if self.commitments.is_empty() {
            return Err(RevealError::NothingCommitted);
        }
        let rubric_digest = ContentHash::of_value(rubric)
            .map_err(|e| RevealError::RubricChanged {
                sealed: "<unhashable rubric>".to_string(),
                presented: e.to_string(),
            })?
            .as_str()
            .to_string();
        let commitment_digest = ContentHash::of_value(
            &serde_json::to_value(&self.commitments).map_err(|e| RevealError::RubricChanged {
                sealed: "<unserializable commitments>".to_string(),
                presented: e.to_string(),
            })?,
        )
        .map_err(|e| RevealError::RubricChanged {
            sealed: "<unhashable commitments>".to_string(),
            presented: e.to_string(),
        })?
        .as_str()
        .to_string();
        Ok(Sealed {
            study: self.study,
            commitments: self.commitments,
            rubric_digest,
            commitment_digest,
            sealed_at,
        })
    }

    /// The commitments made so far.
    pub fn commitments(&self) -> &BTreeMap<String, Commitment> {
        &self.commitments
    }
}

/// The sealed state: commitments are frozen, the outcome is not yet known.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sealed {
    pub study: String,
    commitments: BTreeMap<String, Commitment>,
    rubric_digest: String,
    commitment_digest: String,
    sealed_at: Timestamp,
}

impl Sealed {
    /// Refuse a commitment. Present so the refusal is discoverable where a caller looks for it.
    pub fn commit(&self, _commitment: Commitment) -> Result<(), RevealError> {
        Err(RevealError::AlreadySealed)
    }

    /// Reveal outcomes, consuming the sealed state.
    pub fn reveal(self, outcomes: Vec<Outcome>) -> Revealed {
        Revealed {
            sealed: self,
            outcomes,
        }
    }

    /// The digest of the rubric that was sealed.
    pub fn rubric_digest(&self) -> &str {
        &self.rubric_digest
    }

    /// The digest of the commitment set, so a later reader can verify nothing was substituted.
    pub fn commitment_digest(&self) -> &str {
        &self.commitment_digest
    }

    /// The caller's assertion about when the seal was taken. Not an attestation.
    pub fn sealed_at(&self) -> Timestamp {
        self.sealed_at
    }

    /// The frozen commitments.
    pub fn commitments(&self) -> &BTreeMap<String, Commitment> {
        &self.commitments
    }
}

/// A revealed outcome for one target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub target: String,
    pub observed: Value,
}

impl Outcome {
    /// Reveal what happened for one target.
    pub fn new(target: impl Into<String>, observed: Value) -> Self {
        Outcome {
            target: target.into(),
            observed,
        }
    }
}

/// The revealed state: outcomes are known and scoring is possible under the sealed rubric.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Revealed {
    sealed: Sealed,
    outcomes: Vec<Outcome>,
}

impl Revealed {
    /// Score under the rubric the caller is holding, refusing if it is not the sealed one.
    ///
    /// The comparison is by content hash over canonical bytes, so a reformatted-but-identical
    /// rubric passes and a semantically edited one does not.
    pub fn score_under(&self, rubric: &Value) -> Result<Scoring, RevealError> {
        let presented = ContentHash::of_value(rubric)
            .map_err(|e| RevealError::RubricChanged {
                sealed: self.sealed.rubric_digest.clone(),
                presented: e.to_string(),
            })?
            .as_str()
            .to_string();
        if presented != self.sealed.rubric_digest {
            return Err(RevealError::RubricChanged {
                sealed: self.sealed.rubric_digest.clone(),
                presented,
            });
        }
        let mut scored = Vec::new();
        for outcome in &self.outcomes {
            let commitment = self
                .sealed
                .commitments
                .get(&outcome.target)
                .ok_or_else(|| RevealError::UncommittedOutcome(outcome.target.clone()))?;
            scored.push(ScoredCommitment {
                target: outcome.target.clone(),
                analysis_plan: commitment.analysis_plan.clone(),
                predicted: commitment.prediction.clone(),
                observed: outcome.observed.clone(),
            });
        }
        let unrevealed = self
            .sealed
            .commitments
            .keys()
            .filter(|target| !self.outcomes.iter().any(|o| o.target == **target))
            .cloned()
            .collect();
        Ok(Scoring {
            study: self.sealed.study.clone(),
            rubric_digest: self.sealed.rubric_digest.clone(),
            commitment_digest: self.sealed.commitment_digest.clone(),
            scored,
            unrevealed,
        })
    }

    /// Refuse a second reveal.
    pub fn reveal(&self, _outcomes: Vec<Outcome>) -> Result<(), RevealError> {
        Err(RevealError::AlreadyRevealed)
    }

    /// The sealed state this was revealed from.
    pub fn sealed(&self) -> &Sealed {
        &self.sealed
    }
}

/// One commitment paired with what actually happened.
///
/// Deliberately not scored. Comparing a prediction to an outcome needs a scoring rule over the
/// prediction's own state space, which `bioprism-bioeval` owns and which this module has no
/// business re-deriving. What this module guarantees is that the pair is legitimate: the
/// prediction was sealed first, and the rubric has not moved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScoredCommitment {
    pub target: String,
    pub analysis_plan: String,
    pub predicted: Value,
    pub observed: Value,
}

/// The admissible pairs, plus the commitments nobody revealed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scoring {
    pub study: String,
    pub rubric_digest: String,
    pub commitment_digest: String,
    pub scored: Vec<ScoredCommitment>,
    /// Targets that were committed and never revealed. 26.16's "selective publication" is visible
    /// here and nowhere else, so this list is part of the result rather than an optional extra.
    pub unrevealed: Vec<String>,
}

impl Scoring {
    /// Whether every commitment got an outcome.
    pub fn complete(&self) -> bool {
        self.unrevealed.is_empty()
    }
}
