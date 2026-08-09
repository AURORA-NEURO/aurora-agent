//! Compute providers and Kubernetes (12.13): what a provider publishes, and what publishing is
//! worth.
//!
//! This is the module where the classification splits inside the file. 12.13's detailed design
//! has five subsections. *Kubernetes roles* — control services, worker deployments, artifact
//! gateway, network policies, autoscaling, node pools — is a description of a deployment somebody
//! operates; there is no artifact here to hold a predicate over, and none of it is implemented.
//! The other four are predicates and are:
//!
//! | 12.13 clause | here |
//! |---|---|
//! | "each publishes cost, region, capabilities, and conformance" | [`ProviderProfile`], with conformance as an [`Attested`] value |
//! | "do not assume ordinary Kubernetes namespaces alone isolate hostile agent code" | [`IsolationStrength::adequate_for`] |
//! | "warm pools are limited to compatible trust domains" | [`WarmPool::admits`] |
//! | "the reference local path works without Kubernetes, Redis, Kafka, or cloud object storage" | [`local_path_is_self_contained`] |
//!
//! # Publishing is a declaration
//!
//! "Each publishes cost, region, capabilities, and conformance" is one verb doing two jobs. Cost
//! and region are commercial facts a provider is entitled to state. Conformance is a test result,
//! and a provider stating its own test result is not the same object as a suite the platform ran.
//! [`ProviderProfile`] therefore carries conformance as an [`Attested<ConformanceLevel>`]: a
//! provider's claim is [`Basis::Declared`](crate::basis::Basis::Declared), a platform-run suite is
//! [`Basis::FirstHand`](crate::basis::Basis::FirstHand), and the two are not equal even when the
//! level is identical. [`AdmissionPolicy::require_verified_conformance`] is the switch that makes
//! the difference bite.
//!
//! # The isolation ladder is ordered, and the mapping is mine
//!
//! 12.13 lists "hardened runtimes, separate nodes, microVMs, or external sandboxes" as an
//! unordered set and says to choose "according to threat level" without giving the mapping. The
//! ordering in [`IsolationStrength`] and the floors in [`ThreatLevel::minimum_isolation`] are a
//! reading, stated here so a disagreement lands on one function. What is *not* a reading is that
//! [`IsolationStrength::SharedNamespace`] is inadequate for anything but trusted work; the
//! section says so in as many words.
//!
//! # Not implemented
//!
//! No container runtime, no Kubernetes client, no manifests, no autoscaler, no network policy, no
//! artifact gateway, no credentials of any kind. Nothing here starts, stops or observes a
//! process. `bioprism-safety` states this workspace's position and it applies exactly:
//! a library of plain Rust types may *model* an isolation boundary and may not *claim* one, so
//! [`IsolationStrength`] is a declared property that this crate compares and never verifies.
//! Cost is an opaque integer in caller-defined units — 12.13 says providers publish cost and says
//! nothing about a unit, and inventing a currency would be inventing spec.

use crate::basis::{Attested, PartyId};
use crate::error::{check_name, ProviderError};
use crate::topology::StorageTopology;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

macro_rules! provider_id {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, ProviderError> {
                let value = value.into();
                if !check_name(&value) {
                    return Err(ProviderError::MalformedField {
                        field: $field,
                        value,
                    });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = ProviderError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

provider_id!(ProviderId, "provider id");
provider_id!(Region, "region");
provider_id!(Capability, "capability");
provider_id!(TrustDomain, "trust domain");

/// The three provider families 12.13 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// Docker or Podman on the operator's own machine.
    LocalContainer,
    /// A cluster the operator runs.
    SelfHostedKubernetes,
    /// Somebody else's sandbox or compute service.
    ExternalSandbox,
}

impl ProviderKind {
    pub fn name(self) -> &'static str {
        match self {
            ProviderKind::LocalContainer => "local-container",
            ProviderKind::SelfHostedKubernetes => "self-hosted-kubernetes",
            ProviderKind::ExternalSandbox => "external-sandbox",
        }
    }
}

/// How strongly a provider separates a workload from everything else, weakest first.
///
/// The `Ord` derive is load-bearing: adequacy is a `>=` against a floor, so inserting a variant in
/// the wrong position silently weakens every isolation check in the crate. Variants are ordered
/// by the boundary they interpose, not by cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationStrength {
    /// A namespace on a shared node. 12.13 says explicitly not to treat this as isolation.
    SharedNamespace,
    /// A hardened container runtime on a shared node.
    HardenedRuntime,
    /// A node reserved for one trust domain.
    DedicatedNode,
    /// A virtual machine per workload.
    MicroVm,
    /// A separately operated sandbox service.
    ExternalSandbox,
}

