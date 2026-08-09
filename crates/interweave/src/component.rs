//! Component contracts, capability-oriented imports, and composition checks.
//!
//! Blueprint 23.25.
//!
//! # The one thing this module is for
//!
//! 23.25: "A component can use only imported interfaces. If it has no filesystem, network, clock,
//! or secret import, it cannot directly access those resources through the component boundary."
//! That is a decision procedure over two declarations, and [`permits`] is it. A component that
//! declares `filesystem.write` in its effects while importing no filesystem handle is refused at
//! contract-check time, with the missing import named.
//!
//! The mapping from `bioprism_fabric::EffectKind` to the host service an effect needs is **not in
//! the blueprint**. 23.25 lists eight host-supplied services and 23.14 lists eighteen effect kinds
//! and nothing relates them, so [`HostService::required_for`] is this crate's reading and is
//! documented as such at the function. Without it, 23.25's central sentence has nothing to
//! evaluate.
//!
//! # What "sandbox" means here and what it does not
//!
//! Nothing in this module isolates anything. There is no WebAssembly runtime, no WIT parser, no
//! subprocess, no container and no microVM; the crate performs no effect at all. What is
//! implemented is the *contract layer* that a runtime would enforce: what a component declared,
//! whether a composition of two components type-checks, whether a package manifest satisfies
//! 23.25's supply-chain controls, and which provider was claimed. The value of that split is that
//! a violation is detectable before anything runs, which is when 23.25's own evaluation list
//! ("component attempts undeclared filesystem access", "schema mismatch at boundary") wants it
//! detected.
//!
//! 23.25's WIT example is not parsed. Interfaces here are named and versioned values; the record
//! and function shapes inside them are opaque, so "schema mismatch at boundary" is decided on
//! interface identity and version, never on field structure.

use bioprism_fabric::effect::{EffectKind, EffectSet};
use bioprism_fabric::flow::Sensitivity;
use bioprism_fabric::molecule::Version;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The host services 23.25 says the host supplies, one per bullet of its list.
///
/// "clocks and randomness" is one bullet in the blueprint and is one variant here. Splitting it
/// would be a reading, and for the only question this module asks of it — does importing it cost
/// determinism — both halves answer the same way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostService {
    RestrictedFilesystem,
    NetworkProxy,
    ClocksAndRandomness,
    ArtifactStore,
    LoggingSink,
    BudgetMeter,
    SecretBroker,
    WeaveEventApi,
}

impl HostService {
    pub const ALL: [HostService; 8] = [
        HostService::RestrictedFilesystem,
        HostService::NetworkProxy,
        HostService::ClocksAndRandomness,
        HostService::ArtifactStore,
        HostService::LoggingSink,
        HostService::BudgetMeter,
        HostService::SecretBroker,
        HostService::WeaveEventApi,
    ];

    /// Which host import an effect kind cannot be performed without.
    ///
    /// **Not in the blueprint.** 23.25 gives eight host services, 23.14 gives eighteen effect
    /// kinds, and §23 never relates them — which leaves 23.25's central claim, that a component
    /// without a filesystem import cannot touch the filesystem, with nothing to check. The mapping
    /// below is this crate's reading.
    ///
    /// `None` means the effect needs no host service: it is either pure, or it is a Weave-level
    /// action whose gate is authority rather than a sandbox boundary. `agent.spawn` and
    /// `agent.delegate` fall in the second group and are governed by
    /// `bioprism_weave::AuthorityTable`, not by an import.
    pub fn required_for(kind: EffectKind) -> Option<HostService> {
        match kind {
            EffectKind::PureCompute => None,
            EffectKind::ArtifactRead | EffectKind::ArtifactWrite => Some(HostService::ArtifactStore),
            EffectKind::FilesystemRead | EffectKind::FilesystemWrite | EffectKind::ProcessExecute => {
                Some(HostService::RestrictedFilesystem)
            }
            EffectKind::NetworkRead | EffectKind::NetworkWrite | EffectKind::ExternalPublish => {
                Some(HostService::NetworkProxy)
            }
            EffectKind::MessageSend | EffectKind::HumanContact => Some(HostService::WeaveEventApi),
            EffectKind::BudgetSpend => Some(HostService::BudgetMeter),
            EffectKind::SecretUse => Some(HostService::SecretBroker),
            EffectKind::AgentSpawn
            | EffectKind::AgentDelegate
            | EffectKind::PolicyChange
            | EffectKind::ClinicalOutput
            | EffectKind::IrreversibleEffect => None,
        }
    }
}

