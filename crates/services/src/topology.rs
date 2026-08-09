//! One service set, two placements.
//!
//! Implements the sentence blueprint 40.03 ends on: *the alpha must not require hosted services;
//! the same application services run in process locally and behind API/worker boundaries when
//! hosted; events, object IDs and result bundles remain identical.* Three claims, and all three are
//! checkable against a pair of [`Deployment`]s.
//!
//! The one that earns its keep is the first. A service that exists only in the hosted topology is
//! how a local-first system stops being local-first: nothing announces it, the alpha simply grows a
//! feature that needs a queue. [`TopologyError::AlphaRequiresHosting`] is that growth becoming a
//! test failure.
//!
//! # What a placement is not
//!
//! It is not a process, a container, a host or a port. Nothing here starts, connects to or knows
//! about anything running. A placement records *which side of a boundary* a responsibility sits on,
//! which is the only part of deployment a contract has an opinion about — because it is the part
//! that decides whether a call is a function call or an integration.

use crate::graph::ServiceId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// Which of 40.03's two diagrams a deployment is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Topology {
    /// CLI or local web over an in-process core, SQLite/DuckDB, local CAS, executor subprocess.
    Alpha,
    /// Web and CLI over an API service, PostgreSQL, object storage, a job ledger and worker pools.
    Hosted,
}

impl Topology {
    pub fn as_str(self) -> &'static str {
        match self {
            Topology::Alpha => "alpha",
            Topology::Hosted => "hosted",
        }
    }
}

impl fmt::Display for Topology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a responsibility sits relative to a boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Inside the caller's process. No boundary at all.
    InProcess,
    /// A subprocess the core drives directly. 40.03's alpha draws the OCI executor this way, and
    /// it is still local: no service is required to be reachable over a network.
    Subprocess,
    /// Behind an API boundary.
    ApiService,
    /// Behind a job ledger, taken up by a worker.
    Worker,
}

impl Placement {
    /// Whether this placement requires something to be hosted for the service to work.
    pub fn requires_hosting(self) -> bool {
        matches!(self, Placement::ApiService | Placement::Worker)
    }
}

/// A service set with a placement for each member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub topology: Topology,
    pub placements: Vec<(ServiceId, Placement)>,
}

impl Deployment {
    pub fn new(topology: Topology, placements: Vec<(ServiceId, Placement)>) -> Self {
        Deployment {
            topology,
            placements,
        }
    }

    pub fn services(&self) -> BTreeSet<&ServiceId> {
        self.placements.iter().map(|(id, _)| id).collect()
    }

    pub fn placement_of(&self, service: &ServiceId) -> Option<Placement> {
        self.placements
            .iter()
            .find(|(id, _)| id == service)
            .map(|(_, placement)| *placement)
    }

    /// The services that cannot run without something hosted.
    pub fn hosted_dependencies(&self) -> Vec<&ServiceId> {
        self.placements
            .iter()
            .filter(|(_, placement)| placement.requires_hosting())
            .map(|(id, _)| id)
            .collect()
    }
}

/// Ways a pair of topologies can break 40.03's closing paragraph.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TopologyError {
    #[error("{service} is placed {placement:?} in the alpha topology, so the alpha requires a hosted service")]
    AlphaRequiresHosting {
        service: ServiceId,
        placement: Placement,
    },

    #[error("{service} exists only in the {topology} topology; the same application services must run in both")]
    ServiceOnlyIn {
        service: ServiceId,
        topology: Topology,
    },

    #[error("{service} is placed twice in the {topology} topology")]
    PlacedTwice {
        service: ServiceId,
        topology: Topology,
    },
}

