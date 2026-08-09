//! The boundary between the suite and the thing being certified.
//!
//! Blueprint 40.32 certifies an "implementation/plugin endpoint" against "stable observable
//! contracts". Everything in this module exists to keep that word *observable* honest: the trait
//! takes two JSON documents and returns named JSON documents, and the runner may look at nothing
//! else. No typed handles, no trait objects from `bioprism-fiber`, no access to a compile trace
//! as a Rust value.
//!
//! That constraint is what lets [`crate::fiber_suite()`] be published as data and executed against
//! a CPython or TypeScript compiler. It also costs something real: an expectation can only test
//! what an implementation chooses to publish. An implementation that publishes no compiler report
//! cannot be tested for declaring its deferred passes, and the runner reports that requirement as
//! [`crate::CaseOutcome::Unsupported`] rather than as a pass or a failure — 40.32's "partial
//! unsupported requirement", kept distinct from both.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Well-known artifact names.
///
/// A suite may use any name; these are the ones the FIBER suite expects, fixed here so that an
/// independent implementation knows what to publish under what key.
pub mod artifact {
    /// The `fiber-decision-section/0.1` document (43.25).
    pub const SECTION: &str = "decision_section";
    /// The `fiber-context-certificate/0.1` document (43.26).
    pub const CERTIFICATE: &str = "context_certificate";
    /// The `fiber-context-certificate/0.2-extended` document, carrying the influence-classified
    /// omission manifest.
    pub const CERTIFICATE_EXTENDED: &str = "context_certificate_extended";
    /// The compiler's own report: which passes ran, which were deferred, whether the protected
    /// closure survived. Optional, and its absence is reported as unsupported rather than failed.
    pub const REPORT: &str = "compiler_report";
}

/// Documents an implementation publishes for one compilation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CompileArtifacts {
    pub artifacts: BTreeMap<String, Value>,
}

impl CompileArtifacts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with(mut self, name: impl Into<String>, document: Value) -> Self {
        self.artifacts.insert(name.into(), document);
        self
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.artifacts.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.artifacts.keys().map(String::as_str).collect()
    }
}

/// A refusal to compile, reduced to its typed discriminant.
///
/// `kind` is the contract; `message` is for humans and is never asserted on. An implementation in
/// another language cannot reproduce this crate's error strings, but it can and must reproduce
/// *which class* of refusal a given malformed input provokes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompileFailure {
    pub kind: String,
    pub message: String,
}

impl CompileFailure {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        CompileFailure {
            kind: kind.into(),
            message: message.into(),
        }
    }
}

/// Identity of the thing under test, bound into the certificate by 40.32 invariant 4.
///
/// `digest` is optional and, for an in-process Rust implementation, unavailable: this crate
/// cannot hash its own compiled artifact. A certificate issued without a digest binds a *name and
/// version*, which is weaker, and [`ImplementationIdentity::is_digest_bound`] says so rather than
/// letting a reader assume otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationIdentity {
    pub name: String,
    pub version: String,
    pub language: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
}

impl ImplementationIdentity {
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        language: impl Into<String>,
    ) -> Self {
        ImplementationIdentity {
            name: name.into(),
            version: version.into(),
            language: language.into(),
            digest: None,
        }
    }

    pub fn with_digest(mut self, digest: impl Into<String>) -> Self {
        self.digest = Some(digest.into());
        self
    }

    pub fn is_digest_bound(&self) -> bool {
        self.digest.is_some()
    }
}

/// The environment a run happened in (40.32 output 3).
///
/// Records what a `no_std`-adjacent, dependency-free crate can actually observe. It does **not**
/// record the toolchain version, the dependency lock digest, the container image, or any
/// attestation that the machine was clean — all of which 40.32 asks for and none of which are
/// available without a build script or a process launch. Treat a bundle carrying this manifest as
/// evidence about the platform, not as a clean-environment attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvironmentManifest {
    pub os: String,
    pub arch: String,
    pub family: String,
    pub pointer_width: u32,
    pub endian: String,
    pub runner: String,
    pub runner_version: String,
    /// Facets of the environment this manifest deliberately does not attest.
    pub unattested: Vec<String>,
}

impl EnvironmentManifest {
    pub fn detect() -> Self {
        EnvironmentManifest {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            family: std::env::consts::FAMILY.to_string(),
            pointer_width: usize::BITS,
            endian: if cfg!(target_endian = "big") {
                "big".to_string()
            } else {
                "little".to_string()
            },
            runner: env!("CARGO_PKG_NAME").to_string(),
            runner_version: env!("CARGO_PKG_VERSION").to_string(),
            unattested: [
                "toolchain_version",
                "dependency_lock_digest",
                "container_image",
                "clean_machine_provisioning",
                "network_isolation",
            ]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        }
    }
}

/// What the suite requires of a compiler to certify it.
pub trait Implementation {
    fn identity(&self) -> ImplementationIdentity;

    /// Compile a world and a query into published documents, or refuse with a typed kind.
    fn compile(&self, world: &Value, query: &Value) -> Result<CompileArtifacts, CompileFailure>;
}