/// The component roles 23.25 enumerates. Not every participant is an LLM agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentRole {
    Parser,
    DeterministicEvaluator,
    SchemaAdapter,
    DataTransform,
    PolicyMonitor,
    MutationGenerator,
    ContextScorer,
    ArtifactValidator,
    LocalSpecialistModel,
    CompleteMolecule,
}

impl ComponentRole {
    pub const ALL: [ComponentRole; 10] = [
        ComponentRole::Parser,
        ComponentRole::DeterministicEvaluator,
        ComponentRole::SchemaAdapter,
        ComponentRole::DataTransform,
        ComponentRole::PolicyMonitor,
        ComponentRole::MutationGenerator,
        ComponentRole::ContextScorer,
        ComponentRole::ArtifactValidator,
        ComponentRole::LocalSpecialistModel,
        ComponentRole::CompleteMolecule,
    ];
}

/// A named, versioned interface. The shape inside it is not modelled; see the module docs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Interface {
    /// e.g. `aurora:weave-evidence/scorer`.
    pub name: String,
    pub version: Version,
}

impl Interface {
    pub fn new(name: impl Into<String>, version: Version) -> Self {
        Interface {
            name: name.into(),
            version,
        }
    }
}

/// Whether a component's outputs are a function of its declared inputs.
///
/// 23.25 asks that "verification-critical components should be deterministic where practical", and
/// separately that an LLM-backed component "is still nondeterministic and external-provider
/// dependent". The two are one question, so they are one type, and the nondeterministic case
/// carries its reason: an unexplained nondeterminism claim cannot be audited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "determinism")]
pub enum Determinism {
    /// Inputs are pinned and outputs are content-addressed, as 23.25 requires of this case.
    Deterministic {
        pinned_inputs: BTreeSet<String>,
        content_addressed_outputs: bool,
    },
    Nondeterministic {
        reason: String,
    },
}

impl Determinism {
    /// Whether this component may sit on a replay path.
    ///
    /// A deterministic component that does not content-address its outputs is *not* replayable:
    /// replay compares outputs, and an output with no address cannot be compared. 23.25 states
    /// both halves in one sentence and this predicate keeps them joined.
    pub fn replayable(&self) -> bool {
        matches!(
            self,
            Determinism::Deterministic {
                content_addressed_outputs: true,
                ..
            }
        )
    }
}

/// 23.25's execution providers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    WasmComponent,
    SubprocessSandbox,
    ContainerSandbox,
    MicroVm,
    RemoteService,
    A2AAgent,
    McpServer,
}

/// How much of the component boundary a provider actually enforces.
///
/// **Ordering not in the blueprint.** 23.25 lists seven providers and says "all implement the same
/// executor provider interface and declare isolation fidelity", without saying what fidelities
/// exist or how they compare. Since the sentence exists to let a policy say "this component needs
/// at least X", the levels below are this crate's reading, ordered weakest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IsolationFidelity {
    /// The boundary is a promise by another party. Nothing local enforces it.
    Declared,
    /// Operating-system process boundary.
    Process,
    /// Namespaced filesystem and network.
    Container,
    /// Hardware-assisted virtual machine.
    MicroVm,
    /// Capability-oriented: the component cannot name what it did not import.
    CapabilityScoped,
}

impl Provider {
    pub const ALL: [Provider; 7] = [
        Provider::WasmComponent,
        Provider::SubprocessSandbox,
        Provider::ContainerSandbox,
        Provider::MicroVm,
        Provider::RemoteService,
        Provider::A2AAgent,
        Provider::McpServer,
    ];

    /// See [`IsolationFidelity`]: this mapping is this crate's reading, not the blueprint's.
    pub fn fidelity(self) -> IsolationFidelity {
        match self {
            Provider::WasmComponent => IsolationFidelity::CapabilityScoped,
            Provider::SubprocessSandbox => IsolationFidelity::Process,
            Provider::ContainerSandbox => IsolationFidelity::Container,
            Provider::MicroVm => IsolationFidelity::MicroVm,
            Provider::RemoteService | Provider::A2AAgent | Provider::McpServer => {
                IsolationFidelity::Declared
            }
        }
    }
}

