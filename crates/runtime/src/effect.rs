//! What a run does to the world, and what it is permitted to do.
//!
//! Blueprint 05.08 (effects, permissions and secret broker) classifies every side effect by
//! *reversibility*, not by cost or by how dangerous it sounds: pure/read-only, reversible sandbox
//! mutation, compensable external mutation, irreversible action. Evaluation policies generally
//! permit the first two and only simulated versions of the later ones.
//!
//! Two properties this module exists to enforce:
//!
//! 1. **Every effect declares its class, and an undeclared effect is refused.** The execution plan
//!    names the effect kinds a trial may reach for. A kind that was not declared does not reach the
//!    world at all — it is refused at authorization. Benchmark code is treated as hostile, so the
//!    default is deny and the plan must open each door explicitly.
//! 2. **A request is separated from its outcome.** The request is what the program asked for and is
//!    reproducible from the program alone; the outcome is what the world answered and is not. Only
//!    the second needs recording, and only the first can be checked against a tape during replay.
//!    Conflating them is what makes most "record and replay" harnesses silently go live.
//!
//! Deliberately **not** implemented here: rollback handlers for compensable effects (05.08's
//! "compensation"). Compensation needs a real external system to compensate against; the in-process
//! world has none, so declaring a `rollback` hook would be decoration. Compensable effects are
//! classified and can be simulated or refused, and that is the whole of the guarantee offered.

use crate::error::RuntimeError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// The reversibility tiers of blueprint 05.08.
///
/// Ordered from least to most consequential so that "at or above this tier" is expressible as a
/// comparison rather than a match arm per variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectClass {
    /// Reads that leave no trace: clocks, entropy, reading an immutable input.
    Pure,
    /// Mutations confined to the trial's own disposable world.
    ReversibleSandbox,
    /// External mutation that a compensating action could undo.
    CompensableExternal,
    /// Actions with no undo: outbound communication, payment, production writes.
    Irreversible,
}

impl fmt::Display for EffectClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            EffectClass::Pure => "pure",
            EffectClass::ReversibleSandbox => "reversible_sandbox",
            EffectClass::CompensableExternal => "compensable_external",
            EffectClass::Irreversible => "irreversible",
        };
        f.write_str(name)
    }
}

/// The tag of an effect request, without its payload.
///
/// Declarations in an execution plan are made at this granularity: a plan permits `file_write`, not
/// one specific write. Per-argument restriction is the job of the path allowlist and network mode,
/// which are checked separately and can therefore be reported separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    ClockNow,
    ClockSleep,
    RandomBytes,
    NetworkFetch,
    FileRead,
    FileWrite,
    ProcessSpawn,
    ServiceCall,
    ModelCall,
    OutboundMessage,
    Payment,
}

impl fmt::Display for EffectKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            EffectKind::ClockNow => "clock_now",
            EffectKind::ClockSleep => "clock_sleep",
            EffectKind::RandomBytes => "random_bytes",
            EffectKind::NetworkFetch => "network_fetch",
            EffectKind::FileRead => "file_read",
            EffectKind::FileWrite => "file_write",
            EffectKind::ProcessSpawn => "process_spawn",
            EffectKind::ServiceCall => "service_call",
            EffectKind::ModelCall => "model_call",
            EffectKind::OutboundMessage => "outbound_message",
            EffectKind::Payment => "payment",
        };
        f.write_str(name)
    }
}

/// The deterministic half of an effect: what the program asked for.
///
/// A request must be derivable from the program and its inputs alone. That is what lets a replay
/// check the program against the tape step by step: if the program asks for something the tape does
/// not record at that step, the program is not the program that was recorded, and saying so is more
/// useful than quietly answering the new question.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EffectRequest {
    /// Read the task clock. See 05.07: task time is virtual and separate from wall time.
    ClockNow,
    /// Advance the task clock. Nothing sleeps; the clock is a number.
    ClockSleep { millis: u64 },
    RandomBytes { count: u32 },
    NetworkFetch { method: String, url: String },
    FileRead { path: String },
    FileWrite { path: String, content: String },
    ProcessSpawn { program: String, args: Vec<String> },
    ServiceCall {
        service: String,
        operation: String,
        request: Value,
    },
    ModelCall { model: String, prompt: String },
    OutboundMessage {
        channel: String,
        recipient: String,
        body: String,
    },
    Payment { account: String, amount_micros: u64 },
}

