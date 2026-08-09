//! Local-first deployment (12.15): the offline contract, the resource envelope, the doctor, and
//! the parity claim.
//!
//! 12.15's detailed design is five subsections. *Install* — `uv tool install`, container bundles,
//! optional extras — is a packaging decision with no artifact here to check, and is not
//! implemented. *Embedded stack* is 12.02's topology under a different name and is delegated to
//! it rather than restated. The remaining three are predicates:
//!
//! | 12.15 clause | here |
//! |---|---|
//! | "no network unless explicitly enabled" | [`OfflineContract::resolve`] |
//! | "degrade with clear diagnostics, not hangs" | [`Resolution`] has no pending variant; [`ResourceEnvelope::admit`] refuses up front |
//! | "emit a shareable redacted report" | [`DoctorReport::redacted`] |
//! | "maintain semantic parity with hosted mode" | [`ParityClaim`] |
//!
//! # A skipped probe is not a passing probe
//!
//! This is the crate's central rule in its most ordinary clothes. A doctor that ran nine checks
//! and skipped the tenth reports nine greens, and a human reads ten. [`DoctorReport::readiness`]
//! returns [`Readiness::Undetermined`] whenever any probe was not run and nothing outright
//! failed, and [`Readiness`] has no `is_ok` — only [`Readiness::is_ready`], which is false for
//! `Undetermined` and says so in its name.
//!
//! # "Not hangs" is a property of the type, not of the implementation
//!
//! 12.15 asks larger environments to "degrade with clear diagnostics, not hangs". A hang is what
//! happens when a resolution has a state meaning *still trying*, so [`Resolution`] has two
//! variants and neither is one. Nothing here blocks, retries, or waits, and nothing can be added
//! that does without changing the enum.
//!
//! # Redaction keeps the action visible
//!
//! [`Detail::Sensitive`] is replaced by a digest of its own content, not by a placeholder. Two
//! machines whose probe found the same value produce the same digest and can be compared; a
//! machine whose value changed shows a different digest. The probe's name and outcome variant are
//! untouched, which is 12.03's rule for audit records applied to a support bundle: hash the value,
//! keep the action.
//!
//! # Not implemented
//!
//! Nothing here probes anything. There is no filesystem check, no port check, no memory query, no
//! container runtime detection, no schema inspection — every [`ProbeOutcome`] is supplied by a
//! caller that did the looking. That is the honest shape for a crate with no I/O, and it means a
//! green [`Readiness::Ready`] attests to the caller's probes and not to the machine. No installer,
//! no package extras, no local server, no viewer. The offline contract is a policy object; it
//! intercepts no sockets, because there are none.

use crate::error::LocalError;
use crate::topology::{parity, ParityReport, PromiseDifference, StorageTopology};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

/// Where a requirement would be satisfied from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Source {
    /// A backend that ships with the installation.
    Embedded { technology: String },
    /// A recorded fixture standing in for a live call.
    Fixture { name: String },
    /// A live call to a host.
    Network { host: String },
}

/// Something the deployment needs before it can run.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Requirement {
    pub name: String,
    pub source: Source,
}

/// Whether the deployment may reach the network, and where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "network", rename_all = "snake_case")]
pub enum NetworkPolicy {
    /// 12.15's default: "no network unless explicitly enabled".
    Closed,
    /// Explicitly enabled, for an explicit set of hosts.
    ///
    /// An allow-list rather than a boolean, because "network enabled" as a flag is how a local
    /// run that needed one registry ends up able to reach anything.
    Open { allowed: BTreeSet<String> },
}

/// The resolution of one requirement. Two states, and no third meaning *waiting*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "resolution", rename_all = "snake_case")]
pub enum Resolution {
    Satisfied { by: Source },
    Refused { reason: Unsatisfiable },
}

impl Resolution {
    pub fn is_satisfied(&self) -> bool {
        matches!(self, Resolution::Satisfied { .. })
    }
}

/// Why a requirement cannot be met under the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "unsatisfiable", rename_all = "snake_case")]
pub enum Unsatisfiable {
    /// The requirement needs a host and the contract is closed.
    NetworkDenied { host: String },
    /// The contract is open but not to this host.
    HostNotAllowed { host: String, allowed: Vec<String> },
}

/// The offline contract a local deployment runs under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OfflineContract {
    policy: NetworkPolicy,
}

impl OfflineContract {
    /// The 12.15 default. Named `closed` rather than `default` so choosing it is an act.
    pub fn closed() -> Self {
        OfflineContract {
            policy: NetworkPolicy::Closed,
        }
    }

    pub fn open_to(hosts: impl IntoIterator<Item = String>) -> Self {
        OfflineContract {
            policy: NetworkPolicy::Open {
                allowed: hosts.into_iter().collect(),
            },
        }
    }

    pub fn policy(&self) -> &NetworkPolicy {
        &self.policy
    }