/// A component's declaration: what it imports, what it exports, what it may do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentContract {
    pub name: String,
    pub role: ComponentRole,
    pub provider: Provider,
    pub imports: BTreeSet<Interface>,
    pub exports: BTreeSet<Interface>,
    /// Host services the component asked for, and the runtime granted.
    pub host_imports: BTreeSet<HostService>,
    pub effects: EffectSet,
    pub determinism: Determinism,
    /// The highest sensitivity of data this component is cleared to hold.
    pub clearance: Sensitivity,
}

impl ComponentContract {
    pub fn new(
        name: impl Into<String>,
        role: ComponentRole,
        provider: Provider,
        determinism: Determinism,
    ) -> Self {
        ComponentContract {
            name: name.into(),
            role,
            provider,
            imports: BTreeSet::new(),
            exports: BTreeSet::new(),
            host_imports: BTreeSet::new(),
            effects: EffectSet::new(),
            determinism,
            clearance: Sensitivity::Public,
        }
    }

    pub fn importing(mut self, interface: Interface) -> Self {
        self.imports.insert(interface);
        self
    }

    pub fn exporting(mut self, interface: Interface) -> Self {
        self.exports.insert(interface);
        self
    }

    pub fn with_host(mut self, service: HostService) -> Self {
        self.host_imports.insert(service);
        self
    }

    pub fn with_effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub fn cleared_to(mut self, clearance: Sensitivity) -> Self {
        self.clearance = clearance;
        self
    }
}

/// Why a component's declaration is not self-consistent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "denial")]
pub enum Denial {
    #[error("{component} declares {kind} but imports no {service:?}")]
    MissingHostImport {
        component: String,
        kind: EffectKind,
        service: HostService,
    },

    #[error("{component} claims determinism while importing {service:?}")]
    DeterminismContradictedByImport {
        component: String,
        service: HostService,
    },
}

/// 23.25's capability rule, as a decision procedure.
///
/// Returns every denial rather than the first, because a component with three missing imports
/// should be fixed once.
///
/// The determinism clause is the second half: a component that imports
/// [`HostService::ClocksAndRandomness`] and calls itself deterministic has contradicted itself, and
/// 23.25's "inputs include pinned schemas, artifacts, and seeds" does not cover a live clock.
pub fn permits(contract: &ComponentContract) -> Vec<Denial> {
    let mut denials = Vec::new();
    for effect in contract.effects.iter() {
        if let Some(service) = HostService::required_for(effect.kind) {
            if !contract.host_imports.contains(&service) {
                denials.push(Denial::MissingHostImport {
                    component: contract.name.clone(),
                    kind: effect.kind,
                    service,
                });
            }
        }
    }
    if matches!(contract.determinism, Determinism::Deterministic { .. })
        && contract
            .host_imports
            .contains(&HostService::ClocksAndRandomness)
    {
        denials.push(Denial::DeterminismContradictedByImport {
            component: contract.name.clone(),
            service: HostService::ClocksAndRandomness,
        });
    }
    denials
}

/// 23.25's six composition checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionCheck {
    TypeCompatibility,
    EffectCompatibility,
    AsyncAndStreamBehaviour,
    ResourceOwnership,
    SecurityLabels,
    VersionConstraints,
}

impl CompositionCheck {
    pub const ALL: [CompositionCheck; 6] = [
        CompositionCheck::TypeCompatibility,
        CompositionCheck::EffectCompatibility,
        CompositionCheck::AsyncAndStreamBehaviour,
        CompositionCheck::ResourceOwnership,
        CompositionCheck::SecurityLabels,
        CompositionCheck::VersionConstraints,
    ];
}

/// The outcome of one composition check.
///
/// Three-valued, following `bioprism_fabric::effect::Inclusion`: a check that could not be decided
/// from the declarations is not a pass. Two of the six are always [`CheckOutcome::Undecided`] here
/// and say why, which is more useful than a green tick this crate has not earned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum CheckOutcome {
    Holds,
    Fails { because: String },
    Undecided { because: String },
}

