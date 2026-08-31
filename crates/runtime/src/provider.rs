//! The contract an execution backend implements, and the one backend that actually exists.
//!
//! Blueprint 05.03 (executor provider interface) and 05.01 (runtime strategy). PRISM is supposed to
//! submit an execution plan and receive a normalized event stream, state handles, artifacts and a
//! termination record, whether the work ran in this process, in a subprocess, or in a container on
//! somebody else's machine.
//!
//! Only `InProcess` is implemented. `Subprocess` and `Container` are declared, and every one of
//! their methods returns [`RuntimeError::ProviderUnavailable`]. That is the entire point of writing
//! them out: a plan that asked for container isolation and quietly got a thread would produce
//! results that look like container results, and 05.01's "no false equivalence" rule exists because
//! that failure is silent and permanent. An error is recoverable; a mislabelled result is not.
//!
//! `capabilities()` on an unavailable provider reports [`Capabilities::none`] rather than what a
//! finished implementation would offer, so a planner that selects on capability never selects a
//! provider that cannot run. [`ExecutorProvider::is_available`] states the same fact directly.

use crate::budget::BudgetPlan;
use crate::effect::{EffectPolicy, EffectRequest};
use crate::error::RuntimeError;
use crate::host::RecordingHost;
use crate::orchestrator::{AttemptId, TrialId};
use crate::sandbox::InProcessWorld;
use crate::tape::{Checkpoint, RestorationDeclaration, TapeEntry, WorldTape};
use bioprism_ids::{ContentHash, RunId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a provider can actually do (05.03).
///
/// A flat set of booleans rather than a free-form string list, so "does this provider support
/// process checkpoints" is a question with an answer rather than a grep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Capabilities {
    pub process_isolation: bool,
    pub container_isolation: bool,
    pub gpu: bool,
    pub network_fixtures: bool,
    pub filesystem_snapshots: bool,
    pub process_checkpoints: bool,
    pub live_streaming: bool,
    pub nested_forks: bool,
    pub state_merge: bool,
    pub cache_reuse: bool,
}

impl Capabilities {
    /// The honest answer for a provider that is not implemented.
    pub fn none() -> Self {
        Capabilities::default()
    }
}

/// The frozen description of one trial's execution (05.02's "freeze the manifest before dispatch").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub run: RunId,
    pub trial: TrialId,
    pub attempt: AttemptId,
    pub policy: EffectPolicy,
    pub budget: BudgetPlan,
    /// The seed for any generator the provider controls. Recorded even when the provider ignores
    /// it, because "we asked for seed 7 and the provider could not be seeded" is itself evidence.
    pub seed: u64,
    /// Capabilities the plan requires. A provider missing any of them refuses the plan.
    pub required: Capabilities,
}

impl ExecutionPlan {
    pub fn new(run: RunId, trial: TrialId, attempt: AttemptId) -> Self {
        ExecutionPlan {
            run,
            trial,
            attempt,
            policy: EffectPolicy::evaluation_default(),
            budget: BudgetPlan::new(),
            seed: 0,
            required: Capabilities::none(),
        }
    }

    pub fn with_policy(mut self, policy: EffectPolicy) -> Self {
        self.policy = policy;
        self
    }

    pub fn with_budget(mut self, budget: BudgetPlan) -> Self {
        self.budget = budget;
        self
    }

    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn requiring(mut self, required: Capabilities) -> Self {
        self.required = required;
        self
    }

    /// The plan's immutable digest, used as the identity of the frozen manifest.
    pub fn digest(&self) -> Result<String, RuntimeError> {
        self.budget.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RuntimeError::Uncanonical(error.to_string()))?;
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| RuntimeError::Uncanonical(error.to_string()))
    }
}

/// A provider-issued reference to a piece of trial state.
///
/// 05.03 insists these are wrapped rather than opaque: an unadorned provider blob tells a reader
/// nothing about whether the state can still be restored, by whom, or what it is state *of*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateHandle {
    pub provider: String,
    pub capability_version: u32,
    pub trial: TrialId,
    pub attempt: AttemptId,
    pub parent: Option<String>,
    /// The tape head this handle commits to.
    pub commitment: String,
    pub restoration: RestorationDeclaration,
}

