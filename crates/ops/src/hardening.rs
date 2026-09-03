//! The two hardening invariants of 40.39 that `crates/safety` does not already own.
//!
//! Implements blueprint 40.39 (Security Threat Model and Hardening Contract) **partially and
//! deliberately so**. Read the table before anything else.
//!
//! # There is one threat model in this workspace and it is not here
//!
//! `bioprism-safety` is section 13's adversarial half: 25 threats, scored 6 enforced, 15
//! declared-only, 4 unmitigated, with the finding that every one of the 6 is a threat against the
//! platform's own honesty rather than against an attacker. 40.39 restates section 13 in five
//! sentences and overlaps it almost completely. Building a second threat model here would produce
//! two registers that disagree within a release, so this module builds none:
//!
//! | 40.39 invariant | who holds it | what this module does |
//! |---|---|---|
//! | 1. Untrusted code runs isolated | `bioprism_safety::threat` — `Mitigation::DeclaredOnly`, and `Enforcer` has no variant naming a runtime enforcer | cites it |
//! | 2. Agent and evaluator execution are separated | `bioprism_safety::boundary::BoundaryModel` — the influence paths that exist, including one the blueprint does not mention | cites it |
//! | 3. No ambient credentials | **this module**, over [`crate::config`] | [`audit_credentials`] |
//! | 4. Network and filesystem effects are explicit | **this module** | [`EffectDeclaration::permits`] |
//! | 5. Supply-chain artifacts are verified | `bioprism_safety::supply::SignatureStatus` — one variant, `NotChecked` | cites it |
//!
//! [`coverage`] is that table as data, and a test holds it against `bioprism_safety`'s own coverage
//! so that this crate cannot raise safety's enforced count by writing a sentence here.
//!
//! # Why invariants 3 and 4 land in an operations crate at all
//!
//! Both are properties of a *configuration*, not of a runtime. "No ambient credentials" is the
//! statement that a secret is reachable only through a lease scoped to a named boundary, and
//! [`crate::config::SecretLease`] is where leases live. "Effects are explicit" is the statement
//! that a run declares what it will touch before it touches it, and a declaration is configuration.
//! Neither claim requires intercepting anything, which is exactly why they can be made honestly
//! here — and neither is worth anything unless something enforces the declaration, which nothing in
//! this workspace does.
//!
//! # What this module cannot do, stated plainly
//!
//! [`EffectDeclaration::permits`] is a **predicate over two values**. It does not hook a syscall,
//! wrap a socket, install a seccomp filter, resolve a symlink, canonicalise a path or observe
//! anything a process does. A caller that never asks it is unaffected by it. Its worth is that a
//! run's declared effects become a checkable object that a reviewer can read and a test can hold,
//! which is the same standing `bioprism_safety::Mitigation::DeclaredOnly` has and it is labelled
//! the same way.
//!
//! Specifically absent: no path canonicalisation, so `../` and symlinks defeat the prefix test; no
//! DNS resolution, so two names for one host read as two hosts; no port, protocol or method
//! granularity; no wildcards, because a wildcard host is a declaration of everything and the
//! failure of effect declarations everywhere is that they widen until they mean nothing.

use crate::config::{EffectiveConfig, SecretLease};
use crate::error::OpsError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Something a run does to the world outside its own memory.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum Effect {
    /// Reaching a named host. No port, no protocol, and no wildcard.
    Network { host: String },
    /// Touching a path. `write` is a distinct authority from reading it.
    Filesystem { path: String, write: bool },
    /// Starting a process from a named image.
    ProcessSpawn { image: String },
    /// Reading a clock. Declared like any other effect, because a run that reads one is not
    /// reproducible and the whole workspace advances on caller-supplied epochs instead.
    Clock,
    /// Drawing randomness.
    Randomness,
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Effect::Network { host } => write!(f, "network:{host}"),
            Effect::Filesystem { path, write } => {
                write!(f, "filesystem:{path}:{}", if *write { "rw" } else { "ro" })
            }
            Effect::ProcessSpawn { image } => write!(f, "process:{image}"),
            Effect::Clock => f.write_str("clock"),
            Effect::Randomness => f.write_str("randomness"),
        }
    }
}

