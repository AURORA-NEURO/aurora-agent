//! A deterministic in-process world: files, processes, services, entropy, clock, fixtures, faults.
//!
//! Blueprint 05.06 (filesystem, process and service virtualization) and the source side of 05.07.
//! This is the `EffectSource` behind the `InProcess` executor provider — the "live world" that a
//! recording run asks and a tape then remembers.
//!
//! It is deliberately made of maps rather than of syscalls. Nothing here opens a file on the host,
//! spawns a process, or resolves a hostname, and that is not a stub apology: 05.06's core semantics
//! are an *immutable base with copy-on-write deltas and a change journal*, and those are semantics,
//! not system calls. Implementing them in memory makes every test in this crate reproducible on any
//! machine, and keeps untrusted benchmark code away from the host filesystem by construction rather
//! than by an allowlist that has to be right.
//!
//! What is genuinely **not** implemented, and is not pretended: OCI layers, overlayfs mounts,
//! cgroups, process trees, signal handling, database snapshots, and service health checks. Those
//! require the container provider that 05.03 declares and this crate refuses to fake. A run that
//! needs them gets `ProviderUnavailable`, not a weaker world silently dressed as the real one.

use crate::effect::{EffectOutcome, EffectRequest};
use crate::error::RuntimeError;
use crate::host::EffectSource;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::fmt;

/// A deterministic disturbance the runtime can schedule (05.07's fault injection).
///
/// Scheduled by *call index*, not by time, so a fault-mutation pack reproduces exactly rather than
/// depending on how fast the machine happened to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Fault {
    /// The world takes too long and gives up.
    Timeout,
    /// The answer never arrives.
    Dropped,
    /// The answer arrives late; task time advances by `millis`.
    Delay { millis: u64 },
    /// The answer arrives truncated to `keep_bytes`.
    Truncated { keep_bytes: usize },
}

/// Maximum response size a random-byte request may ask the deterministic world to allocate.
pub const MAX_RANDOM_BYTES: u32 = 16 * 1024 * 1024;

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Timeout => f.write_str("timeout"),
            Fault::Dropped => f.write_str("dropped response"),
            Fault::Delay { millis } => write!(f, "delay of {millis}ms"),
            Fault::Truncated { keep_bytes } => {
                write!(f, "response truncated to {keep_bytes} bytes")
            }
        }
    }
}

/// One entry in the copy-on-write change journal (05.06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub call: u64,
    pub path: String,
    pub bytes: u64,
    /// Whether the path already existed, so a restore knows the difference between a create and an
    /// overwrite without diffing two whole trees.
    pub existed_before: bool,
}

/// The deterministic world an in-process trial runs against.
#[derive(Debug, Clone, Default)]
pub struct InProcessWorld {
    clock_millis: u64,
    clock_tick_millis: u64,
    rng_state: u64,
    base: BTreeMap<String, String>,
    delta: BTreeMap<String, String>,
    journal: Vec<FileChange>,
    fixtures: BTreeMap<String, String>,
    services: BTreeMap<String, Value>,
    faults: BTreeMap<u64, Fault>,
    calls: u64,
}

impl InProcessWorld {
    pub fn new() -> Self {
        InProcessWorld::default()
    }