/// A file the trial produced or consumed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    pub path: String,
    pub digest: String,
    pub bytes: u64,
    /// False when the trial only read it.
    pub created: bool,
}

/// The lifecycle every backend implements identically (05.03's core methods).
pub trait ExecutorProvider {
    fn provider_id(&self) -> &str;

    fn capabilities(&self) -> Capabilities;

    /// Whether this provider can run anything at all.
    ///
    /// Default true. Declared-but-unimplemented providers override it to false, so a planner can
    /// ask before dispatching rather than discovering it from an error.
    fn is_available(&self) -> bool {
        true
    }

    fn prepare(&mut self, plan: &ExecutionPlan) -> Result<StateHandle, RuntimeError>;
    fn start(&mut self, handle: &StateHandle) -> Result<(), RuntimeError>;
    /// The normalized event stream from `cursor` onward.
    fn events(&self, handle: &StateHandle, cursor: u64) -> Result<Vec<TapeEntry>, RuntimeError>;
    fn checkpoint(&mut self, handle: &StateHandle) -> Result<Checkpoint, RuntimeError>;
    fn resume(&mut self, checkpoint: &Checkpoint) -> Result<StateHandle, RuntimeError>;
    fn cancel(&mut self, handle: &StateHandle) -> Result<(), RuntimeError>;
    fn collect(&mut self, handle: &StateHandle) -> Result<Vec<Artifact>, RuntimeError>;
    fn destroy(&mut self, handle: &StateHandle) -> Result<(), RuntimeError>;
}

const IN_PROCESS: &str = "in_process";
const CAPABILITY_VERSION: u32 = 1;

#[derive(Debug)]
struct InProcessTrial {
    plan: ExecutionPlan,
    tape: WorldTape,
    started: bool,
    cancelled: bool,
}

/// The reference provider: everything happens in this process, against `InProcessWorld`.
#[derive(Debug, Default)]
pub struct InProcessProvider {
    trials: BTreeMap<String, InProcessTrial>,
}

impl InProcessProvider {
    pub fn new() -> Self {
        InProcessProvider::default()
    }

    fn trial(&self, handle: &StateHandle) -> Result<&InProcessTrial, RuntimeError> {
        let trial =
            self.trials
                .get(handle.trial.as_str())
                .ok_or_else(|| RuntimeError::UnknownHandle {
                    provider: IN_PROCESS.to_string(),
                    trial: handle.trial.as_str().to_string(),
                })?;
        validate_handle(handle, trial)?;
        Ok(trial)
    }