impl EffectRequest {
    pub fn kind(&self) -> EffectKind {
        match self {
            EffectRequest::ClockNow => EffectKind::ClockNow,
            EffectRequest::ClockSleep { .. } => EffectKind::ClockSleep,
            EffectRequest::RandomBytes { .. } => EffectKind::RandomBytes,
            EffectRequest::NetworkFetch { .. } => EffectKind::NetworkFetch,
            EffectRequest::FileRead { .. } => EffectKind::FileRead,
            EffectRequest::FileWrite { .. } => EffectKind::FileWrite,
            EffectRequest::ProcessSpawn { .. } => EffectKind::ProcessSpawn,
            EffectRequest::ServiceCall { .. } => EffectKind::ServiceCall,
            EffectRequest::ModelCall { .. } => EffectKind::ModelCall,
            EffectRequest::OutboundMessage { .. } => EffectKind::OutboundMessage,
            EffectRequest::Payment { .. } => EffectKind::Payment,
        }
    }

    /// The reversibility tier this request belongs to.
    ///
    /// A model call is classed `Pure` on purpose: it changes nothing in the world and can be
    /// repeated. It is expensive, but expense is the budget controller's concern (05.09) and
    /// conflating the two would make cost look like a safety property.
    ///
    /// A `NetworkFetch` is classed by method, because a GET is a read and a POST may not be. This
    /// is a heuristic over an opaque remote, and it errs toward the more consequential tier for
    /// anything that is not a plainly safe method.
    pub fn class(&self) -> EffectClass {
        match self {
            EffectRequest::ClockNow
            | EffectRequest::ClockSleep { .. }
            | EffectRequest::RandomBytes { .. }
            | EffectRequest::FileRead { .. }
            | EffectRequest::ModelCall { .. } => EffectClass::Pure,
            EffectRequest::FileWrite { .. }
            | EffectRequest::ProcessSpawn { .. }
            | EffectRequest::ServiceCall { .. } => EffectClass::ReversibleSandbox,
            EffectRequest::NetworkFetch { method, .. } => {
                let safe = method.eq_ignore_ascii_case("get") || method.eq_ignore_ascii_case("head");
                if safe {
                    EffectClass::Pure
                } else {
                    EffectClass::CompensableExternal
                }
            }
            EffectRequest::OutboundMessage { .. } | EffectRequest::Payment { .. } => {
                EffectClass::Irreversible
            }
        }
    }

    /// The host an outbound request names, for allowlist checks.
    ///
    /// A deliberately small parse rather than a URL crate: the runtime builds offline against
    /// pinned dependencies and adding a parser for one field is not worth the surface. The one
    /// subtlety that matters for security is userinfo — `https://allowed.test@evil.test/` is a
    /// request to `evil.test` — so the authority is split on `@` and the *last* segment wins.
    pub fn target_host(&self) -> Option<String> {
        let EffectRequest::NetworkFetch { url, .. } = self else {
            return None;
        };
        let after_scheme = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
        let authority = after_scheme
            .split(['/', '?', '#'])
            .next()
            .unwrap_or(after_scheme);
        let host_and_port = authority.rsplit('@').next().unwrap_or(authority);
        let host = host_and_port
            .rsplit_once(':')
            .map_or(host_and_port, |(host, _)| host);
        Some(host.to_ascii_lowercase())
    }

    /// The path a filesystem request names, for allowlist checks.
    pub fn target_path(&self) -> Option<&str> {
        match self {
            EffectRequest::FileRead { path } | EffectRequest::FileWrite { path, .. } => Some(path),
            _ => None,
        }
    }
}

impl fmt::Display for EffectRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EffectRequest::ClockNow => write!(f, "clock_now"),
            EffectRequest::ClockSleep { millis } => write!(f, "clock_sleep({millis}ms)"),
            EffectRequest::RandomBytes { count } => write!(f, "random_bytes({count})"),
            EffectRequest::NetworkFetch { method, url } => {
                write!(f, "network_fetch({method} {url})")
            }
            EffectRequest::FileRead { path } => write!(f, "file_read({path})"),
            EffectRequest::FileWrite { path, .. } => write!(f, "file_write({path})"),
            EffectRequest::ProcessSpawn { program, .. } => write!(f, "process_spawn({program})"),
            EffectRequest::ServiceCall {
                service, operation, ..
            } => write!(f, "service_call({service}.{operation})"),
            EffectRequest::ModelCall { model, .. } => write!(f, "model_call({model})"),
            EffectRequest::OutboundMessage {
                channel, recipient, ..
            } => write!(f, "outbound_message({channel} -> {recipient})"),
            EffectRequest::Payment { account, .. } => write!(f, "payment({account})"),
        }
    }
}