impl CheckOutcome {
    pub fn holds(&self) -> bool {
        matches!(self, CheckOutcome::Holds)
    }
}

/// A composition report: every check, named, with its outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositionReport {
    pub producer: String,
    pub consumer: String,
    pub outcomes: BTreeMap<CompositionCheck, CheckOutcome>,
}

impl CompositionReport {
    /// Whether every check holds. Undecided counts against, which is what a gate must do with it.
    pub fn admits(&self) -> bool {
        CompositionCheck::ALL
            .into_iter()
            .all(|check| self.outcomes.get(&check).is_some_and(CheckOutcome::holds))
    }

    pub fn failing(&self) -> BTreeSet<CompositionCheck> {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, CheckOutcome::Fails { .. }))
            .map(|(check, _)| *check)
            .collect()
    }

    pub fn undecided(&self) -> BTreeSet<CompositionCheck> {
        self.outcomes
            .iter()
            .filter(|(_, outcome)| matches!(outcome, CheckOutcome::Undecided { .. }))
            .map(|(check, _)| *check)
            .collect()
    }
}

/// Run 23.25's six checks over a producer/consumer pair.
///
/// `producer` exports what `consumer` imports. The four decidable checks:
///
/// - **type compatibility**: every interface the consumer imports is exported by the producer at
///   the same name. Version is the next check's business.
/// - **version constraints**: the exported major version equals the imported one. Minor and patch
///   are permitted to differ upward only, since a consumer written against `0.2.0` cannot rely on
///   `0.1.0`.
/// - **effect compatibility**: the consumer's effects are contained in the producer's declared
///   envelope where they overlap in kind — a consumer that writes the filesystem behind a producer
///   that declared no filesystem effect has escaped the composed declaration.
/// - **security labels**: the consumer's clearance is at least the producer's, or data flows
///   downhill into a component not cleared to hold it.
///
/// The two undecidable ones are async/stream behaviour and resource ownership: 23.25 states both as
/// properties of the WIT resource and stream types, which this crate does not model. They return
/// [`CheckOutcome::Undecided`] with that reason rather than passing.
pub fn compose(producer: &ComponentContract, consumer: &ComponentContract) -> CompositionReport {
    let mut outcomes = BTreeMap::new();

    let exported: BTreeMap<&str, Version> = producer
        .exports
        .iter()
        .map(|i| (i.name.as_str(), i.version))
        .collect();
    let unmatched: Vec<&str> = consumer
        .imports
        .iter()
        .map(|i| i.name.as_str())
        .filter(|name| !exported.contains_key(name))
        .collect();
    outcomes.insert(
        CompositionCheck::TypeCompatibility,
        if unmatched.is_empty() {
            CheckOutcome::Holds
        } else {
            CheckOutcome::Fails {
                because: format!("{} exports no {}", producer.name, unmatched.join(", ")),
            }
        },
    );

    let version_mismatch: Vec<String> = consumer
        .imports
        .iter()
        .filter_map(|imported| {
            exported.get(imported.name.as_str()).and_then(|available| {
                let compatible = available.major == imported.version.major
                    && (available.minor, available.patch)
                        >= (imported.version.minor, imported.version.patch);
                (!compatible).then(|| {
                    format!(
                        "{} wants {} and finds {}",
                        imported.name, imported.version, available
                    )
                })
            })
        })
        .collect();
    outcomes.insert(
        CompositionCheck::VersionConstraints,
        if version_mismatch.is_empty() {
            CheckOutcome::Holds
        } else {
            CheckOutcome::Fails {
                because: version_mismatch.join("; "),
            }
        },
    );

    let escaping: Vec<String> = consumer
        .effects
        .iter()
        .filter(|effect| !producer.effects.iter().any(|outer| outer.kind == effect.kind))
        .map(|effect| effect.kind.to_string())
        .collect();
    outcomes.insert(
        CompositionCheck::EffectCompatibility,
        if escaping.is_empty() {
            CheckOutcome::Holds
        } else {
            CheckOutcome::Fails {
                because: format!(
                    "{} performs {} outside {}'s declaration",
                    consumer.name,
                    escaping.join(", "),
                    producer.name
                ),
            }
        },
    );

    outcomes.insert(
        CompositionCheck::SecurityLabels,
        if consumer.clearance >= producer.clearance {
            CheckOutcome::Holds
        } else {
            CheckOutcome::Fails {
                because: format!(
                    "{} is cleared to {:?} and {} produces {:?}",
                    consumer.name, consumer.clearance, producer.name, producer.clearance
                ),
            }
        },
    );

    outcomes.insert(
        CompositionCheck::AsyncAndStreamBehaviour,
        CheckOutcome::Undecided {
            because: "stream and future types are not modelled; interfaces here are opaque".into(),
        },
    );
    outcomes.insert(
        CompositionCheck::ResourceOwnership,
        CheckOutcome::Undecided {
            because: "WIT resource handles are not modelled; ownership transfer is unrepresented"
                .into(),
        },
    );

    CompositionReport {
        producer: producer.name.clone(),
        consumer: consumer.name.clone(),
        outcomes,
    }
}