impl IsolationStrength {
    pub fn name(self) -> &'static str {
        match self {
            IsolationStrength::SharedNamespace => "shared-namespace",
            IsolationStrength::HardenedRuntime => "hardened-runtime",
            IsolationStrength::DedicatedNode => "dedicated-node",
            IsolationStrength::MicroVm => "micro-vm",
            IsolationStrength::ExternalSandbox => "external-sandbox",
        }
    }

    pub fn adequate_for(self, threat: ThreatLevel) -> bool {
        self >= threat.minimum_isolation()
    }
}

/// How much the platform trusts the code it is about to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatLevel {
    /// Platform code and first-party fixtures.
    Trusted,
    /// Third-party benchmark code with no reason to suspect it.
    Untrusted,
    /// Code assumed to be actively trying to escape.
    Hostile,
}

impl ThreatLevel {
    pub fn name(self) -> &'static str {
        match self {
            ThreatLevel::Trusted => "trusted",
            ThreatLevel::Untrusted => "untrusted",
            ThreatLevel::Hostile => "hostile",
        }
    }

    /// The weakest isolation this crate will accept for the level.
    ///
    /// A reading of 12.13, not a quotation. Only the `Trusted` row is directly supported by the
    /// text; the other two encode "do not assume ordinary namespaces isolate hostile code" plus
    /// the judgement that a hardened runtime on a shared kernel is not a boundary to bet a
    /// hostile workload on.
    pub fn minimum_isolation(self) -> IsolationStrength {
        match self {
            ThreatLevel::Trusted => IsolationStrength::SharedNamespace,
            ThreatLevel::Untrusted => IsolationStrength::HardenedRuntime,
            ThreatLevel::Hostile => IsolationStrength::MicroVm,
        }
    }
}

/// How much of the executor contract a provider passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceLevel {
    None,
    Partial,
    Full,
}

impl ConformanceLevel {
    pub fn name(self) -> &'static str {
        match self {
            ConformanceLevel::None => "none",
            ConformanceLevel::Partial => "partial",
            ConformanceLevel::Full => "full",
        }
    }
}

/// Everything a provider publishes about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProfile {
    id: ProviderId,
    kind: ProviderKind,
    region: Region,
    cost_units: u64,
    trust_domain: TrustDomain,
    isolation: IsolationStrength,
    capabilities: BTreeSet<Capability>,
    conformance: Attested<ConformanceLevel>,
}

impl ProviderProfile {
    /// All eight published facts at once.
    ///
    /// Long by clippy's default and deliberately not shortened into a builder: 12.13 requires a
    /// provider to publish cost, region, capabilities and conformance, and a builder with
    /// defaults would let a profile be constructed with three of the four. Every argument here is
    /// something the section says must be stated.
    #[expect(clippy::too_many_arguments, reason = "every field is a required declaration")]
    pub fn new(
        id: ProviderId,
        kind: ProviderKind,
        region: Region,
        cost_units: u64,
        trust_domain: TrustDomain,
        isolation: IsolationStrength,
        capabilities: impl IntoIterator<Item = Capability>,
        conformance: Attested<ConformanceLevel>,
    ) -> Self {
        ProviderProfile {
            id,
            kind,
            region,
            cost_units,
            trust_domain,
            isolation,
            capabilities: capabilities.into_iter().collect(),
            conformance,
        }
    }