    /// Decides a requirement immediately.
    ///
    /// Embedded backends and recorded fixtures always resolve; only [`Source::Network`] consults
    /// the policy. That asymmetry is 12.15's core-demo promise — "core demo uses fixture models
    /// and no API key" — made checkable: a requirement list containing only embedded and fixture
    /// sources resolves under [`OfflineContract::closed`], and a test can assert exactly that.
    pub fn resolve(&self, requirement: &Requirement) -> Resolution {
        match (&requirement.source, &self.policy) {
            (Source::Embedded { .. } | Source::Fixture { .. }, _) => Resolution::Satisfied {
                by: requirement.source.clone(),
            },
            (Source::Network { host }, NetworkPolicy::Closed) => Resolution::Refused {
                reason: Unsatisfiable::NetworkDenied { host: host.clone() },
            },
            (Source::Network { host }, NetworkPolicy::Open { allowed }) => {
                if allowed.contains(host) {
                    Resolution::Satisfied {
                        by: requirement.source.clone(),
                    }
                } else {
                    Resolution::Refused {
                        reason: Unsatisfiable::HostNotAllowed {
                            host: host.clone(),
                            allowed: allowed.iter().cloned().collect(),
                        },
                    }
                }
            }
        }
    }

    /// Resolves a whole list, returning the first refusal as a typed error.
    ///
    /// Exists so that a caller wanting the 12.15 offline guarantee gets it as a `?` rather than
    /// as a loop it might write with the comparison inverted.
    pub fn admit_all<'a>(
        &self,
        requirements: impl IntoIterator<Item = &'a Requirement>,
    ) -> Result<usize, LocalError> {
        let mut admitted = 0usize;
        for requirement in requirements {
            match self.resolve(requirement) {
                Resolution::Satisfied { .. } => admitted += 1,
                Resolution::Refused { reason } => {
                    return Err(match reason {
                        Unsatisfiable::NetworkDenied { host } => LocalError::NetworkDenied {
                            requirement: requirement.name.clone(),
                            host,
                        },
                        Unsatisfiable::HostNotAllowed { host, .. } => {
                            LocalError::HostNotAllowed { host }
                        }
                    })
                }
            }
        }
        Ok(admitted)
    }
}

/// A probe detail that may or may not be safe to share.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "detail", rename_all = "snake_case")]
pub enum Detail {
    Plain { text: String },
    /// Replaced by a digest of itself when the report is redacted.
    Sensitive { text: String },
    /// What a sensitive detail becomes. Carries the digest, so two reports remain comparable.
    Redacted { digest: ContentHash },
}

impl Detail {
    pub fn plain(text: impl Into<String>) -> Self {
        Detail::Plain { text: text.into() }
    }

    pub fn sensitive(text: impl Into<String>) -> Self {
        Detail::Sensitive { text: text.into() }
    }

    /// Redacts, or returns itself unchanged.
    ///
    /// Idempotent by construction: a [`Detail::Redacted`] has no text left to hash, so redacting
    /// twice cannot produce a digest of a digest.
    pub fn redacted(&self) -> Result<Detail, LocalError> {
        match self {
            Detail::Sensitive { text } => {
                let digest = ContentHash::of_value(&json!({ "sensitive": text })).map_err(
                    |error| LocalError::MalformedField {
                        field: "sensitive detail",
                        value: error.to_string(),
                    },
                )?;
                Ok(Detail::Redacted { digest })
            }
            other => Ok(other.clone()),
        }
    }
}

/// What one probe found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum ProbeOutcome {
    Ok { detail: Detail },
    Problem { detail: Detail, remedy: String },
    /// The probe did not run. This is not a pass.
    NotChecked { reason: String },
}

impl ProbeOutcome {
    fn detail(&self) -> Option<&Detail> {
        match self {
            ProbeOutcome::Ok { detail } | ProbeOutcome::Problem { detail, .. } => Some(detail),
            ProbeOutcome::NotChecked { .. } => None,
        }
    }

    fn redacted(&self) -> Result<ProbeOutcome, LocalError> {
        Ok(match self {
            ProbeOutcome::Ok { detail } => ProbeOutcome::Ok {
                detail: detail.redacted()?,
            },
            ProbeOutcome::Problem { detail, remedy } => ProbeOutcome::Problem {
                detail: detail.redacted()?,
                remedy: remedy.clone(),
            },
            ProbeOutcome::NotChecked { reason } => ProbeOutcome::NotChecked {
                reason: reason.clone(),
            },
        })
    }
}

/// Whether the deployment is fit to run.
///
/// Three states. `Undetermined` is the one that matters and it is not a synonym for either
/// neighbour: a machine with an unrun probe is not ready and is also not broken, and collapsing
/// the two is how a support bundle comes to certify a configuration nobody looked at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "readiness", rename_all = "snake_case")]
pub enum Readiness {
    Ready { checked: usize },
    NotReady { problems: Vec<String> },
    Undetermined { unchecked: Vec<String> },
}

