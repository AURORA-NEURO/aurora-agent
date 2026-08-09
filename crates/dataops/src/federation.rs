//! Cloud and federated deployment (12.16): planes, residency and imports from hubs somebody else
//! governs.
//!
//! 12.16's five subsections split three to two. *Planes* and *tenant patterns* are enumerations
//! with a compatibility rule between them, *private workers* is a connection-direction constraint,
//! and *regions* and *federation* are placement and trust predicates. What is **not** here is the
//! operational half of each: no VPC, no load balancer, no secret manager, no key distribution, no
//! replication transport.
//!
//! # The plane split is the whole security argument
//!
//! "Execution pools may live in customer networks" is the sentence 12.16 builds on, and it is
//! stated about exactly one plane. [`DeploymentPlan::validate`] is that asymmetry made
//! executable: a plan that puts the control API, the catalog, the scheduler or the signing
//! service inside a customer network under a shared-control pattern is refused by name. Under a
//! dedicated or air-gapped pattern the same placement is correct, which is why the pattern is an
//! argument rather than a constant.
//!
//! # Outbound-only is a property of the plan, not of a firewall
//!
//! 12.16 requires private workers to make an "outbound-only connection … no inbound firewall
//! requirement". A crate with no sockets cannot enforce that; what it can do is refuse to produce
//! a plan that would need it. [`private_worker_link`] takes the direction as a value and rejects
//! [`ConnectionDirection::InboundToCustomer`] for any plane placed in a customer network, so the
//! requirement fails at design time instead of at a customer's security review.
//!
//! # A federated record is never first-hand
//!
//! [`import_record`] is the only way into a local catalog from a hub, and it stamps
//! [`Basis::Replicated`] unconditionally — there is no argument by which a caller can ask for
//! anything else. A record replicated from another hub and a record this deployment published are
//! therefore distinguishable by `==`, permanently, and 12.16's "local policy decides which
//! external publishers and attestations to trust" has something concrete to decide about.
//!
//! The two `Replicated` epochs are both required for a reason 12.16 does not state: signed
//! catalog replication tells you when the origin's answer was current, and the local receipt tells
//! you when you got it. An import that recorded only the second cannot distinguish a fresh copy of
//! stale data from a stale copy of fresh data, and both happen in a federation whose members
//! replicate on different schedules.
//!
//! # Not implemented
//!
//! No signature verification. [`SignedRecord::attestation`] is an opaque string whose presence is
//! checked and whose contents are not, because verifying it needs a public key this crate has no
//! way to obtain; [`FederationPolicy::accept_unattested`] is therefore a policy about *presence*,
//! and saying it is a policy about validity would be a lie. No transport, no replication schedule,
//! no conflict resolution between hubs that both changed a record, no revocation. Residency is a
//! label compared against another label; nothing here can tell where a byte actually is.

use crate::basis::{Attested, Basis, Coverage, PartyId};
use crate::error::{check_name, FederationError};
use crate::provider::Region;
use bioprism_infra::Epoch;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The nine planes 12.16 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Plane {
    ControlApi,
    Catalog,
    ArtifactStorage,
    Scheduler,
    ExecutionPool,
    Analytics,
    Search,
    Signing,
    Observability,
}

impl Plane {
    pub const ALL: [Plane; 9] = [
        Plane::ControlApi,
        Plane::Catalog,
        Plane::ArtifactStorage,
        Plane::Scheduler,
        Plane::ExecutionPool,
        Plane::Analytics,
        Plane::Search,
        Plane::Signing,
        Plane::Observability,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Plane::ControlApi => "control-api",
            Plane::Catalog => "catalog",
            Plane::ArtifactStorage => "artifact-storage",
            Plane::Scheduler => "scheduler",
            Plane::ExecutionPool => "execution-pool",
            Plane::Analytics => "analytics",
            Plane::Search => "search",
            Plane::Signing => "signing",
            Plane::Observability => "observability",
        }
    }

    /// Whether the plane is part of the control plane rather than the execution plane.
    ///
    /// 12.16 opens with "public API/registry control" and lists the scheduler and the signing
    /// service alongside it, then says only that *execution pools* may live in customer networks.
    /// Everything that is not the execution pool is treated as control here; the section does not
    /// classify the analytics, search and observability planes, and putting them on the
    /// conservative side is a decision, stated so it can be argued with.
    pub fn is_control_plane(self) -> bool {
        !matches!(self, Plane::ExecutionPool)
    }
}