    /// The provider's own claim about its conformance.
    ///
    /// A convenience that cannot lie: it stamps [`crate::basis::Basis::Declared`] with the
    /// provider's own id, so a caller cannot accidentally record a vendor claim as a measurement
    /// by picking the wrong constructor.
    pub fn self_declared_conformance(
        provider: &ProviderId,
        level: ConformanceLevel,
        at: Epoch,
    ) -> Result<Attested<ConformanceLevel>, ProviderError> {
        let party =
            PartyId::parse(provider.as_str()).map_err(|_| ProviderError::MalformedField {
                field: "provider id",
                value: provider.to_string(),
            })?;
        Ok(Attested::declared(level, party, at))
    }

    pub fn id(&self) -> &ProviderId {
        &self.id
    }

    pub fn kind(&self) -> ProviderKind {
        self.kind
    }

    pub fn region(&self) -> &Region {
        &self.region
    }

    pub fn cost_units(&self) -> u64 {
        self.cost_units
    }

    pub fn trust_domain(&self) -> &TrustDomain {
        &self.trust_domain
    }

    pub fn isolation(&self) -> IsolationStrength {
        self.isolation
    }

    pub fn capabilities(&self) -> &BTreeSet<Capability> {
        &self.capabilities
    }

    pub fn conformance(&self) -> &Attested<ConformanceLevel> {
        &self.conformance
    }
}

/// What the platform requires before it will send work to a provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionPolicy {
    pub minimum_conformance: ConformanceLevel,
    /// Whether a provider's own conformance claim is acceptable.
    ///
    /// Defaulting this to `false` would be the comfortable choice and would make the distinction
    /// [`Attested`] draws invisible in practice, so there is no `Default` impl and a caller has to
    /// write down which of the two worlds it is in.
    pub require_verified_conformance: bool,
}

/// A set of declared providers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderCatalog {
    providers: BTreeMap<ProviderId, ProviderProfile>,
}

impl ProviderCatalog {
    pub fn new() -> Self {
        ProviderCatalog::default()
    }

    pub fn declare(&mut self, profile: ProviderProfile) -> Result<(), ProviderError> {
        if self.providers.contains_key(profile.id()) {
            return Err(ProviderError::DuplicateProvider {
                provider: profile.id().to_string(),
            });
        }
        self.providers.insert(profile.id().clone(), profile);
        Ok(())
    }

    pub fn get(&self, id: &ProviderId) -> Option<&ProviderProfile> {
        self.providers.get(id)
    }

    pub fn ids(&self) -> Vec<&ProviderId> {
        self.providers.keys().collect()
    }

    pub fn profiles(&self) -> impl Iterator<Item = &ProviderProfile> {
        self.providers.values()
    }

    /// Whether this provider may run work at this threat level under this policy.
    ///
    /// Isolation is checked before conformance because it is the security decision: a provider
    /// with a perfect verified conformance result and a shared namespace must not run hostile
    /// code, and reporting the conformance failure first would suggest that fixing the paperwork
    /// is the remedy.
    pub fn admit(
        &self,
        id: &ProviderId,
        threat: ThreatLevel,
        policy: &AdmissionPolicy,
    ) -> Result<&ProviderProfile, ProviderError> {
        let profile = self
            .providers
            .get(id)
            .ok_or_else(|| ProviderError::UnknownProvider {
                provider: id.to_string(),
            })?;
        if !profile.isolation().adequate_for(threat) {
            return Err(ProviderError::IsolationInadequate {
                provider: id.to_string(),
                offered: profile.isolation().name(),
                required: threat.minimum_isolation().name(),
                threat: threat.name(),
            });
        }
        if policy.require_verified_conformance && !profile.conformance().basis().is_first_hand() {
            return Err(ProviderError::ConformanceNotVerified {
                provider: id.to_string(),
                basis: profile.conformance().basis().name().to_string(),
            });
        }
        if *profile.conformance().value() < policy.minimum_conformance {
            return Err(ProviderError::ConformanceNotVerified {
                provider: id.to_string(),
                basis: profile.conformance().value().name().to_string(),
            });
        }
        Ok(profile)
    }
}