    /// Seeds the generator. A seeded world is still recorded: 05.07 requires entropy requests on the
    /// tape because not every provider can be seeded, and a guarantee that only holds for the ones
    /// that can is not a guarantee.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.rng_state = seed;
        self
    }

    pub fn with_clock_start(mut self, millis: u64) -> Self {
        self.clock_millis = millis;
        self
    }

    /// Makes each clock read advance the task clock by `millis`.
    ///
    /// Zero by default, so a task clock moves only when the program asks it to. A run that never
    /// sleeps sees a frozen clock, which is the honest answer for virtual time.
    pub fn with_clock_tick(mut self, millis: u64) -> Self {
        self.clock_tick_millis = millis;
        self
    }

    /// Adds a file to the immutable base. Base files are never mutated; a write lands in the delta.
    pub fn with_base_file(mut self, path: impl Into<String>, content: impl Into<String>) -> Self {
        self.base.insert(path.into(), content.into());
        self
    }

    /// Records a network fixture, keyed by method and URL.
    pub fn with_fixture(
        mut self,
        method: &str,
        url: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        self.fixtures
            .insert(fixture_key(method, &url.into()), body.into());
        self
    }

    pub fn with_service(mut self, service: &str, operation: &str, response: Value) -> Self {
        self.services
            .insert(format!("{service}.{operation}"), response);
        self
    }

    /// Schedules a fault at a given call index (zero-based).
    pub fn with_fault_at(mut self, call: u64, fault: Fault) -> Self {
        self.faults.insert(call, fault);
        self
    }

    /// How many times the world was actually asked.
    ///
    /// Load-bearing for 05.05: a fork that replays a prefix must leave this untouched, because a
    /// counterfactual branch may not re-perform its ancestor's effects.
    pub fn calls(&self) -> u64 {
        self.calls
    }

    pub fn task_millis(&self) -> u64 {
        self.clock_millis
    }

    /// The change journal, in order.
    pub fn journal(&self) -> &[FileChange] {
        &self.journal
    }

    /// Resolves a path through the overlay: delta first, then the immutable base.
    pub fn file(&self, path: &str) -> Option<&str> {
        self.delta
            .get(path)
            .or_else(|| self.base.get(path))
            .map(String::as_str)
    }

    /// The portable state manifest 05.06 asks for: every visible path and its content digest.
    pub fn state_manifest(&self) -> BTreeMap<String, String> {
        let mut manifest: BTreeMap<String, String> = BTreeMap::new();
        for (path, content) in self.base.iter().chain(self.delta.iter()) {
            manifest.insert(
                path.clone(),
                ContentHash::of_bytes(content.as_bytes())
                    .as_str()
                    .to_string(),
            );
        }
        manifest
    }

    /// splitmix64. Chosen because it is four lines of arithmetic with no dependency and a fixed,
    /// documented output sequence, which is exactly what a reproducible fixture needs. It is not a
    /// cryptographic generator and nothing here pretends it is.
    fn next_u64(&mut self) -> u64 {
        self.rng_state = self.rng_state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.rng_state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn random_hex(&mut self, count: u32) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(count as usize * 2);
        for _ in 0..count {
            let byte = (self.next_u64() & 0xff) as u8;
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }

    fn apply_fault(&mut self, call: u64) -> Result<(), RuntimeError> {
        let Some(fault) = self.faults.get(&call).copied() else {
            return Ok(());
        };
        match fault {
            Fault::Delay { millis } => {
                self.clock_millis = self.clock_millis.checked_add(millis).ok_or_else(|| {
                    RuntimeError::InvariantViolation {
                        detail: "virtual task clock overflowed during a scheduled delay".into(),
                    }
                })?;
                Ok(())
            }
            // Truncation shapes the answer rather than preventing it, so it is applied at the
            // point the answer is built.
            Fault::Truncated { .. } => Ok(()),
            Fault::Timeout | Fault::Dropped => Err(RuntimeError::InjectedFault {
                call,
                fault: fault.to_string(),
            }),
        }
    }

    fn truncation_at(&self, call: u64) -> Option<usize> {
        match self.faults.get(&call) {
            Some(Fault::Truncated { keep_bytes }) => Some(*keep_bytes),
            _ => None,
        }
    }
}

fn fixture_key(method: &str, url: &str) -> String {
    format!("{} {url}", method.to_ascii_uppercase())
}

fn truncate(body: &str, keep_bytes: usize) -> String {
    let mut end = keep_bytes.min(body.len());
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body[..end].to_string()
}