/// 23.25's eight supply-chain controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupplyChainControl {
    SignedPackageManifest,
    ContentHash,
    DependencyLock,
    SoftwareBillOfMaterials,
    ReproducibleBuild,
    VulnerabilityAndLicenseScan,
    CapabilityReview,
    NoDynamicDownloadWithoutPolicy,
}

impl SupplyChainControl {
    pub const ALL: [SupplyChainControl; 8] = [
        SupplyChainControl::SignedPackageManifest,
        SupplyChainControl::ContentHash,
        SupplyChainControl::DependencyLock,
        SupplyChainControl::SoftwareBillOfMaterials,
        SupplyChainControl::ReproducibleBuild,
        SupplyChainControl::VulnerabilityAndLicenseScan,
        SupplyChainControl::CapabilityReview,
        SupplyChainControl::NoDynamicDownloadWithoutPolicy,
    ];

    /// Whether 23.25 states this control unconditionally.
    ///
    /// Seven of the eight are stated flat. [`SupplyChainControl::ReproducibleBuild`] is qualified
    /// — "reproducible build **where possible**" — so a package may be admitted without it, and the
    /// admission says so rather than pretending the control was met.
    pub fn unconditional(self) -> bool {
        self != SupplyChainControl::ReproducibleBuild
    }
}

/// A package presented for admission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageManifest {
    pub component: String,
    pub satisfied: BTreeSet<SupplyChainControl>,
}

/// The result of a supply-chain admission check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "admission")]
pub enum Admission {
    /// Every unconditional control met, and the qualified one too.
    Admitted,
    /// Every unconditional control met; the qualified control was not, and is named.
    AdmittedWithQualifiedGap { unmet: BTreeSet<SupplyChainControl> },
    Refused { unmet: BTreeSet<SupplyChainControl> },
}

/// Admit or refuse a package against 23.25's controls.
pub fn admit(manifest: &PackageManifest) -> Admission {
    let unmet: BTreeSet<SupplyChainControl> = SupplyChainControl::ALL
        .into_iter()
        .filter(|c| !manifest.satisfied.contains(c))
        .collect();
    let blocking: BTreeSet<SupplyChainControl> = unmet
        .iter()
        .copied()
        .filter(|c| c.unconditional())
        .collect();
    if !blocking.is_empty() {
        Admission::Refused { unmet: blocking }
    } else if unmet.is_empty() {
        Admission::Admitted
    } else {
        Admission::AdmittedWithQualifiedGap { unmet }
    }
}

/// 23.25's evaluation list, recorded as the scenarios a runtime would have to survive.
///
/// Nothing here executes them; the crate has no runtime. Four of the eight are decidable from
/// declarations alone and are covered by this module's tests — undeclared access, schema mismatch,
/// cross-language composition and resource-handle ownership in its undecided form. The other four
/// need execution.
pub const EVALUATION_SCENARIOS: [&str; 8] = [
    "component attempts undeclared filesystem access",
    "schema mismatch at boundary",
    "deterministic replay",
    "cross-language composition",
    "cancellation of async streams",
    "resource-handle ownership",
    "malicious package quarantine",
    "comparison of Wasm and container execution fidelity",
];