/// A pool of pre-started workers, pinned to one trust domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WarmPool {
    provider: ProviderId,
    trust_domain: TrustDomain,
    warm: u64,
}

impl WarmPool {
    pub fn new(provider: ProviderId, trust_domain: TrustDomain, warm: u64) -> Self {
        WarmPool {
            provider,
            trust_domain,
            warm,
        }
    }

    pub fn provider(&self) -> &ProviderId {
        &self.provider
    }

    pub fn trust_domain(&self) -> &TrustDomain {
        &self.trust_domain
    }

    pub fn warm(&self) -> u64 {
        self.warm
    }

    /// 12.13: "warm pools are limited to compatible trust domains."
    ///
    /// Compatible is read as identical. A warm worker has already executed something, and the
    /// only defensible statement about what it may execute next is "more of the same tenant's
    /// work"; anything looser is a reuse rule that needs a residue argument this crate cannot
    /// make.
    pub fn admits(&self, job_domain: &TrustDomain) -> Result<(), ProviderError> {
        if &self.trust_domain == job_domain {
            Ok(())
        } else {
            Err(ProviderError::TrustDomainMismatch {
                pool: self.trust_domain.to_string(),
                job: job_domain.to_string(),
            })
        }
    }
}

/// The four services 12.13 forbids the reference local path from requiring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalService {
    Kubernetes,
    Redis,
    Kafka,
    CloudObjectStorage,
}

impl ExternalService {
    pub const ALL: [ExternalService; 4] = [
        ExternalService::Kubernetes,
        ExternalService::Redis,
        ExternalService::Kafka,
        ExternalService::CloudObjectStorage,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ExternalService::Kubernetes => "kubernetes",
            ExternalService::Redis => "redis",
            ExternalService::Kafka => "kafka",
            ExternalService::CloudObjectStorage => "cloud object storage",
        }
    }

    /// Substrings in a technology name that indicate this service.
    ///
    /// Substring matching on a declared technology string is a weak test and is the strongest one
    /// available without a package manager. It catches a topology that names the service and does
    /// not catch one that reaches it through a wrapper, which is stated so nobody reads a pass
    /// here as proof the local path is self-contained.
    pub fn markers(self) -> &'static [&'static str] {
        match self {
            ExternalService::Kubernetes => &["kubernetes", "k8s"],
            ExternalService::Redis => &["redis"],
            ExternalService::Kafka => &["kafka"],
            ExternalService::CloudObjectStorage => &["s3", "gcs", "azure-blob"],
        }
    }
}

/// 12.13: "the reference local path works without Kubernetes, Redis, Kafka, or cloud object
/// storage."
///
/// Checked over the technologies a topology names and the kinds of provider it declares. The
/// section states this as a property of the shipped system, which means it is exactly the kind of
/// claim that decays silently; this is the assertion that fails a test instead.
pub fn local_path_is_self_contained(
    topology: &StorageTopology,
    providers: &ProviderCatalog,
) -> Result<(), ProviderError> {
    if providers
        .profiles()
        .any(|profile| profile.kind() == ProviderKind::SelfHostedKubernetes)
    {
        return Err(ProviderError::LocalPathNeedsExternalService {
            service: ExternalService::Kubernetes.name(),
        });
    }
    for technology in topology.technologies() {
        let lowered = technology.to_ascii_lowercase();
        for service in ExternalService::ALL {
            if service
                .markers()
                .iter()
                .any(|marker| lowered.contains(marker))
            {
                return Err(ProviderError::LocalPathNeedsExternalService {
                    service: service.name(),
                });
            }
        }
    }
    Ok(())
}