/// Where a plane runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanePlacement {
    /// In the hub operator's own infrastructure.
    HubOperated,
    /// Inside the customer's network.
    CustomerNetwork,
    /// In an installation with no route to anything.
    AirGapped,
}

impl PlanePlacement {
    pub fn name(self) -> &'static str {
        match self {
            PlanePlacement::HubOperated => "hub-operated",
            PlanePlacement::CustomerNetwork => "customer-network",
            PlanePlacement::AirGapped => "air-gapped",
        }
    }
}

/// 12.16's four tenant patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantPattern {
    /// Shared control with isolated data and worker pools.
    SharedControl,
    /// A dedicated installation for one tenant.
    DedicatedInstallation,
    /// An air-gapped registry.
    AirGappedRegistry,
    /// Public metadata, private artifacts.
    HybridPublicMetadata,
}

impl TenantPattern {
    pub fn name(self) -> &'static str {
        match self {
            TenantPattern::SharedControl => "shared-control",
            TenantPattern::DedicatedInstallation => "dedicated-installation",
            TenantPattern::AirGappedRegistry => "air-gapped-registry",
            TenantPattern::HybridPublicMetadata => "hybrid-public-metadata",
        }
    }

    /// Whether the pattern permits a control plane inside a customer network.
    fn permits_customer_control_plane(self) -> bool {
        matches!(
            self,
            TenantPattern::DedicatedInstallation | TenantPattern::AirGappedRegistry
        )
    }
}

/// A placement for every plane.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentPlan {
    placements: BTreeMap<Plane, PlanePlacement>,
}

impl DeploymentPlan {
    pub fn new() -> Self {
        DeploymentPlan::default()
    }

    pub fn place(mut self, plane: Plane, placement: PlanePlacement) -> Self {
        self.placements.insert(plane, placement);
        self
    }

    pub fn placement_of(&self, plane: Plane) -> Option<PlanePlacement> {
        self.placements.get(&plane).copied()
    }

