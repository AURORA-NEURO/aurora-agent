//! Forking a tape and running a different continuation from that exact state.
//!
//! Blueprint 05.05 (fork, replay and suffix execution). This is the machinery that makes matched
//! counterfactuals possible: take a run, choose the step where a decision was made, and ask what
//! would have happened if a different architecture had been in the chair from that step onward.
//! Without it, comparing two agents means comparing two runs that diverged at step zero, and every
//! difference downstream is confounded by every difference upstream.
//!
//! Three properties hold the design together.
//!
//! **The fork point is identified, not approximated.** A child tape inherits the parent's prefix
//! entries verbatim, digests included, so `child.state_digest_at(n) == parent.state_digest_at(n)`
//! is a checkable equality rather than a claim about two similar-looking worlds.
//!
//! **The prefix is inherited, never re-performed.** The child's world is asked nothing for steps it
//! did not run. A fork therefore cannot repeat a real-world side effect — not because a rule
//! forbids it, but because there is no code path that would ask. This is 05.05's irreversibility
//! requirement, discharged structurally.
//!
//! **Reconstruction differences are declared, not smoothed over.** 05.01's "no false equivalence"
//! says a reconstructed container with a freshly called model is not an exact process fork, and a
//! report must say which it had. `compare_suffixes` therefore returns the differences it found
//! rather than a verdict; deliberately there is no `comparable: bool`, because collapsing "the
//! branch simulated its payment" and "the branch inherited a different number of steps" into one
//! boolean is exactly the loss of information the blueprint warns about.

use crate::effect::{EffectOutcome, EffectPolicy, EffectRequest, PolicyDecision, Provenance};
use crate::error::RuntimeError;
use crate::host::{EffectSource, Host, RecordingHost};
use crate::tape::{TapeLineage, WorldTape};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};

/// Builds a child tape that begins at `step` of the parent.
pub fn fork_tape(
    parent: &WorldTape,
    step: u64,
    child_run: RunId,
) -> Result<WorldTape, RuntimeError> {
    if step > parent.len() {
        return Err(RuntimeError::ForkBeyondEnd {
            step,
            length: parent.len(),
        });
    }
    let entries = parent.prefix(step)?;
    let lineage = TapeLineage {
        parent_run: parent.run().clone(),
        forked_at_step: step,
        parent_head: parent.state_digest_at(step)?.to_string(),
    };
    Ok(WorldTape::forked(child_run, lineage, entries))
}

/// Opens a host that inherits a prefix and records a new suffix.
pub fn open_suffix<S: EffectSource>(
    parent: &WorldTape,
    step: u64,
    child_run: RunId,
    source: S,
    policy: EffectPolicy,
) -> Result<SuffixHost<S>, RuntimeError> {
    let tape = fork_tape(parent, step, child_run)?;
    Ok(SuffixHost {
        recording: RecordingHost::resuming(tape, source, policy),
        inherited: step,
        cursor: 0,
    })
}

/// A host whose first `inherited` steps come off the parent tape and whose later steps are live.
///
/// Two ways to use it, and 05.05 wants both:
///
/// - Re-walk the prefix with the original program, which *verifies* that the fork point is where
///   you think it is: every inherited request must match. Useful when the continuation shares code
///   with the parent.
/// - Call [`SuffixHost::resume_at_fork`] and start the continuation directly at the fork state,
///   handing it [`observable_state`] instead. This is the interesting case — a different
///   architecture does not have the parent's program to re-walk, it has the parent's *situation*.
#[derive(Debug)]
pub struct SuffixHost<S: EffectSource> {
    recording: RecordingHost<S>,
    inherited: u64,
    cursor: u64,
}

impl<S: EffectSource> SuffixHost<S> {
    /// The number of steps inherited from the parent.
    pub fn inherited(&self) -> u64 {
        self.inherited
    }

    /// True while the host is still walking the inherited prefix.
    pub fn is_replaying(&self) -> bool {
        self.cursor < self.inherited
    }

    /// Skips straight to the fork point without re-walking the prefix.
    pub fn resume_at_fork(&mut self) {
        self.cursor = self.inherited;
    }

    /// The parent's observable state at the fork point, for a continuation that does not share the
    /// parent's program.
    pub fn observed_state(&self) -> Result<ObservedState, RuntimeError> {
        observable_state(self.recording.tape(), self.inherited)
    }

    pub fn journal(&self) -> &[PolicyDecision] {
        self.recording.journal()
    }

    pub fn source(&self) -> &S {
        self.recording.source()
    }

    /// Ends the branch, insisting the inherited prefix was accounted for.
    pub fn finish(self) -> Result<WorldTape, RuntimeError> {
        if self.cursor < self.inherited {
            return Err(RuntimeError::SuffixNotReached {
                step: self.cursor,
                inherited: self.inherited,
            });
        }
        Ok(self.recording.into_tape())
    }
}

impl<S: EffectSource> Host for SuffixHost<S> {
    fn perform(&mut self, request: EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        if self.cursor < self.inherited {
            let entry = &self.recording.tape().entries()[self.cursor as usize];
            if entry.effect.request != request {
                return Err(RuntimeError::DivergentRequest {
                    step: self.cursor,
                    recorded: entry.effect.request.to_string(),
                    requested: request.to_string(),
                });
            }
            let outcome = entry.effect.outcome.clone();
            self.cursor += 1;
            return Ok(outcome);
        }
        self.recording.perform(request)
    }

    fn tape(&self) -> &WorldTape {
        self.recording.tape()
    }
}