/// The nondeterministic half of an effect: what the world answered.
///
/// Held as canonical JSON so that a tape's digest is stable across machines and languages, which is
/// the whole basis of the byte-identical replay claim. Binary answers are carried as lowercase hex
/// rather than as a JSON array of integers, so the encoding does not depend on a serializer's mood.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectOutcome(Value);

impl EffectOutcome {
    pub fn new(value: Value) -> Self {
        EffectOutcome(value)
    }

    pub fn value(&self) -> &Value {
        &self.0
    }

    pub fn into_value(self) -> Value {
        self.0
    }

    pub fn field(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.0.get(name).and_then(Value::as_str)
    }

    pub fn integer(&self, name: &str) -> Option<u64> {
        self.0.get(name).and_then(Value::as_u64)
    }
}

/// How a plan treats effects above its permitted tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterializationPolicy {
    /// The effect is refused. The trial fails rather than pretending.
    Refuse,
    /// The effect is answered by the runtime without touching the world, and the tape records that
    /// the answer was invented. 05.05 requires this for forks: a counterfactual branch must never
    /// repeat a real-world side effect.
    Simulate,
}

/// The network modes of blueprint 05.07.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum NetworkMode {
    /// No outbound access at all. The default, and the only mode with no contamination question.
    Denied,
    Allowlist { hosts: BTreeSet<String> },
    /// Answers come from recorded fixtures; a miss fails rather than reaching the network.
    RecordedFixture,
    DeterministicEmulator,
    /// Reserved for specially governed packs. Results from an unrestricted run carry a
    /// contamination risk that no downstream analysis can remove.
    Unrestricted,
}

impl fmt::Display for NetworkMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NetworkMode::Denied => f.write_str("denied"),
            NetworkMode::Allowlist { hosts } => {
                write!(f, "allowlist({})", hosts.len())
            }
            NetworkMode::RecordedFixture => f.write_str("recorded_fixture"),
            NetworkMode::DeterministicEmulator => f.write_str("deterministic_emulator"),
            NetworkMode::Unrestricted => f.write_str("unrestricted"),
        }
    }
}

/// The verdict on one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authorization {
    /// Reach for the world.
    Perform,
    /// Answer without reaching for the world, and say so on the tape.
    Simulate,
}

/// What an execution plan permits.
///
/// Constructed deny-by-default: `EffectPolicy::evaluation_default()` declares no kind, permits only
/// the two harmless tiers, denies the network and refuses irreversible actions. Everything a trial
/// may do has to be written down, which is also what makes a trial's permissions comparable across
/// architectures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectPolicy {
    declared: BTreeSet<EffectKind>,
    permitted_classes: BTreeSet<EffectClass>,
    path_allowlist: BTreeSet<String>,
    network: NetworkMode,
    materialization: MaterializationPolicy,
}

impl EffectPolicy {
    /// The policy a benchmark trial gets unless something explicitly widens it.
    pub fn evaluation_default() -> Self {
        EffectPolicy {
            declared: BTreeSet::new(),
            permitted_classes: [EffectClass::Pure, EffectClass::ReversibleSandbox]
                .into_iter()
                .collect(),
            path_allowlist: BTreeSet::new(),
            network: NetworkMode::Denied,
            materialization: MaterializationPolicy::Refuse,
        }
    }

    pub fn declaring<I: IntoIterator<Item = EffectKind>>(mut self, kinds: I) -> Self {
        self.declared.extend(kinds);
        self
    }

    pub fn permitting_class(mut self, class: EffectClass) -> Self {
        self.permitted_classes.insert(class);
        self
    }

    /// Adds a path prefix the trial may read or write under.
    pub fn allowing_path(mut self, prefix: impl Into<String>) -> Self {
        self.path_allowlist.insert(prefix.into());
        self
    }

    pub fn with_network(mut self, network: NetworkMode) -> Self {
        self.network = network;
        self
    }

    pub fn with_materialization(mut self, materialization: MaterializationPolicy) -> Self {
        self.materialization = materialization;
        self
    }

    pub fn declares(&self, kind: EffectKind) -> bool {
        self.declared.contains(&kind)
    }

    pub fn network(&self) -> &NetworkMode {
        &self.network
    }

    pub fn materialization(&self) -> MaterializationPolicy {
        self.materialization
    }

