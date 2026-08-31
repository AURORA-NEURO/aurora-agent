//! Isolation *requests*, recorded and never enforced.
//!
//! Blueprint 40.16's fourth invariant is "untrusted code is isolated", 11.12 asks for timeouts,
//! resource limits and a circuit breaker, and 11.13 wants untrusted plugins in "a plugin sandbox
//! or worker process".
//!
//! # This crate implements none of that, and here is the precise statement
//!
//! `bioprism-sdk` is a library of plain Rust types. It does not spawn processes, load shared
//! objects, instantiate WebAssembly, set resource limits, install seccomp filters, open sockets or
//! touch the filesystem. A plugin registered through this crate is *already* code the host
//! compiled and linked into its own address space, and no data structure in this module can
//! constrain what that code does. A plugin that declares [`IsolationClass::Sandboxed`] and then
//! reads `/etc/shadow` will be recorded as sandboxed and will read `/etc/shadow`.
//!
//! So what is the point? Three things a declaration does buy, none of which is a security
//! boundary:
//!
//! 1. **Refusal before execution.** A host with an [`crate::EffectPolicy`] can decline to register
//!    a plugin whose declared needs exceed the grant. That is a check on the declaration, and it
//!    catches an honest plugin asking for too much. It does not catch a lying one.
//! 2. **An audit record.** 40.16 requires the sandbox and policy requirements to be part of the
//!    registration output. An operator reading the registry can see which plugin claimed to need
//!    the network.
//! 3. **A place for a real enforcer to read from.** If this workspace ever grows a process- or
//!    Wasm-based host, its supervisor needs a declaration to enforce *against*, and this is it.
//!
//! # The type says so too
//!
//! [`Enforcement`] has exactly one variant, [`Enforcement::DeclaredOnly`]. There is no
//! `Enforcement::Enforced` for a future refactor to reach for by accident, and no code path
//! anywhere can record that isolation was applied, because no such value exists. Adding one would
//! be a visible change to a public enum rather than a quiet change to a boolean.

use serde::{Deserialize, Serialize};
use std::fmt;

/// How a plugin says it wants to be run.
///
/// Ordered from least to most isolation so a host can compare a request against a floor
/// (`request >= policy.minimum`). The ordering is over the *requests*, not over any delivered
/// property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationClass {
    /// Linked into the host. 11.13's "trusted library plugins may load in process". Everything
    /// this workspace can actually run is this, whatever it declares.
    InProcess,
    /// 11.12's "restricted process" for pure parsers.
    RestrictedProcess,
    /// 11.12's sandbox for benchmark and evaluator code.
    Sandboxed,
}

impl fmt::Display for IsolationClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            IsolationClass::InProcess => "in-process",
            IsolationClass::RestrictedProcess => "restricted-process",
            IsolationClass::Sandboxed => "sandboxed",
        };
        f.write_str(text)
    }
}

/// Whether an isolation declaration was enforced. It was not.
///
/// One variant, on purpose. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enforcement {
    /// Recorded in the registry, applied by nothing.
    DeclaredOnly,
}

impl fmt::Display for Enforcement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("declared-only")
    }
}

/// Resource ceilings a plugin says it needs, in the units 11.12 asks for.
///
/// `None` means "unstated", which is different from "unlimited" and is why these are options.
/// Nothing reads them at run time; a supervisor that does not exist yet would.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceRequest {
    pub wall_clock_ms: Option<u64>,
    pub memory_mib: Option<u64>,
    pub max_output_bytes: Option<u64>,
}

impl ResourceRequest {
    pub fn unstated() -> Self {
        ResourceRequest::default()
    }

    pub fn is_unstated(&self) -> bool {
        *self == ResourceRequest::default()
    }

    pub fn with_wall_clock_ms(mut self, ms: u64) -> Self {
        self.wall_clock_ms = Some(ms);
        self
    }

    pub fn with_memory_mib(mut self, mib: u64) -> Self {
        self.memory_mib = Some(mib);
        self
    }

    /// The third ceiling 11.12 asks for. Present so a declaration can state all three; a struct
    /// with a field no builder sets would make "unstated" indistinguishable from "unstatable".
    pub fn with_max_output_bytes(mut self, bytes: u64) -> Self {
        self.max_output_bytes = Some(bytes);
        self
    }
}

/// The whole sandbox section of a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxDeclaration {
    pub isolation: IsolationClass,
    pub resources: ResourceRequest,
    /// Always [`Enforcement::DeclaredOnly`]. Serialised anyway, so that a reader of a stored
    /// registry does not have to know this crate's limitations to learn them.
    pub enforcement: Enforcement,
}

impl SandboxDeclaration {
    pub fn requesting(isolation: IsolationClass) -> Self {
        SandboxDeclaration {
            isolation,
            resources: ResourceRequest::unstated(),
            enforcement: Enforcement::DeclaredOnly,
        }
    }

    pub fn with_resources(mut self, resources: ResourceRequest) -> Self {
        self.resources = resources;
        self
    }

    /// The sentence a host should print next to any isolation claim it displays.
    pub fn honest_label(&self) -> String {
        format!(
            "requests {} isolation; recorded only — this process applies no isolation",
            self.isolation
        )
    }
}

impl Default for SandboxDeclaration {
    fn default() -> Self {
        SandboxDeclaration::requesting(IsolationClass::InProcess)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_value_can_claim_isolation_was_enforced() {
        let declaration = SandboxDeclaration::requesting(IsolationClass::Sandboxed);
        assert_eq!(declaration.enforcement, Enforcement::DeclaredOnly);
        assert!(declaration.honest_label().contains("applies no isolation"));
    }

    #[test]
    fn unstated_resource_limits_are_distinguishable_from_stated_ones() {
        assert!(ResourceRequest::unstated().is_unstated());
        assert!(!ResourceRequest::unstated()
            .with_memory_mib(512)
            .is_unstated());
        assert_eq!(ResourceRequest::unstated().wall_clock_ms, None);
    }

    /// Each of the three ceilings can be stated, and stating one leaves the other two unstated.
    ///
    /// A missing builder would not fail to compile anywhere — it would just make one ceiling
    /// permanently unsayable, so the field count and the builder count are asserted together.
    #[test]
    fn every_resource_ceiling_has_a_builder_and_setting_one_leaves_the_others_unstated() {
        let stated = ResourceRequest::unstated()
            .with_wall_clock_ms(30_000)
            .with_memory_mib(512)
            .with_max_output_bytes(1_048_576);
        assert_eq!(stated.wall_clock_ms, Some(30_000));
        assert_eq!(stated.memory_mib, Some(512));
        assert_eq!(stated.max_output_bytes, Some(1_048_576));

        let only_output = ResourceRequest::unstated().with_max_output_bytes(4_096);
        assert!(!only_output.is_unstated());
        assert_eq!(only_output.wall_clock_ms, None);
        assert_eq!(only_output.memory_mib, None);
    }

    #[test]
    fn isolation_classes_order_from_least_to_most_isolated() {
        assert!(IsolationClass::InProcess < IsolationClass::RestrictedProcess);
        assert!(IsolationClass::RestrictedProcess < IsolationClass::Sandboxed);
    }

    #[test]
    fn the_enforcement_field_survives_a_json_round_trip() {
        let declaration = SandboxDeclaration::requesting(IsolationClass::RestrictedProcess)
            .with_resources(ResourceRequest::unstated().with_wall_clock_ms(30_000));
        let json = serde_json::to_string(&declaration).expect("serialises");
        assert!(json.contains("declared_only"), "{json}");
        let back: SandboxDeclaration = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, declaration);
    }
}