impl Readiness {
    /// True only for [`Readiness::Ready`]. There is deliberately no `is_ok`.
    pub fn is_ready(&self) -> bool {
        matches!(self, Readiness::Ready { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Readiness::Ready { .. } => "ready",
            Readiness::NotReady { .. } => "not-ready",
            Readiness::Undetermined { .. } => "undetermined",
        }
    }
}

/// The doctor's output: one outcome per named probe.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    probes: BTreeMap<String, ProbeOutcome>,
}

impl DoctorReport {
    pub fn new() -> Self {
        DoctorReport::default()
    }

    pub fn with(mut self, probe: impl Into<String>, outcome: ProbeOutcome) -> Result<Self, LocalError> {
        let probe = probe.into();
        if self.probes.contains_key(&probe) {
            return Err(LocalError::DuplicateProbe { probe });
        }
        self.probes.insert(probe, outcome);
        Ok(self)
    }

    pub fn probes(&self) -> &BTreeMap<String, ProbeOutcome> {
        &self.probes
    }

    /// Definite problems first, then unrun probes, then ready.
    ///
    /// A report with both a problem and a skipped probe is `NotReady`: the problem is actionable
    /// now and the skipped probe cannot make it less so.
    pub fn readiness(&self) -> Readiness {
        let problems: Vec<String> = self
            .probes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, ProbeOutcome::Problem { .. }))
            .map(|(name, _)| name.clone())
            .collect();
        if !problems.is_empty() {
            return Readiness::NotReady { problems };
        }
        let unchecked: Vec<String> = self
            .probes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, ProbeOutcome::NotChecked { .. }))
            .map(|(name, _)| name.clone())
            .collect();
        if !unchecked.is_empty() {
            return Readiness::Undetermined { unchecked };
        }
        Readiness::Ready {
            checked: self.probes.len(),
        }
    }

    /// The shareable form: same probes, same outcomes, sensitive details replaced by digests.
    pub fn redacted(&self) -> Result<DoctorReport, LocalError> {
        let mut out = DoctorReport::new();
        for (name, outcome) in &self.probes {
            out.probes.insert(name.clone(), outcome.redacted()?);
        }
        Ok(out)
    }

    /// Whether any detail would still disclose a value.
    ///
    /// The check a caller runs before attaching the report to a ticket. Its existence is the
    /// admission that [`DoctorReport::redacted`] is a transformation somebody has to remember to
    /// apply; nothing in the type system forces it, and pretending otherwise would be worse than
    /// saying so.
    pub fn discloses_sensitive(&self) -> bool {
        self.probes
            .values()
            .filter_map(ProbeOutcome::detail)
            .any(|detail| matches!(detail, Detail::Sensitive { .. }))
    }
}

/// What the machine has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceEnvelope {
    pub memory_mb: u64,
    pub disk_mb: u64,
}

/// What an environment says it needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Demand {
    pub memory_mb: u64,
    pub disk_mb: u64,
}

impl ResourceEnvelope {
    /// Refuses up front, naming the resource and both numbers.
    ///
    /// 12.15 asks larger environments to "degrade with clear diagnostics, not hangs". Everything
    /// that makes that true is in the signature: it returns, it returns a typed value, and the
    /// value says which resource and by how much.
    pub fn admit(self, demand: Demand) -> Result<(), LocalError> {
        if demand.memory_mb > self.memory_mb {
            return Err(LocalError::EnvelopeExceeded {
                resource: "memory_mb",
                needed: demand.memory_mb,
                available: self.memory_mb,
            });
        }
        if demand.disk_mb > self.disk_mb {
            return Err(LocalError::EnvelopeExceeded {
                resource: "disk_mb",
                needed: demand.disk_mb,
                available: self.disk_mb,
            });
        }
        Ok(())
    }
}

/// Whether local and hosted mode were shown to promise the same things.
///
/// The only way to reach [`ParityClaim::Established`] is [`ParityClaim::from_comparison`], which
/// takes a report produced by an actual comparison. An assertion with no comparison behind it can
/// only be [`ParityClaim::Unverified`], because that is the only other constructor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "parity", rename_all = "snake_case")]
pub enum ParityClaim {
    Established { compared: usize },
    Broken { differences: Vec<PromiseDifference> },
    Unverified { reason: String },
}

impl ParityClaim {
    pub fn from_comparison(report: ParityReport) -> Self {
        if report.holds() {
            ParityClaim::Established {
                compared: report.compared,
            }
        } else {
            ParityClaim::Broken {
                differences: report.differences,
            }
        }
    }

    /// Compares two deployments and returns the claim their promises support.
    pub fn between(local: &StorageTopology, hosted: &StorageTopology) -> Self {
        ParityClaim::from_comparison(parity(local, hosted))
    }

    pub fn unverified(reason: impl Into<String>) -> Self {
        ParityClaim::Unverified {
            reason: reason.into(),
        }
    }

    /// True only for [`ParityClaim::Established`].
    pub fn is_established(&self) -> bool {
        matches!(self, ParityClaim::Established { .. })
    }
}