    fn trial_mut(&mut self, handle: &StateHandle) -> Result<&mut InProcessTrial, RuntimeError> {
        let trial = self.trials.get_mut(handle.trial.as_str()).ok_or_else(|| {
            RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: handle.trial.as_str().to_string(),
            }
        })?;
        validate_handle(handle, trial)?;
        Ok(trial)
    }

    fn handle_for(&self, trial: &InProcessTrial) -> StateHandle {
        StateHandle {
            provider: IN_PROCESS.to_string(),
            capability_version: CAPABILITY_VERSION,
            trial: trial.plan.trial.clone(),
            attempt: trial.plan.attempt.clone(),
            parent: trial.tape.lineage().map(|l| l.parent_head.clone()),
            commitment: trial.tape.head().to_string(),
            restoration: RestorationDeclaration::portable(),
        }
    }

    /// Hands out a recording host for a started trial.
    ///
    /// Ownership is explicit — `open` lends the tape, `commit` takes it back — rather than the
    /// provider handing out an interior-mutable reference. A trial's tape must have exactly one
    /// writer at a time, and making that a move rather than a convention means it is checked.
    pub fn open(
        &mut self,
        handle: &StateHandle,
        world: InProcessWorld,
    ) -> Result<RecordingHost<InProcessWorld>, RuntimeError> {
        let trial = self.trial(handle)?;
        if !trial.started || trial.cancelled {
            return Err(RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: format!("{} (not active)", handle.trial.as_str()),
            });
        }
        let policy = trial.plan.policy.clone();
        let tape = trial.tape.clone();
        Ok(RecordingHost::resuming(tape, world, policy))
    }

    /// Returns a tape to the trial it belongs to.
    pub fn commit(&mut self, handle: &StateHandle, tape: WorldTape) -> Result<(), RuntimeError> {
        let trial = self.trial_mut(handle)?;
        if !trial.started {
            return Err(RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: format!("{} (prepared but not started)", handle.trial.as_str()),
            });
        }
        if tape.run() != &trial.plan.run {
            return Err(RuntimeError::InvariantViolation {
                detail: format!(
                    "cannot commit tape for run {} to trial run {}",
                    tape.run().as_str(),
                    trial.plan.run.as_str()
                ),
            });
        }
        if tape.len() < trial.tape.len()
            || tape.entries().get(..trial.tape.entries().len()) != Some(trial.tape.entries())
        {
            return Err(RuntimeError::InvariantViolation {
                detail: format!(
                    "cannot commit a tape that does not preserve the provider-owned prefix of {} steps",
                    trial.tape.len()
                ),
            });
        }
        tape.verify_chain()?;
        for checkpoint in tape.checkpoints() {
            tape.verify_checkpoint(checkpoint)?;
        }
        trial.tape = tape;
        Ok(())
    }

    /// The trial's tape, for callers assembling evidence.
    pub fn tape(&self, handle: &StateHandle) -> Result<&WorldTape, RuntimeError> {
        Ok(&self.trial(handle)?.tape)
    }
}