    /// Checks the plan against a tenant pattern.
    ///
    /// An unplaced plane is refused before any compatibility question, because a plan with a hole
    /// in it will be completed by whoever deploys it and the completion will not be reviewed.
    pub fn validate(&self, pattern: TenantPattern) -> Result<(), FederationError> {
        for plane in Plane::ALL {
            let Some(placement) = self.placement_of(plane) else {
                return Err(FederationError::PlaneUnplaced { plane: plane.name() });
            };
            if placement == PlanePlacement::CustomerNetwork
                && plane.is_control_plane()
                && !pattern.permits_customer_control_plane()
            {
                return Err(FederationError::PlaneMisplaced {
                    plane: plane.name(),
                    pattern: pattern.name(),
                });
            }
        }
        if pattern == TenantPattern::AirGappedRegistry {
            for plane in Plane::ALL {
                if self.placement_of(plane) != Some(PlanePlacement::AirGapped) {
                    return Err(FederationError::PlaneMisplaced {
                        plane: plane.name(),
                        pattern: pattern.name(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Which end opens the connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionDirection {
    /// The customer's worker dials out to the hub. 12.16's requirement.
    OutboundFromCustomer,
    /// The hub dials in. Needs a hole in the customer's firewall.
    InboundToCustomer,
}

/// Checks a link to a plane running in a customer network.
///
/// Refuses inbound. A plane that is not in a customer network is unconstrained, because the
/// requirement is about the customer's firewall and there is nothing to protect otherwise.
pub fn private_worker_link(
    plan: &DeploymentPlan,
    plane: Plane,
    direction: ConnectionDirection,
) -> Result<(), FederationError> {
    if plan.placement_of(plane) == Some(PlanePlacement::CustomerNetwork)
        && direction == ConnectionDirection::InboundToCustomer
    {
        return Err(FederationError::InboundRequired { plane: plane.name() });
    }
    Ok(())
}

/// Whether an artifact may leave its home region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactSensitivity {
    /// 12.16: "public artifacts replicated".
    Public,
    /// 12.16: "sensitive artifacts pinned".
    Sensitive,
}

/// What was decided about an artifact's copies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "replication", rename_all = "snake_case")]
pub enum Replication {
    Replicated { to: BTreeSet<Region> },
    Pinned { region: Region },
}

/// Decides replication for one artifact.
///
/// A sensitive artifact with a non-empty target list is a typed error rather than a silently
/// pinned artifact: the caller asked for something the residency rule forbids, and quietly doing
/// the safe thing instead would leave them believing the copies exist.
pub fn plan_replication(
    artifact: &str,
    sensitivity: ArtifactSensitivity,
    home: &Region,
    targets: impl IntoIterator<Item = Region>,
) -> Result<Replication, FederationError> {
    if !check_name(artifact) {
        return Err(FederationError::MalformedField {
            field: "artifact",
            value: artifact.to_string(),
        });
    }
    let targets: BTreeSet<Region> = targets.into_iter().filter(|region| region != home).collect();
    match sensitivity {
        ArtifactSensitivity::Sensitive if !targets.is_empty() => {
            Err(FederationError::SensitiveArtifactReplication {
                artifact: artifact.to_string(),
                region: home.to_string(),
            })
        }
        ArtifactSensitivity::Sensitive => Ok(Replication::Pinned {
            region: home.clone(),
        }),
        ArtifactSensitivity::Public => Ok(Replication::Replicated { to: targets }),
    }
}

/// 12.16: "cross-region metadata minimized".
///
/// Keeps only the fields on the allow-list. An allow-list rather than a deny-list because the
/// failure mode is a field nobody thought about, and a deny-list gets that one wrong by default.
pub fn minimize_cross_region(
    record: &BTreeMap<String, Value>,
    allowed: &BTreeSet<String>,
) -> BTreeMap<String, Value> {
    record
        .iter()
        .filter(|(field, _)| allowed.contains(*field))
        .map(|(field, value)| (field.clone(), value.clone()))
        .collect()
}

/// A catalog record offered by another hub.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedRecord {
    pub hub: PartyId,
    pub payload: Value,
    /// An opaque attestation blob. Its presence is checked; its validity is not.
    pub attestation: Option<String>,
    /// The origin's own epoch for this record.
    pub origin_epoch: Epoch,
}

/// Which publishers this deployment trusts, and whether an unattested record is acceptable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationPolicy {
    trusted_publishers: BTreeSet<PartyId>,
    /// Whether a record with no attestation blob may be imported.
    ///
    /// Not defaulted. 12.16 leaves the decision to "local policy" and a default here would be
    /// this crate quietly making it for every deployment that never looked.
    pub accept_unattested: bool,
}

impl FederationPolicy {
    pub fn new(
        trusted: impl IntoIterator<Item = PartyId>,
        accept_unattested: bool,
    ) -> Self {
        FederationPolicy {
            trusted_publishers: trusted.into_iter().collect(),
            accept_unattested,
        }
    }

    pub fn trusts(&self, hub: &PartyId) -> bool {
        self.trusted_publishers.contains(hub)
    }
}

/// Imports a record from a federated hub.
///
/// The only import path, and it always produces [`Basis::Replicated`]. There is no parameter that
/// makes it first-hand and no second function that skips the stamp, so no amount of downstream
/// code can lose the fact that this record came from somewhere else.
pub fn import_record(
    record: &SignedRecord,
    policy: &FederationPolicy,
    received_at: Epoch,
) -> Result<Attested<Value>, FederationError> {
    if !policy.trusts(&record.hub) {
        return Err(FederationError::UntrustedPublisher {
            hub: record.hub.to_string(),
        });
    }
    if record.attestation.is_none() && !policy.accept_unattested {
        return Err(FederationError::UnattestedImport {
            hub: record.hub.to_string(),
        });
    }
    Ok(Attested::new(
        record.payload.clone(),
        Basis::Replicated {
            origin: record.hub.clone(),
            origin_epoch: record.origin_epoch,
            received_at,
        },
        Coverage::Complete { observed: 1 },
    ))
}
