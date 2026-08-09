//! Who owns which §27 module.
//!
//! §27 is an *index* as much as a specification: much of its content restates material that other
//! blueprint sections carry in more detail, and the workspace built against those sections rather
//! than against 27's own ids. `crates/scale` implements §27's scale half while citing section 35;
//! `crates/registry`, `crates/hub` and `crates/hubapi` implement its hub half while citing sections
//! 10 and 34; `crates/mutation` and `crates/stress` implement its mutation half while citing
//! sections 03.08 and 32.
//!
//! That is fine, and the table below records it rather than complaining about it. What is not fine
//! is a reader inferring from a crate name that §27 is covered. This module is the machine-checked
//! answer to "which of the twenty-two are actually implemented, by what, and which are not".
//!
//! [`Owner::Unclaimed`] entries carry a reason. A gap that is stated is a limitation; one that is
//! implied to be filled is a lie, and §27 has enough surface for the difference to matter.

use serde::{Deserialize, Serialize};

/// What discharges a §27 module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "owner", rename_all = "snake_case")]
pub enum Owner {
    /// Implemented here, in the named module.
    ThisCrate { module: &'static str },
    /// Implemented by another crate, which cites a different blueprint section for the same
    /// content. `cites` records which, so a reader can find it.
    Sibling {
        crate_name: &'static str,
        cites: &'static str,
    },
    /// Nobody implements it, and this is why.
    Unclaimed { because: &'static str },
}

impl Owner {
    pub fn is_here(&self) -> bool {
        matches!(self, Owner::ThisCrate { .. })
    }

    pub fn is_unclaimed(&self) -> bool {
        matches!(self, Owner::Unclaimed { .. })
    }
}

/// One row of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleCoverage {
    pub id: &'static str,
    pub title: &'static str,
    pub owner: Owner,
}

/// The twenty-two modules of §27 and what discharges each.
pub fn coverage() -> [ModuleCoverage; 22] {
    use Owner::*;
    [
        ModuleCoverage {
            id: "27.01",
            title: "Parent BioWorld Authoring Program",
            owner: ThisCrate {
                module: "authoring",
            },
        },
        ModuleCoverage {
            id: "27.02",
            title: "Observed Worlds from Real Data and Workflows",
            owner: ThisCrate { module: "observed" },
        },
        ModuleCoverage {
            id: "27.03",
            title: "Semi-Synthetic Biological Worlds",
            owner: ThisCrate {
                module: "semisynthetic",
            },
        },
        ModuleCoverage {
            id: "27.04",
            title: "Mechanistic and Simulated BioWorlds",
            owner: ThisCrate {
                module: "mechanistic",
            },
        },
        ModuleCoverage {
            id: "27.05",
            title: "Prospective Escrow and Holdout Vault",
            owner: Sibling {
                crate_name: "bioprism-scale",
                cites: "35.05",
            },
        },
        ModuleCoverage {
            id: "27.06",
            title: "Trajectory Mining and BioDecision Compilation",
            owner: Unclaimed {
                because: "it turns real model, pipeline and agent executions into decision units, \
                          and there are no real executions in this workspace to mine",
            },
        },
        ModuleCoverage {
            id: "27.07",
            title: "BioMutator Engine",
            owner: Sibling {
                crate_name: "bioprism-mutation",
                cites: "03.08 and 32",
            },
        },
        ModuleCoverage {
            id: "27.08",
            title: "Semantics-Preserving Biological Mutations",
            owner: Sibling {
                crate_name: "bioprism-mutation",
                cites: "03.08 (Relation::PreservesVerdict)",
            },
        },
        ModuleCoverage {
            id: "27.09",
            title: "Controlled Biological Semantic Mutations",
            owner: Sibling {
                crate_name: "bioprism-stress",
                cites: "32 (StressRelation, the required-response half)",
            },
        },
        ModuleCoverage {
            id: "27.10",
            title: "Assay Fault, Batch, and Preanalytic Mutation Programs",
            owner: ThisCrate {
                module: "preanalytic",
            },
        },
        ModuleCoverage {
            id: "27.11",
            title: "Specimen Lineage, Mix-Up, and Identity Mutation Programs",
            owner: ThisCrate { module: "lineage" },
        },
        ModuleCoverage {
            id: "27.12",
            title: "Site, Batch, Platform, and Population Shift Programs",
            owner: Sibling {
                crate_name: "bioprism-stress",
                cites: "32 (prevalence shift, batch and site effects)",
            },
        },
        ModuleCoverage {
            id: "27.13",
            title: "Temporal, Censoring, and Exposure-History Mutation Programs",
            owner: Sibling {
                crate_name: "bioprism-mutation",
                cites: "32 (Mechanism::Temporal); the censoring half is bioprism-onco's estimands",
            },
        },
        ModuleCoverage {
            id: "27.14",
            title: "Multimodal Contradiction and Reconciliation Programs",
            owner: ThisCrate {
                module: "contradiction",
            },
        },
        ModuleCoverage {
            id: "27.15",
            title: "Deduplication, Diversity, and Effective Sample Size",
            owner: Sibling {
                crate_name: "bioprism-scale",
                cites: "35.10; also bioprism-mutation::diversity",
            },
        },
        ModuleCoverage {
            id: "27.16",
            title: "Million-Scale Registry Accounting and Release Policy",
            owner: Sibling {
                crate_name: "bioprism-scale",
                cites: "35.16 and 35.17; release gating in bioprism-registry",
            },
        },
        ModuleCoverage {
            id: "27.17",
            title: "BioPRISM Hub Domain Model",
            owner: Sibling {
                crate_name: "bioprism-registry",
                cites: "10.02 and 10.03",
            },
        },
        ModuleCoverage {
            id: "27.18",
            title: "Registry Trust Tiers, Review, and Promotion",
            owner: Sibling {
                crate_name: "bioprism-registry",
                cites: "27.18 and 10.07",
            },
        },
        ModuleCoverage {
            id: "27.19",
            title: "World, Pack, Assay, Oracle, and Result Cards",
            owner: Sibling {
                crate_name: "bioprism-hub",
                cites: "34.14 and 10.12",
            },
        },
        ModuleCoverage {
            id: "27.20",
            title: "Submission, Challenge, Appeal, and Reproduction Workflows",
            owner: Sibling {
                crate_name: "bioprism-hub",
                cites: "34.16 and 10.06",
            },
        },
        ModuleCoverage {
            id: "27.21",
            title: "Private, Controlled, and Federated Evaluation",
            owner: Sibling {
                crate_name: "bioprism-hubapi",
                cites: "10.18 and 10.19",
            },
        },
        ModuleCoverage {
            id: "27.22",
            title: "Hub APIs, SDK, Search, and Visualization",
            owner: Sibling {
                crate_name: "bioprism-hubapi",
                cites: "10.10 and 10.11; the visualisation half is a rendering nobody owns",
            },
        },
    ]
}

/// The modules this crate implements.
pub fn owned_here() -> Vec<ModuleCoverage> {
    coverage().into_iter().filter(|m| m.owner.is_here()).collect()
}

/// The modules nobody implements, with the reason.
pub fn unclaimed() -> Vec<ModuleCoverage> {
    coverage()
        .into_iter()
        .filter(|m| m.owner.is_unclaimed())
        .collect()
}