impl ExecutorProvider for InProcessProvider {
    fn provider_id(&self) -> &str {
        IN_PROCESS
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            process_isolation: false,
            container_isolation: false,
            gpu: false,
            network_fixtures: true,
            filesystem_snapshots: true,
            process_checkpoints: false,
            live_streaming: true,
            nested_forks: true,
            state_merge: false,
            cache_reuse: true,
        }
    }

    fn prepare(&mut self, plan: &ExecutionPlan) -> Result<StateHandle, RuntimeError> {
        plan.budget.validate()?;
        if self.trials.contains_key(plan.trial.as_str()) {
            return Err(RuntimeError::InvariantViolation {
                detail: format!("trial {} is already prepared", plan.trial.as_str()),
            });
        }
        let available = self.capabilities();
        for (needed, has, name) in [
            (
                plan.required.container_isolation,
                available.container_isolation,
                "container_isolation",
            ),
            (
                plan.required.process_isolation,
                available.process_isolation,
                "process_isolation",
            ),
            (plan.required.gpu, available.gpu, "gpu"),
            (
                plan.required.process_checkpoints,
                available.process_checkpoints,
                "process_checkpoints",
            ),
            (
                plan.required.state_merge,
                available.state_merge,
                "state_merge",
            ),
        ] {
            if needed && !has {
                return Err(RuntimeError::CapabilityUnsupported {
                    provider: IN_PROCESS.to_string(),
                    capability: name.to_string(),
                });
            }
        }

        let trial = InProcessTrial {
            plan: plan.clone(),
            tape: WorldTape::new(plan.run.clone()),
            started: false,
            cancelled: false,
        };
        let handle = self.handle_for(&trial);
        self.trials.insert(plan.trial.as_str().to_string(), trial);
        Ok(handle)
    }

    fn start(&mut self, handle: &StateHandle) -> Result<(), RuntimeError> {
        let trial = self.trial_mut(handle)?;
        if trial.cancelled {
            return Err(RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: format!("{} (cancelled)", handle.trial.as_str()),
            });
        }
        trial.started = true;
        Ok(())
    }

    fn events(&self, handle: &StateHandle, cursor: u64) -> Result<Vec<TapeEntry>, RuntimeError> {
        let trial = self.trial(handle)?;
        let length = trial.tape.len();
        if cursor > length {
            return Err(RuntimeError::StepOutOfRange {
                step: cursor,
                length,
            });
        }
        Ok(trial.tape.entries()[cursor as usize..].to_vec())
    }

    fn checkpoint(&mut self, handle: &StateHandle) -> Result<Checkpoint, RuntimeError> {
        let trial = self.trial_mut(handle)?;
        if !trial.started || trial.cancelled {
            return Err(RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: format!("{} (not active)", handle.trial.as_str()),
            });
        }
        trial
            .tape
            .checkpoint(IN_PROCESS, RestorationDeclaration::portable())
    }

    /// Restores by rewinding to the checkpointed prefix.
    ///
    /// Legitimate for this provider only because its state *is* the tape: `RestorationDeclaration`
    /// says `portable`, and it is telling the truth. A provider whose state lives in a container
    /// filesystem could not honestly make the same claim, which is why the declaration is a field
    /// rather than an assumption.
    fn resume(&mut self, checkpoint: &Checkpoint) -> Result<StateHandle, RuntimeError> {
        let matching_trials: Vec<String> = self
            .trials
            .iter()
            .filter_map(|(trial_id, trial)| {
                trial
                    .tape
                    .checkpoints()
                    .iter()
                    .any(|existing| existing.id == checkpoint.id)
                    .then_some(trial_id.clone())
            })
            .collect();
        let trial_id = match matching_trials.as_slice() {
            [] => {
                return Err(RuntimeError::CorruptCheckpoint {
                    id: checkpoint.id.clone(),
                    expected: checkpoint.tape_head.clone(),
                    found: "no trial holds this checkpoint".to_string(),
                });
            }
            [trial_id] => trial_id,
            _ => {
                return Err(RuntimeError::InvariantViolation {
                    detail: format!(
                        "checkpoint {} is ambiguous across {} trials",
                        checkpoint.id,
                        matching_trials.len()
                    ),
                });
            }
        };
        let trial =
            self.trials
                .get_mut(trial_id)
                .ok_or_else(|| RuntimeError::CorruptCheckpoint {
                    id: checkpoint.id.clone(),
                    expected: checkpoint.tape_head.clone(),
                    found: "checkpoint holder disappeared before resume".to_string(),
                })?;
        if trial.cancelled {
            return Err(RuntimeError::UnknownHandle {
                provider: IN_PROCESS.to_string(),
                trial: format!("{} (cancelled)", trial.plan.trial.as_str()),
            });
        }
        trial.tape.verify_checkpoint(checkpoint)?;
        let existing = trial
            .tape
            .checkpoints()
            .iter()
            .find(|existing| existing.id == checkpoint.id)
            .ok_or_else(|| RuntimeError::CorruptCheckpoint {
                id: checkpoint.id.clone(),
                expected: checkpoint.tape_head.clone(),
                found: "checkpoint disappeared before resume".to_string(),
            })?;
        if existing != checkpoint {
            return Err(RuntimeError::InvariantViolation {
                detail: format!(
                    "checkpoint {} metadata does not match the provider-owned checkpoint",
                    checkpoint.id
                ),
            });
        }
        trial.tape.rewind_to(checkpoint.step)?;
        let plan = trial.plan.clone();
        let head = trial.tape.state_digest_at(checkpoint.step)?.to_string();
        Ok(StateHandle {
            provider: IN_PROCESS.to_string(),
            capability_version: CAPABILITY_VERSION,
            trial: plan.trial,
            attempt: plan.attempt,
            parent: Some(checkpoint.tape_head.clone()),
            commitment: head,
            restoration: RestorationDeclaration::portable(),
        })
    }

    fn cancel(&mut self, handle: &StateHandle) -> Result<(), RuntimeError> {
        let trial = self.trial_mut(handle)?;
        trial.started = false;
        trial.cancelled = true;
        Ok(())
    }

    fn collect(&mut self, handle: &StateHandle) -> Result<Vec<Artifact>, RuntimeError> {
        let trial = self.trial(handle)?;
        let mut artifacts: BTreeMap<String, Artifact> = BTreeMap::new();
        for entry in trial.tape.entries() {
            match &entry.effect.request {
                EffectRequest::FileRead { path } => {
                    artifacts.entry(path.clone()).or_insert(Artifact {
                        path: path.clone(),
                        digest: String::new(),
                        bytes: 0,
                        created: false,
                    });
                }
                EffectRequest::FileWrite { path, content } => {
                    artifacts.insert(
                        path.clone(),
                        Artifact {
                            path: path.clone(),
                            digest: ContentHash::of_bytes(content.as_bytes())
                                .as_str()
                                .to_string(),
                            bytes: content.len() as u64,
                            created: true,
                        },
                    );
                }
                _ => {}
            }
        }
        Ok(artifacts.into_values().collect())
    }

    fn destroy(&mut self, handle: &StateHandle) -> Result<(), RuntimeError> {
        self.trial(handle)?;
        self.trials.remove(handle.trial.as_str());
        Ok(())
    }
}

