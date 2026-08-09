//! The seam between a run and the world, and the two things that can sit behind it.
//!
//! Blueprint 05.07 (network, time and randomness virtualization). Everything nondeterministic a run
//! can touch goes through one narrow door — `Host::perform` — and there are exactly two doors on
//! offer: one that asks the world and writes down the answer, and one that reads the answer off a
//! tape and never asks anything.
//!
//! The property this file exists to guarantee is that **a replay cannot fall through to the live
//! world**, and it is guaranteed structurally rather than by a check. `ReplayHost` has no source
//! field. There is no live world in scope for it to consult, no flag that could be set wrong, no
//! `if recorded.is_none() { live() }` branch to review. A replay that meets an effect its tape does
//! not record has nothing to do but fail, and it fails saying which step and which request.
//!
//! The second guarantee is that a replay is *checked*, not merely fed. A recorded outcome is only
//! returned when the request at that step matches the request the program made. A program that has
//! changed does not get plausible answers to questions nobody recorded; it gets
//! `DivergentRequest` naming both sides. This is the difference between a replay harness and a
//! fixture cache, and it is why byte-identical reproduction means anything.
//!
//! Deliberately **not** implemented: a wall-clock, OS-entropy or real-socket `EffectSource`. Those
//! belong to the subprocess and container providers, which 05.03 declares and this crate refuses to
//! fake (see `provider`). What ships here is the seam and a deterministic in-process world, so
//! every test in this crate is reproducible and no test depends on the machine it runs on.

use crate::budget::{BudgetController, RuntimeResource};
use crate::effect::{
    Authorization, DecisionOutcome, Effect, EffectOutcome, EffectPolicy, EffectRequest,
    PolicyDecision,
};
use crate::error::RuntimeError;
use crate::tape::WorldTape;
use bioprism_ids::RunId;

/// A live world that can answer an effect request.
///
/// Implementors are the only code in a run that is allowed to be nondeterministic. Everything above
/// this trait is a pure function of the request stream and the answers.
pub trait EffectSource {
    fn perform(&mut self, request: &EffectRequest) -> Result<EffectOutcome, RuntimeError>;
}

/// What a running program sees of the outside.
///
/// Object-safe on purpose: a continuation handed to `fork` is written against `&mut dyn Host`, so
/// the same program text can be run recording, replaying, or as the suffix of a fork without being
/// generic over which.
pub trait Host {
    fn perform(&mut self, request: EffectRequest) -> Result<EffectOutcome, RuntimeError>;

    /// The tape this host is writing or reading.
    fn tape(&self) -> &WorldTape;
}

/// Asks the world and writes down what it said.
#[derive(Debug)]
pub struct RecordingHost<S: EffectSource> {
    tape: WorldTape,
    source: S,
    policy: EffectPolicy,
    journal: Vec<PolicyDecision>,
    budget: Option<BudgetController>,
}

impl<S: EffectSource> RecordingHost<S> {
    pub fn new(run: RunId, source: S, policy: EffectPolicy) -> Self {
        RecordingHost {
            tape: WorldTape::new(run),
            source,
            policy,
            journal: Vec::new(),
            budget: None,
        }
    }

    /// Continues recording onto an existing tape.
    ///
    /// Used by 05.05's suffix execution, where the tape already carries an inherited prefix and new
    /// entries must chain onto it rather than starting a fresh chain.
    pub fn resuming(tape: WorldTape, source: S, policy: EffectPolicy) -> Self {
        RecordingHost {
            tape,
            source,
            policy,
            journal: Vec::new(),
            budget: None,
        }
    }

    /// Attaches a meter.
    ///
    /// Metering is opt-in at the host because the trial-level budget belongs to the orchestrator
    /// (05.02) and a host that invented its own would double-count. When attached, each authorized
    /// effect costs one `ToolCalls` unit, charged *before* the effect happens, so an exhausted
    /// budget stops the effect instead of paying for one it already performed.
    pub fn with_budget(mut self, budget: BudgetController) -> Self {
        self.budget = Some(budget);
        self
    }