/// Hold a local and a hosted deployment against 40.03.
///
/// Deterministic and complete: every finding, alpha-only checks first, then set differences.
pub fn compare(alpha: &Deployment, hosted: &Deployment) -> Vec<TopologyError> {
    let mut findings = Vec::new();
    findings.extend(duplicates(alpha));
    findings.extend(duplicates(hosted));

    for (service, placement) in &alpha.placements {
        if placement.requires_hosting() {
            findings.push(TopologyError::AlphaRequiresHosting {
                service: service.clone(),
                placement: *placement,
            });
        }
    }

    let local = alpha.services();
    let remote = hosted.services();
    for service in local.difference(&remote) {
        findings.push(TopologyError::ServiceOnlyIn {
            service: (*service).clone(),
            topology: alpha.topology,
        });
    }
    for service in remote.difference(&local) {
        findings.push(TopologyError::ServiceOnlyIn {
            service: (*service).clone(),
            topology: hosted.topology,
        });
    }
    findings
}

fn duplicates(deployment: &Deployment) -> Vec<TopologyError> {
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for (service, _) in &deployment.placements {
        if !seen.insert(service) {
            findings.push(TopologyError::PlacedTwice {
                service: service.clone(),
                topology: deployment.topology,
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(placements: &[(&str, Placement)]) -> Deployment {
        Deployment::new(
            Topology::Alpha,
            placements
                .iter()
                .map(|(name, placement)| (ServiceId::new(*name), *placement))
                .collect(),
        )
    }

    fn hosted(placements: &[(&str, Placement)]) -> Deployment {
        Deployment::new(
            Topology::Hosted,
            placements
                .iter()
                .map(|(name, placement)| (ServiceId::new(*name), *placement))
                .collect(),
        )
    }

    #[test]
    fn the_same_services_placed_differently_satisfy_the_topology_contract() {
        let findings = compare(
            &alpha(&[
                ("world-builder", Placement::InProcess),
                ("executor", Placement::Subprocess),
            ]),
            &hosted(&[
                ("world-builder", Placement::Worker),
                ("executor", Placement::Worker),
            ]),
        );
        assert_eq!(findings, Vec::new());
    }

    #[test]
    fn a_service_the_alpha_places_behind_a_boundary_makes_the_alpha_require_hosting() {
        let findings = compare(
            &alpha(&[("registry-backend", Placement::ApiService)]),
            &hosted(&[("registry-backend", Placement::ApiService)]),
        );
        assert!(matches!(
            findings.as_slice(),
            [TopologyError::AlphaRequiresHosting { .. }]
        ));
    }

    #[test]
    fn an_executor_subprocess_does_not_make_the_alpha_hosted() {
        let deployment = alpha(&[("executor", Placement::Subprocess)]);
        assert_eq!(deployment.hosted_dependencies(), Vec::<&ServiceId>::new());
    }

    #[test]
    fn a_service_that_exists_only_when_hosted_is_reported_against_the_hosted_topology() {
        let findings = compare(
            &alpha(&[("world-builder", Placement::InProcess)]),
            &hosted(&[
                ("world-builder", Placement::Worker),
                ("search-indexer", Placement::Worker),
            ]),
        );
        assert!(matches!(
            findings.as_slice(),
            [TopologyError::ServiceOnlyIn {
                topology: Topology::Hosted,
                ..
            }]
        ));
    }

    #[test]
    fn a_service_placed_twice_is_reported_before_anything_else() {
        let findings = compare(
            &alpha(&[
                ("world-builder", Placement::InProcess),
                ("world-builder", Placement::InProcess),
            ]),
            &hosted(&[("world-builder", Placement::Worker)]),
        );
        assert!(matches!(
            findings.first(),
            Some(TopologyError::PlacedTwice { .. })
        ));
    }

    #[test]
    fn placement_lookup_reports_the_side_of_the_boundary_a_service_sits_on() {
        let deployment = hosted(&[("adaptive-scheduler", Placement::ApiService)]);
        assert_eq!(
            deployment.placement_of(&ServiceId::new("adaptive-scheduler")),
            Some(Placement::ApiService)
        );
        assert_eq!(deployment.placement_of(&ServiceId::new("absent")), None);
    }
}