fn validate_handle(handle: &StateHandle, trial: &InProcessTrial) -> Result<(), RuntimeError> {
    if handle.provider != IN_PROCESS
        || handle.capability_version != CAPABILITY_VERSION
        || handle.attempt != trial.plan.attempt
    {
        return Err(RuntimeError::UnknownHandle {
            provider: IN_PROCESS.to_string(),
            trial: handle.trial.as_str().to_string(),
        });
    }
    Ok(())
}

/// Declared, not implemented: isolation in a child process.
///
/// Kept in the tree so the gap is visible in the type system rather than in a roadmap document.
#[derive(Debug, Default)]
pub struct SubprocessProvider;

/// Declared, not implemented: isolation in an OCI container.
#[derive(Debug, Default)]
pub struct ContainerProvider;

macro_rules! unavailable_provider {
    ($provider:ty, $id:literal) => {
        impl ExecutorProvider for $provider {
            fn provider_id(&self) -> &str {
                $id
            }

            fn capabilities(&self) -> Capabilities {
                Capabilities::none()
            }

            fn is_available(&self) -> bool {
                false
            }

            fn prepare(&mut self, _plan: &ExecutionPlan) -> Result<StateHandle, RuntimeError> {
                Err(unavailable($id, "prepare"))
            }

            fn start(&mut self, _handle: &StateHandle) -> Result<(), RuntimeError> {
                Err(unavailable($id, "start"))
            }

            fn events(
                &self,
                _handle: &StateHandle,
                _cursor: u64,
            ) -> Result<Vec<TapeEntry>, RuntimeError> {
                Err(unavailable($id, "events"))
            }

            fn checkpoint(&mut self, _handle: &StateHandle) -> Result<Checkpoint, RuntimeError> {
                Err(unavailable($id, "checkpoint"))
            }

            fn resume(&mut self, _checkpoint: &Checkpoint) -> Result<StateHandle, RuntimeError> {
                Err(unavailable($id, "resume"))
            }

            fn cancel(&mut self, _handle: &StateHandle) -> Result<(), RuntimeError> {
                Err(unavailable($id, "cancel"))
            }

            fn collect(&mut self, _handle: &StateHandle) -> Result<Vec<Artifact>, RuntimeError> {
                Err(unavailable($id, "collect"))
            }

            fn destroy(&mut self, _handle: &StateHandle) -> Result<(), RuntimeError> {
                Err(unavailable($id, "destroy"))
            }
        }
    };
}

fn unavailable(provider: &str, operation: &str) -> RuntimeError {
    RuntimeError::ProviderUnavailable {
        provider: provider.to_string(),
        operation: operation.to_string(),
    }
}

unavailable_provider!(SubprocessProvider, "subprocess");
unavailable_provider!(ContainerProvider, "container");