impl EffectSource for InProcessWorld {
    fn perform(&mut self, request: &EffectRequest) -> Result<EffectOutcome, RuntimeError> {
        let call = self.calls;
        self.calls = self
            .calls
            .checked_add(1)
            .ok_or_else(|| RuntimeError::InvariantViolation {
                detail: "in-process world call counter overflowed".into(),
            })?;
        self.apply_fault(call)?;

        let outcome = match request {
            EffectRequest::ClockNow => {
                self.clock_millis = self
                    .clock_millis
                    .checked_add(self.clock_tick_millis)
                    .ok_or_else(|| RuntimeError::InvariantViolation {
                        detail: "virtual task clock overflowed during a clock read".into(),
                    })?;
                json!({ "task_millis": self.clock_millis })
            }
            EffectRequest::ClockSleep { millis } => {
                self.clock_millis = self.clock_millis.checked_add(*millis).ok_or_else(|| {
                    RuntimeError::InvariantViolation {
                        detail: "virtual task clock overflowed during sleep".into(),
                    }
                })?;
                json!({ "task_millis": self.clock_millis })
            }
            EffectRequest::RandomBytes { count } => {
                if *count > MAX_RANDOM_BYTES {
                    return Err(RuntimeError::InvariantViolation {
                        detail: format!(
                            "random-byte request exceeds the {} byte sandbox bound",
                            MAX_RANDOM_BYTES
                        ),
                    });
                }
                json!({ "hex": self.random_hex(*count) })
            }
            EffectRequest::FileRead { path } => match self.file(path) {
                Some(content) => json!({ "found": true, "content": content }),
                None => json!({ "found": false }),
            },
            EffectRequest::FileWrite { path, content } => {
                let existed_before = self.file(path).is_some();
                self.delta.insert(path.clone(), content.clone());
                self.journal.push(FileChange {
                    call,
                    path: path.clone(),
                    bytes: content.len() as u64,
                    existed_before,
                });
                json!({
                    "bytes": content.len() as u64,
                    "digest": ContentHash::of_bytes(content.as_bytes()).as_str(),
                    "created": !existed_before,
                })
            }
            EffectRequest::ProcessSpawn { program, args } => {
                let mut line = program.clone();
                for arg in args {
                    line.push(' ');
                    line.push_str(arg);
                }
                json!({ "exit_code": 0, "stdout": line })
            }
            EffectRequest::ServiceCall {
                service, operation, ..
            } => {
                let key = format!("{service}.{operation}");
                let Some(response) = self.services.get(&key) else {
                    return Err(RuntimeError::SourceFailure {
                        request: request.to_string(),
                        reason: format!("no service declared for {key}"),
                    });
                };
                json!({ "response": response })
            }
            EffectRequest::NetworkFetch { method, url } => {
                let key = fixture_key(method, url);
                let Some(body) = self.fixtures.get(&key).cloned() else {
                    return Err(RuntimeError::SourceFailure {
                        request: request.to_string(),
                        reason: format!("no recorded fixture for {key}"),
                    });
                };
                let body = match self.truncation_at(call) {
                    Some(keep) => truncate(&body, keep),
                    None => body,
                };
                json!({ "status": 200, "body": body })
            }
            EffectRequest::ModelCall { model, prompt } => {
                let digest = ContentHash::of_bytes(format!("{model}\u{1f}{prompt}").as_bytes());
                let reply = format!("{model}:{}", &digest.as_str()[..16]);
                json!({
                    "text": reply,
                    "tokens": prompt.split_whitespace().count() as u64,
                })
            }
            EffectRequest::OutboundMessage { .. } | EffectRequest::Payment { .. } => {
                // Belt and braces behind the policy: even if a plan permitted an irreversible
                // class, this world has no outside to act on and will not invent one.
                return Err(RuntimeError::SourceFailure {
                    request: request.to_string(),
                    reason: "the in-process world will not materialize an irreversible effect"
                        .into(),
                });
            }
        };

        Ok(EffectOutcome::new(outcome))
    }
}