/// The version of the prefix-to-context transformation.
///
/// 05.05 requires this transformation to be versioned and itself evaluable, because handing a
/// candidate architecture "the situation so far" is a modelling decision that can be done well or
/// badly, and a comparison across two different transformations is not a comparison of the agents.
pub const OBSERVABLE_STATE_VERSION: &str = "observable-state/1";

/// One step of the parent's history as a candidate architecture sees it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedStep {
    pub step: u64,
    pub request: EffectRequest,
    pub outcome: EffectOutcome,
    /// Carried through so a candidate is never shown a simulated answer as if it were observed.
    pub provenance: Provenance,
}

/// The captured state a fork hands to a continuation that did not run the prefix.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObservedState {
    pub version: String,
    pub fork_step: u64,
    pub state_digest: String,
    pub steps: Vec<ObservedStep>,
}

/// Transforms a tape prefix into the observable state at the fork point.
pub fn observable_state(tape: &WorldTape, step: u64) -> Result<ObservedState, RuntimeError> {
    let steps = tape
        .prefix(step)?
        .into_iter()
        .map(|entry| ObservedStep {
            step: entry.step,
            request: entry.effect.request,
            outcome: entry.effect.outcome,
            provenance: entry.effect.provenance,
        })
        .collect();
    Ok(ObservedState {
        version: OBSERVABLE_STATE_VERSION.to_string(),
        fork_step: step,
        state_digest: tape.state_digest_at(step)?.to_string(),
        steps,
    })
}

/// What a provider cache would have to agree on before reuse is legitimate (05.05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachePrefixKey {
    pub prefix_digest: String,
    pub model: String,
    pub tool_schema_digest: String,
    pub provider_policy: String,
}

impl CachePrefixKey {
    /// Builds the key for a fork point on a tape.
    pub fn for_fork(
        tape: &WorldTape,
        step: u64,
        model: impl Into<String>,
        tool_schema: &serde_json::Value,
        provider_policy: impl Into<String>,
    ) -> Result<Self, RuntimeError> {
        let tool_schema_digest = ContentHash::of_value(tool_schema)
            .map_err(|error| RuntimeError::Uncanonical(error.to_string()))?
            .as_str()
            .to_string();
        Ok(CachePrefixKey {
            prefix_digest: tape.state_digest_at(step)?.to_string(),
            model: model.into(),
            tool_schema_digest,
            provider_policy: provider_policy.into(),
        })
    }
}

/// Whether a cached prefix may be reused, and if not, why not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ReuseStatus {
    Reused,
    Rejected { reason: String },
}

/// Decides cache reuse, reporting the first field that disagreed.
///
/// Reporting *which* field disagreed rather than a bare `false` is the point: "reuse was rejected"
/// tells an operator nothing, while "the tool schema changed" tells them what to fix.
pub fn cache_reuse(recorded: &CachePrefixKey, candidate: &CachePrefixKey) -> ReuseStatus {
    let checks = [
        ("prefix", &recorded.prefix_digest, &candidate.prefix_digest),
        ("model", &recorded.model, &candidate.model),
        (
            "tool schema",
            &recorded.tool_schema_digest,
            &candidate.tool_schema_digest,
        ),
        (
            "provider policy",
            &recorded.provider_policy,
            &candidate.provider_policy,
        ),
    ];
    for (field, left, right) in checks {
        if left != right {
            return ReuseStatus::Rejected {
                reason: format!("{field} differs"),
            };
        }
    }
    ReuseStatus::Reused
}

/// The result of comparing two branches that claim a common ancestor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchedComparison {
    /// The last step at which the two tapes still agree, digest for digest.
    pub common_ancestor_step: u64,
    pub first_divergence: Option<u64>,
    pub left_steps: u64,
    pub right_steps: u64,
    pub left_simulated: Vec<u64>,
    pub right_simulated: Vec<u64>,
    /// Everything that makes the two branches less than exactly comparable.
    pub reconstruction_differences: Vec<String>,
}

/// Compares two branches, declaring what makes them less than exactly comparable.
pub fn compare_suffixes(left: &WorldTape, right: &WorldTape) -> MatchedComparison {
    let shared = left.entries().len().min(right.entries().len());
    let mut common = 0u64;
    while (common as usize) < shared
        && left.entries()[common as usize].digest == right.entries()[common as usize].digest
    {
        common += 1;
    }

    let mut differences = Vec::new();
    if left.inherited_steps() != right.inherited_steps() {
        differences.push(format!(
            "branches inherited different prefixes: {} steps against {}",
            left.inherited_steps(),
            right.inherited_steps()
        ));
    }
    if common < left.inherited_steps().min(right.inherited_steps()) {
        differences.push(format!(
            "branches claim a common ancestor but agree only to step {common}"
        ));
    }
    let left_simulated = simulated_after(left, common);
    let right_simulated = simulated_after(right, common);
    if !left_simulated.is_empty() || !right_simulated.is_empty() {
        differences.push(format!(
            "outcomes after the fork were simulated rather than performed: {} on the left, {} on the right",
            left_simulated.len(),
            right_simulated.len()
        ));
    }

    MatchedComparison {
        common_ancestor_step: common,
        first_divergence: left.first_divergence(right),
        left_steps: left.len(),
        right_steps: right.len(),
        left_simulated,
        right_simulated,
        reconstruction_differences: differences,
    }
}

fn simulated_after(tape: &WorldTape, step: u64) -> Vec<u64> {
    tape.entries()
        .iter()
        .filter(|entry| entry.step >= step && entry.effect.provenance == Provenance::Simulated)
        .map(|entry| entry.step)
        .collect()
}