    /// Every authorization verdict, including the refusals (05.08).
    pub fn journal(&self) -> &[PolicyDecision] {
        &self.journal
    }

    pub fn policy(&self) -> &EffectPolicy {
        &self.policy
    }

    pub fn budget(&self) -> Option<&BudgetController> {
        self.budget.as_ref()
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    pub fn into_tape(self) -> WorldTape {
        self.tape
    }

    pub fn into_parts(self) -> (WorldTape, S, Vec<PolicyDecision>, Option<BudgetController>) {
        (self.tape, self.source, self.journal, self.budget)
    }

    fn record_decision(&mut self, request: &EffectRequest, outcome: DecisionOutcome) {
        self.journal.push(PolicyDecision {
            seq: self.journal.len() as u64,
            request: request.clone(),
            class: request.class(),
            outcome,
        });
    }
}

impl<S: EffectSource> Host for RecordingHost<S> {
    fn perform(&mut self, request: EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        let authorization = match self.policy.authorize(&request) {
            Ok(authorization) => authorization,
            Err(denial) => {
                self.record_decision(
                    &request,
                    DecisionOutcome::Denied {
                        reason: denial.to_string(),
                    },
                );
                return Err(denial);
            }
        };

        if let Some(budget) = self.budget.as_mut() {
            budget.charge(RuntimeResource::ToolCalls, 1)?;
        }

        let effect = match authorization {
            Authorization::Perform => {
                let outcome = self.source.perform(&request)?;
                self.record_decision(&request, DecisionOutcome::Permitted);
                Effect::performed(request, outcome)
            }
            Authorization::Simulate => {
                self.record_decision(&request, DecisionOutcome::Simulated);
                Effect::simulated(request)
            }
        };

        let outcome = effect.outcome.clone();
        self.tape.append(effect)?;
        Ok(outcome)
    }

    fn tape(&self) -> &WorldTape {
        &self.tape
    }
}

/// Reads answers off a tape, and has nothing else to read from.
///
/// Note what this struct does not contain: a source, a policy, a fallback. A `ReplayHost` is the
/// tape plus a cursor. That is the whole implementation of "a replay never goes live".
#[derive(Debug)]
pub struct ReplayHost {
    tape: WorldTape,
    cursor: u64,
}

impl ReplayHost {
    pub fn new(tape: WorldTape) -> Self {
        ReplayHost { tape, cursor: 0 }
    }

    pub fn cursor(&self) -> u64 {
        self.cursor
    }

    pub fn remaining(&self) -> u64 {
        self.tape.len().saturating_sub(self.cursor)
    }

    /// Ends the replay, insisting that the whole tape was consumed.
    ///
    /// A program that stops early is as much a divergence as one that asks for too much — it means
    /// the replayed program is not the recorded one — but it produces no error on its own, because
    /// nothing went wrong at any single step. Checking it here is the only place the discrepancy is
    /// visible.
    pub fn finish(self) -> Result<WorldTape, RuntimeError> {
        if self.cursor == self.tape.len() {
            Ok(self.tape)
        } else {
            Err(RuntimeError::ReplayIncomplete {
                consumed: self.cursor,
                total: self.tape.len(),
            })
        }
    }
}

impl Host for ReplayHost {
    fn perform(&mut self, request: EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        let Some(entry) = self.tape.entries().get(self.cursor as usize) else {
            return Err(RuntimeError::UnrecordedEffect {
                step: self.cursor,
                request: request.to_string(),
            });
        };
        if entry.effect.request != request {
            return Err(RuntimeError::DivergentRequest {
                step: self.cursor,
                recorded: entry.effect.request.to_string(),
                requested: request.to_string(),
            });
        }
        self.cursor += 1;
        Ok(entry.effect.outcome.clone())
    }

    fn tape(&self) -> &WorldTape {
        &self.tape
    }
}
