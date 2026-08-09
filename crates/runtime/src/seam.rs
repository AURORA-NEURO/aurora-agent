//! The named seams a program actually calls: clock, randomness, network, sandbox, external action.
//!
//! Blueprint 05.06 and 05.07. These are extension traits with default bodies over `Host`, blanket
//! implemented for every host. That shape is chosen for one reason: there is exactly one definition
//! of "read the clock", and recording, replaying and forked-suffix hosts all run *that* definition.
//! If each host implemented the seams itself, byte-identical replay would depend on three
//! implementations agreeing forever, which is the kind of promise that holds until it doesn't.
//!
//! The seams are also where a request's *shape* is fixed. `now_millis` always emits
//! `EffectRequest::ClockNow`, so a replay's request comparison is comparing like with like. A
//! program that constructs raw requests can still do so — the door is `Host::perform` — but then it
//! owns the shape, and the tape will hold it to it.

use crate::effect::{EffectOutcome, EffectRequest};
use crate::error::RuntimeError;
use crate::host::Host;
use serde_json::Value;

fn missing_field(request: &str, field: &str) -> RuntimeError {
    RuntimeError::SourceFailure {
        request: request.to_string(),
        reason: format!("outcome has no {field} field"),
    }
}

/// Virtual task time (05.07).
///
/// Task time is not wall time and is never derived from it. A task with a deadline must behave the
/// same on a loaded machine as on an idle one, otherwise the deadline is measuring the runner.
pub trait Clock: Host {
    /// Reads the task clock without advancing it.
    fn now_millis(&mut self) -> Result<u64, RuntimeError> {
        let outcome = self.perform(EffectRequest::ClockNow)?;
        outcome
            .integer("task_millis")
            .ok_or_else(|| missing_field("clock_now", "task_millis"))
    }

    /// Advances the task clock. Nothing actually waits; the clock is a number.
    fn sleep(&mut self, millis: u64) -> Result<u64, RuntimeError> {
        let outcome = self.perform(EffectRequest::ClockSleep { millis })?;
        outcome
            .integer("task_millis")
            .ok_or_else(|| missing_field("clock_sleep", "task_millis"))
    }
}

impl<H: Host + ?Sized> Clock for H {}

/// Entropy (05.07).
///
/// Seeding is preferred where a provider allows it, but seeding alone is not enough: providers do
/// not all expose their generators, so every entropy request is *recorded* as well. Recording is
/// what makes the guarantee hold across providers that could not be seeded.
pub trait Randomness: Host {
    /// Draws `count` bytes, returned as lowercase hex.
    ///
    /// Hex rather than a byte array so the recorded value has exactly one canonical encoding; a
    /// JSON array of integers would leave the tape's digest hostage to a serializer's choices.
    fn random_hex(&mut self, count: u32) -> Result<String, RuntimeError> {
        let outcome = self.perform(EffectRequest::RandomBytes { count })?;
        outcome
            .text("hex")
            .map(str::to_string)
            .ok_or_else(|| missing_field("random_bytes", "hex"))
    }

    fn random_u64(&mut self) -> Result<u64, RuntimeError> {
        let hex = self.random_hex(8)?;
        u64::from_str_radix(&hex, 16).map_err(|_| RuntimeError::SourceFailure {
            request: "random_bytes".into(),
            reason: format!("recorded value {hex:?} is not 8 bytes of hex"),
        })
    }
}

impl<H: Host + ?Sized> Randomness for H {}

/// Outbound requests, and model calls, which have the same determinism problem (05.07).
pub trait Network: Host {
    fn fetch(&mut self, method: &str, url: &str) -> Result<EffectOutcome, RuntimeError> {
        self.perform(EffectRequest::NetworkFetch {
            method: method.to_string(),
            url: url.to_string(),
        })
    }

    fn get_body(&mut self, url: &str) -> Result<String, RuntimeError> {
        let outcome = self.fetch("GET", url)?;
        outcome
            .text("body")
            .map(str::to_string)
            .ok_or_else(|| missing_field("network_fetch", "body"))
    }

    /// A model call. Recorded like any other answer the run did not compute itself.
    ///
    /// 05.07 notes that *model* nondeterminism is handled statistically rather than by replay — you
    /// cannot pin a sampler you do not own. What replay guarantees is narrower and still worth
    /// having: the second run sees the same text the first one saw, so everything downstream of the
    /// model is reproducible even when the model itself is not.
    fn call_model(&mut self, model: &str, prompt: &str) -> Result<String, RuntimeError> {
        let outcome = self.perform(EffectRequest::ModelCall {
            model: model.to_string(),
            prompt: prompt.to_string(),
        })?;
        outcome
            .text("text")
            .map(str::to_string)
            .ok_or_else(|| missing_field("model_call", "text"))
    }
}

impl<H: Host + ?Sized> Network for H {}

/// The task world: files, processes, local services (05.06).
pub trait Sandbox: Host {
    /// Reads a file. A missing file is `None`, not an error — absence is an answer, and recording
    /// it is what lets a replay reproduce a program that branches on it.
    fn read_file(&mut self, path: &str) -> Result<Option<String>, RuntimeError> {
        let outcome = self.perform(EffectRequest::FileRead {
            path: path.to_string(),
        })?;
        match outcome.field("found").and_then(Value::as_bool) {
            Some(true) => Ok(Some(
                outcome
                    .text("content")
                    .ok_or_else(|| missing_field("file_read", "content"))?
                    .to_string(),
            )),
            Some(false) => Ok(None),
            None => Err(missing_field("file_read", "found")),
        }
    }

    /// Writes a file into the copy-on-write overlay. Returns the byte length written.
    fn write_file(&mut self, path: &str, content: &str) -> Result<u64, RuntimeError> {
        let outcome = self.perform(EffectRequest::FileWrite {
            path: path.to_string(),
            content: content.to_string(),
        })?;
        outcome
            .integer("bytes")
            .ok_or_else(|| missing_field("file_write", "bytes"))
    }

    fn spawn(&mut self, program: &str, args: &[&str]) -> Result<EffectOutcome, RuntimeError> {
        self.perform(EffectRequest::ProcessSpawn {
            program: program.to_string(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
        })
    }

    fn call_service(
        &mut self,
        service: &str,
        operation: &str,
        request: Value,
    ) -> Result<EffectOutcome, RuntimeError> {
        self.perform(EffectRequest::ServiceCall {
            service: service.to_string(),
            operation: operation.to_string(),
            request,
        })
    }
}

impl<H: Host + ?Sized> Sandbox for H {}

/// Actions with no undo (05.08).
///
/// Present as a seam so that an agent *proposing* one is observable behaviour rather than a crash:
/// the intent reaches the policy, the policy refuses or simulates it, and the verdict is evidence.
/// An evaluation harness that made these unrepresentable could not benchmark restraint.
pub trait ExternalActions: Host {
    fn send_message(
        &mut self,
        channel: &str,
        recipient: &str,
        body: &str,
    ) -> Result<EffectOutcome, RuntimeError> {
        self.perform(EffectRequest::OutboundMessage {
            channel: channel.to_string(),
            recipient: recipient.to_string(),
            body: body.to_string(),
        })
    }

    fn pay(&mut self, account: &str, amount_micros: u64) -> Result<EffectOutcome, RuntimeError> {
        self.perform(EffectRequest::Payment {
            account: account.to_string(),
            amount_micros,
        })
    }
}

impl<H: Host + ?Sized> ExternalActions for H {}