impl Effect {
    /// Whether a declaration of `self` covers a request for `requested`.
    ///
    /// Exact for hosts and images. Prefix for paths, at a separator boundary so that a declaration
    /// of `/data` does not cover `/database`, and a read request is covered by a write declaration
    /// while the reverse is not.
    fn covers(&self, requested: &Effect) -> bool {
        match (self, requested) {
            (Effect::Network { host: declared }, Effect::Network { host: wanted }) => {
                declared == wanted
            }
            (Effect::ProcessSpawn { image: declared }, Effect::ProcessSpawn { image: wanted }) => {
                declared == wanted
            }
            (
                Effect::Filesystem {
                    path: declared,
                    write: declared_write,
                },
                Effect::Filesystem {
                    path: wanted,
                    write: wanted_write,
                },
            ) => {
                if *wanted_write && !*declared_write {
                    return false;
                }
                path_covers(declared, wanted)
            }
            (Effect::Clock, Effect::Clock) => true,
            (Effect::Randomness, Effect::Randomness) => true,
            _ => false,
        }
    }
}

fn path_covers(declared: &str, wanted: &str) -> bool {
    if declared == wanted {
        return true;
    }
    let boundary = if declared.ends_with('/') {
        declared.to_string()
    } else {
        format!("{declared}/")
    };
    wanted.starts_with(&boundary)
}

/// What a run says it will touch, before it touches it.
///
/// Deny by default. An empty declaration permits nothing, which is the only defensible starting
/// point: a run that forgot to declare should fail, not proceed.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EffectDeclaration {
    run: String,
    declared: Vec<Effect>,
}

impl EffectDeclaration {
    pub fn new(run: impl Into<String>) -> Self {
        EffectDeclaration {
            run: run.into(),
            declared: Vec::new(),
        }
    }

    pub fn declaring(mut self, effect: Effect) -> Self {
        self.declared.push(effect);
        self
    }

    pub fn run(&self) -> &str {
        &self.run
    }

    pub fn declared(&self) -> &[Effect] {
        &self.declared
    }

    /// Whether the declaration covers a requested effect.
    pub fn permits(&self, requested: &Effect) -> Result<(), OpsError> {
        if self
            .declared
            .iter()
            .any(|declared| declared.covers(requested))
        {
            return Ok(());
        }
        Err(OpsError::UndeclaredEffect {
            run: self.run.clone(),
            effect: requested.to_string(),
        })
    }
}

/// One secret reachable without a lease.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmbientFinding {
    pub reference: String,
    /// Why the reference is ambient, in words a reviewer can act on.
    pub reason: String,
}

/// The result of holding a configuration against 40.39's third invariant.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CredentialAudit {
    findings: Vec<AmbientFinding>,
    leased: Vec<String>,
}

impl CredentialAudit {
    pub fn findings(&self) -> &[AmbientFinding] {
        &self.findings
    }

    /// References that are scoped to a named boundary, in reference order.
    pub fn leased(&self) -> &[String] {
        &self.leased
    }

    pub fn is_clean(&self) -> bool {
        self.findings.is_empty()
    }

    /// Turns the first finding into a typed failure, for a caller that wants to stop.
    ///
    /// Reports rather than fails by default, because a report names every ambient credential and an
    /// error names one; the error exists so a gate can be built on it.
    pub fn assert_none(&self) -> Result<(), OpsError> {
        match self.findings.first() {
            None => Ok(()),
            Some(finding) => Err(OpsError::AmbientCredential {
                reference: finding.reference.clone(),
            }),
        }
    }
}

/// Holds an effective configuration against *no ambient credentials*.
///
/// A secret whose source is process-wide — the environment — is readable by every line of code in
/// the process, so it is ambient unless a lease scopes it to a named execution boundary. A secret
/// from a vault or a file is not ambient by itself: something has to go and fetch it, and the fetch
/// is where the boundary sits.
///
/// A lease with a blank boundary does not scope anything and is reported as if there were no lease
/// at all.
pub fn audit_credentials(config: &EffectiveConfig, leases: &[SecretLease]) -> CredentialAudit {
    let mut findings = Vec::new();
    let mut leased = Vec::new();

    for reference in config.secret_references() {
        let scoped = leases
            .iter()
            .any(|lease| lease.reference() == reference && !lease.boundary().trim().is_empty());
        if scoped {
            leased.push(reference.to_string());
            continue;
        }
        if reference.source().is_process_wide() {
            findings.push(AmbientFinding {
                reference: reference.to_string(),
                reason: "process environment is readable by the whole process and no lease names \
                         an execution boundary for it"
                    .to_string(),
            });
        }
    }

    CredentialAudit { findings, leased }
}