    /// Decides one request, or refuses it with a reason that names what was violated.
    ///
    /// The order of checks is chosen so the *most structural* refusal is reported first. A request
    /// for an undeclared kind is a plan error, and reporting "path denied" for it would send the
    /// reader to the wrong file.
    pub fn authorize(&self, request: &EffectRequest) -> Result<Authorization, RuntimeError> {
        let kind = request.kind();
        if !self.declared.contains(&kind) {
            return Err(RuntimeError::UndeclaredEffect { kind });
        }

        let class = request.class();
        if !self.permitted_classes.contains(&class) {
            return match (self.materialization, class) {
                (MaterializationPolicy::Simulate, _) => Ok(Authorization::Simulate),
                (MaterializationPolicy::Refuse, EffectClass::Irreversible) => {
                    Err(RuntimeError::IrreversibleRefused { kind })
                }
                (MaterializationPolicy::Refuse, _) => {
                    Err(RuntimeError::ClassForbidden { class, kind })
                }
            };
        }

        if let Some(path) = request.target_path() {
            let allowed = is_canonical_path(path)
                && self
                    .path_allowlist
                    .iter()
                    .any(|prefix| path.starts_with(prefix.as_str()));
            if !allowed {
                return Err(RuntimeError::PathDenied {
                    path: path.to_string(),
                });
            }
        }

        if let Some(host) = request.target_host() {
            let permitted = match &self.network {
                NetworkMode::Denied => false,
                NetworkMode::Allowlist { hosts } => hosts.contains(&host),
                NetworkMode::RecordedFixture
                | NetworkMode::DeterministicEmulator
                | NetworkMode::Unrestricted => true,
            };
            if !permitted {
                return Err(RuntimeError::NetworkDenied {
                    mode: self.network.to_string(),
                    host,
                });
            }
        }

        Ok(Authorization::Perform)
    }

    /// The answer a simulated effect receives.
    ///
    /// Derived from the request alone, so two runs that simulate the same action agree, and marked
    /// so no reader can mistake it for something that happened.
    pub fn simulated_outcome(request: &EffectRequest) -> EffectOutcome {
        EffectOutcome::new(serde_json::json!({
            "simulated": true,
            "kind": request.kind(),
            "summary": request.to_string(),
        }))
    }
}

impl Default for EffectPolicy {
    fn default() -> Self {
        Self::evaluation_default()
    }
}

/// Whether a path is already in the only form the allowlist will consider.
///
/// The blueprint (05.06) says "path allowlists" and stops there, which under-specifies the part
/// that matters: a prefix allowlist alone is defeated by `/work/../etc/shadow`, which starts with
/// `/work/` and is not in `/work/` at all. Rather than normalize and hope the normalizer agrees
/// with whatever eventually resolves the path, this refuses anything that is not *already*
/// absolute, traversal-free and free of empty or `.` segments. Two implementations that disagree
/// about how to canonicalize a path is exactly how sandbox escapes happen; refusing the ambiguous
/// input means there is nothing to disagree about.
///
/// Backslash is rejected inside a segment because a virtual path that later reaches a Windows host
/// would gain a separator it did not have here.
fn is_canonical_path(path: &str) -> bool {
    let Some(rest) = path.strip_prefix('/') else {
        return false;
    };
    if rest.is_empty() {
        return false;
    }
    rest.split('/').all(|segment| {
        !segment.is_empty() && segment != "." && segment != ".." && !segment.contains('\\')
    })
}

/// What the policy decided, kept as evidence.
///
/// 05.08 makes denials, approvals and escalations first-class events precisely so that "the agent
/// tried to wire money and was stopped" is benchmarkable behaviour rather than a swallowed error.
///
/// These live in their own journal rather than on the WorldTape, and that is deliberate: the tape
/// must contain exactly the effects a replay needs to consume. A denial produced no outcome, and
/// its verdict is recomputable from the policy, so putting it on the tape would make the tape
/// unreplayable under any other policy while adding nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PolicyDecision {
    pub seq: u64,
    pub request: EffectRequest,
    pub class: EffectClass,
    pub outcome: DecisionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum DecisionOutcome {
    Permitted,
    Simulated,
    Denied { reason: String },
}

/// One effect as it happened, request and answer together.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub class: EffectClass,
    pub request: EffectRequest,
    pub outcome: EffectOutcome,
    pub provenance: Provenance,
}

impl Effect {
    pub fn performed(request: EffectRequest, outcome: EffectOutcome) -> Self {
        Effect {
            class: request.class(),
            request,
            outcome,
            provenance: Provenance::Performed,
        }
    }

    pub fn simulated(request: EffectRequest) -> Self {
        let outcome = EffectPolicy::simulated_outcome(&request);
        Effect {
            class: request.class(),
            request,
            outcome,
            provenance: Provenance::Simulated,
        }
    }
}

/// Whether the world was actually touched.
///
/// 05.01's "no false equivalence" rule in miniature: a reconstructed answer and a real one are both
/// on the tape, and the tape says which is which. Reports that aggregate over a tape must not have
/// to guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Performed,
    Simulated,
}