/// Who holds one of 40.39's five invariants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum ControlOwner {
    /// Modelled in `bioprism-safety`, which states in its own documentation that it enforces
    /// nothing and cannot.
    Safety { item: &'static str },
    /// A predicate in this crate. Still not a runtime control; see the module docs.
    Ops { item: &'static str },
}

impl ControlOwner {
    pub fn item(&self) -> &'static str {
        match self {
            ControlOwner::Safety { item } | ControlOwner::Ops { item } => item,
        }
    }
}

/// One row of the coverage table in the module docs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlCoverage {
    pub invariant: &'static str,
    pub owner: ControlOwner,
    /// Whether a predicate in *this crate* decides it. Never a claim that a runtime prevents it.
    pub decided_here: bool,
    pub note: &'static str,
}

/// 40.39's five invariants and who holds each.
pub fn coverage() -> Vec<ControlCoverage> {
    vec![
        ControlCoverage {
            invariant: "untrusted code runs isolated",
            owner: ControlOwner::Safety {
                item: "bioprism_safety::threat::Mitigation::DeclaredOnly",
            },
            decided_here: false,
            note: "no process, container, microVM or syscall filter exists anywhere in this \
                   workspace; safety records the control as declared and names no enforcer",
        },
        ControlCoverage {
            invariant: "agent and evaluator execution are separated",
            owner: ControlOwner::Safety {
                item: "bioprism_safety::boundary::BoundaryModel::influence_paths",
            },
            decided_here: false,
            note: "safety models the paths that exist, including one the blueprint does not \
                   mention; nothing here re-derives them",
        },
        ControlCoverage {
            invariant: "no ambient credentials",
            owner: ControlOwner::Ops {
                item: "bioprism_ops::hardening::audit_credentials",
            },
            decided_here: true,
            note: "a secret from the process environment with no lease naming an execution \
                   boundary is reported; nothing revokes it",
        },
        ControlCoverage {
            invariant: "network and filesystem effects are explicit",
            owner: ControlOwner::Ops {
                item: "bioprism_ops::hardening::EffectDeclaration::permits",
            },
            decided_here: true,
            note: "a predicate over a declaration and a request; no syscall is intercepted and an \
                   undeclared effect taken without asking is invisible",
        },
        ControlCoverage {
            invariant: "supply-chain artifacts are verified",
            owner: ControlOwner::Safety {
                item: "bioprism_safety::supply::SignatureStatus",
            },
            decided_here: false,
            note: "one variant, NotChecked; there is no key material in this workspace and a \
                   Verified variant would be set by code that does not exist",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Binding, ConfigStack, Layer, Schema, SecretRef, SecretSource, SettingKey, SettingSpec,
        Source,
    };
    use bioprism_infra::Epoch;

    fn key(name: &str) -> SettingKey {
        SettingKey::parse(name).expect("well-formed")
    }

    fn config_with(source: SecretSource) -> EffectiveConfig {
        let schema = Schema::new()
            .with(SettingSpec::secret(key("hub.token")))
            .unwrap();
        ConfigStack::new(schema)
            .push(Source::new(Layer::Environment, "declared").bind(
                key("hub.token"),
                Binding::Secret(SecretRef::new(source, "hub-token").unwrap()),
            ))
            .resolve()
            .expect("resolves")
    }

    #[test]
    fn a_secret_from_the_process_environment_with_no_lease_is_ambient() {
        let config = config_with(SecretSource::Environment);
        let audit = audit_credentials(&config, &[]);
        assert!(!audit.is_clean());
        assert!(matches!(
            audit.assert_none().unwrap_err(),
            OpsError::AmbientCredential { .. }
        ));
    }

    #[test]
    fn a_lease_naming_an_execution_boundary_stops_a_secret_being_ambient() {
        let config = config_with(SecretSource::Environment);
        let lease = config
            .lease(&key("hub.token"), "publish-boundary", Epoch::new(1), 3)
            .unwrap();
        let audit = audit_credentials(&config, std::slice::from_ref(&lease));
        assert!(audit.is_clean());
        assert_eq!(audit.leased(), ["env:hub-token".to_string()]);
    }

    #[test]
    fn a_lease_with_a_blank_boundary_scopes_nothing_and_leaves_the_secret_ambient() {
        let config = config_with(SecretSource::Environment);
        let lease = config
            .lease(&key("hub.token"), "   ", Epoch::new(1), 3)
            .unwrap();
        let audit = audit_credentials(&config, std::slice::from_ref(&lease));
        assert!(!audit.is_clean());
    }

    #[test]
    fn a_vault_reference_is_not_ambient_because_something_has_to_go_and_fetch_it() {
        let config = config_with(SecretSource::Vault);
        assert!(audit_credentials(&config, &[]).is_clean());
    }

    #[test]
    fn an_empty_effect_declaration_permits_nothing() {
        let declaration = EffectDeclaration::new("compile");
        let error = declaration
            .permits(&Effect::Network {
                host: "hub.example".into(),
            })
            .unwrap_err();
        assert!(matches!(error, OpsError::UndeclaredEffect { .. }));
        assert!(declaration.permits(&Effect::Clock).is_err());
    }

    #[test]
    fn a_declared_host_covers_itself_and_no_other() {
        let declaration = EffectDeclaration::new("publish").declaring(Effect::Network {
            host: "hub.example".into(),
        });
        assert!(declaration
            .permits(&Effect::Network {
                host: "hub.example".into()
            })
            .is_ok());
        assert!(declaration
            .permits(&Effect::Network {
                host: "hub.example.evil".into()
            })
            .is_err());
    }

    #[test]
    fn a_declared_path_covers_a_child_and_not_a_sibling_sharing_its_prefix() {
        let declaration = EffectDeclaration::new("ingest").declaring(Effect::Filesystem {
            path: "/data".into(),
            write: false,
        });
        assert!(declaration
            .permits(&Effect::Filesystem {
                path: "/data/cohort.parquet".into(),
                write: false
            })
            .is_ok());
        assert!(declaration
            .permits(&Effect::Filesystem {
                path: "/database/secrets".into(),
                write: false
            })
            .is_err());
    }

    #[test]
    fn a_read_declaration_does_not_cover_a_write_and_a_write_declaration_covers_a_read() {
        let read_only = EffectDeclaration::new("ingest").declaring(Effect::Filesystem {
            path: "/data".into(),
            write: false,
        });
        assert!(read_only
            .permits(&Effect::Filesystem {
                path: "/data/x".into(),
                write: true
            })
            .is_err());

        let writable = EffectDeclaration::new("ingest").declaring(Effect::Filesystem {
            path: "/data".into(),
            write: true,
        });
        assert!(writable
            .permits(&Effect::Filesystem {
                path: "/data/x".into(),
                write: false
            })
            .is_ok());
    }

    #[test]
    fn reading_a_clock_is_an_effect_that_has_to_be_declared_like_any_other() {
        let declaration = EffectDeclaration::new("replay").declaring(Effect::Clock);
        assert!(declaration.permits(&Effect::Clock).is_ok());
        assert!(declaration.permits(&Effect::Randomness).is_err());
    }

    #[test]
    fn this_crate_decides_two_of_the_five_hardening_invariants_and_claims_no_more() {
        let table = coverage();
        assert_eq!(table.len(), 5);
        let decided: Vec<&str> = table
            .iter()
            .filter(|row| row.decided_here)
            .map(|row| row.invariant)
            .collect();
        assert_eq!(
            decided,
            [
                "no ambient credentials",
                "network and filesystem effects are explicit"
            ]
        );
        for row in table.iter().filter(|row| !row.decided_here) {
            assert!(
                matches!(row.owner, ControlOwner::Safety { .. }),
                "{} is not decided here and must name the crate that models it",
                row.invariant
            );
        }
    }

    #[test]
    fn nothing_in_this_crate_raises_the_enforced_count_of_the_shipped_threat_model() {
        let coverage = bioprism_safety::model::section_13().coverage();
        assert_eq!(coverage.total(), 25);
        assert_eq!(
            coverage.mitigated, 6,
            "safety's honest count is 6 enforced; a hardening module that moved it would be \
             claiming a control by writing a sentence"
        );
        assert_eq!(coverage.declared_only, 15);
        assert_eq!(coverage.unmitigated, 4);
    }
}
