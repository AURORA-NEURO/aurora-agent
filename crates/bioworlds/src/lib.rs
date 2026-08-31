//! Reference BioWorlds and vertical slices.
//!
//! Implements blueprint cluster `38_REFERENCE_BIOWORLDS_AND_VERTICAL_SLICES` — specifically 38.02
//! (longitudinal post-treatment uncertainty) and 38.08 (trial eligibility and the temporal
//! evidence firewall) — against the `fiber-world/0.1` schema `bioprism-world` accepts.
//!
//! # Why this crate is not another machinery crate
//!
//! The most consequential result this repository has produced came from building a *world*. The
//! shipped reference world cannot separate FIBER from a tuned 5-hop graph walk or a BM25
//! retriever — all three select the identical eleven facts — and that negative result is in
//! `docs/FINDINGS.md`. It took a generated world with different structure to produce a case where
//! FIBER is uniquely admissible. Worlds are where falsification happens, so the work here is
//! chosen by what it makes falsifiable, not by what §38 lists.
//!
//! `crates/examples` maintains the list that matters: platform claims it cannot demonstrate, each
//! with the obstacle written down. Two of those obstacles are world-shaped, and this crate is
//! aimed at them.
//!
//! ## `non_protected_temporal_withholding` — the world side is now built
//!
//! The recorded obstacle: *"in the generated family every event-managed variable is also
//! protected, so an early cut cannot withhold evidence without simultaneously breaking the
//! closure; separating them needs an event over a non-protected variable that the target depends
//! on."* [`temporal`] builds exactly that. `central_lab_confirmation` is reachable from the
//! target, governed by an event released after the screening cut, and carries no protected tag;
//! every protected variable in the world stays readable at that same cut. The structural property
//! is asserted in `tests/non_protected_temporal_withholding.rs`.
//!
//! ## `underdetermined_abstention` — the input exists; the default compile still calls it valid
//!
//! That blocker names `bioprism-fiber`, which this crate cannot touch and does not depend on.
//! What it *can* supply is a world that genuinely underdetermines: [`underdetermined`] leaves
//! three mutually exclusive hypotheses live over an evidence set that is complete, fully readable
//! at the cut, and records its absences explicitly rather than omitting them. `bioprism-fiber`
//! has since gained `compile_with_oracle`, through which an injected oracle — `bioprism-domain`'s
//! rule oracles are the worked case — can return an abstaining verdict; but the default
//! `compile()` still runs the split-integrity oracle, whose witness list is empty on this world,
//! so the default path still compiles it to `valid` — a *wrong* answer rather than a missing one.
//! [`underdetermined::AbstentionStep`] enumerates the six things a judge of *this* world would
//! still have to do; no oracle yet interprets its hypothesis factors.
//!
//! # The line this crate does not cross
//!
//! `bioprism-fiber` is absent from the dependency set on purpose. Nothing here compiles a query,
//! runs an oracle or produces a verdict, so nothing here can claim a world *discriminates* — only
//! that it has the structure discrimination would need. Every measurement in [`structure`] is
//! defined, and the neighbourhood metric is calibrated against the published depths in
//! `docs/FINDINGS.md` (`tests/reference_world_calibration.rs`) so it is anchored to a number
//! somebody else measured rather than to one this crate invented.
//!
//! Two of the four shipped slices are **controls that come out unfavourably**: they reproduce the
//! reference world's structural corner and therefore have a separating depth and no tag
//! camouflage. They ship with that finding attached.
//!
//! # Layout
//!
//! | module | what it is |
//! |---|---|
//! | [`builder`] | the writer that emits `fiber-world/0.1` and hands it to `World::from_json` |
//! | [`query`] | the query *shape* a slice pairs with a world; serialisable, never compiled here |
//! | [`structure`] | separating depth, protected/withheld splits, elimination width, camouflage |
//! | [`fixtures`] | the shipped worlds as committed bytes, for consumers without this builder |
//! | [`temporal`] | §38.08, the non-protected temporal withholding world and its control |
//! | [`underdetermined`] | §38.02, the underdetermining world and its resolved control |
//! | [`slice`] | a vertical slice as a value, with checkable structural predicates |
//! | [`catalog`] | the four shipped slices and the report over them |
//! | [`knobs`] | structural knobs `bioprism-worldgen` does not have, as a change proposal |
//!
//! # Determinism
//!
//! Every world is a pure function of its spec, including the seed. No clock is read, no system RNG
//! is used, and the RNG is `bioprism-worldgen`'s own `SplitMix64`. `fixtures/` holds each world as
//! it is built, and `tests/fixtures_are_current.rs` fails if a rebuild disagrees byte for byte.
//!
//! # Reading a report back
//!
//! [`SliceReport`] and [`CatalogReport`], and every struct they are made of, refuse a field they
//! do not declare. Both seal themselves by re-serialising the parsed struct and hashing that, so a
//! key the reader dropped would be outside the seal by construction: the recomputation never sees
//! it, the claimed digest still agrees, and a report carrying content nobody hashed reads as
//! intact.
//!
//! # Not implemented
//!
//! * **No compiled verdicts.** See above. This is the load-bearing limitation.
//! * **No Oracle Mesh, no latent state, no resource budget, no mutation programs.** §38's world
//!   manifest names all of them and `fiber-world/0.1` carries none; inventing wire fields would
//!   produce documents nothing else in the workspace reads.
//! * **Points 3-9 of §38's vertical-slice acceptance list** — sandboxed actions, architecture
//!   forks, oracle scoring, PRISM minimisation, mutation validation, signed bundles, CI replay.
//!   Each needs a crate outside this dependency set.
//! * **Fourteen of §38's sixteen worlds.** Two are built, chosen for the blocked properties they
//!   close rather than by list order.
//! * **Exact treewidth.** [`structure::StructuralProfile::elimination_width`] is a greedy
//!   min-degree upper bound and is named for what it is.

pub mod builder;
pub mod catalog;
pub mod error;
pub mod evaluation_assurance;
pub mod evidence_workbench;
pub mod federated_continual_context_research_workbench;
pub mod federated_ingestion;
pub mod fixtures;
pub mod knobs;
pub mod query;
pub mod slice;
pub mod structure;
pub mod temporal;
pub mod underdetermined;

pub use builder::{BioWorld, WorldBuilder, WORLD_SCHEMA_VERSION};
pub use catalog::{CatalogReport, SliceCatalog};
pub use error::BioWorldError;
pub use evaluation_assurance::{
    assure_evaluation_observability, EvaluationAssuranceError, EvaluationAssuranceReceipt,
    EvaluationAssuranceRequest, EvaluationDisposition, EvaluationObservation, EvaluationSummary,
    MetricOutcome, ObservationState, CONTRACT_VERSION as EVALUATION_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as EVALUATION_ASSURANCE_FEATURE_ID,
    PRECLINICAL_BOUNDARY as EVALUATION_ASSURANCE_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as EVALUATION_ASSURANCE_SCHEMA_VERSION,
};
pub use evidence_workbench::{
    operate_evidence_workbench, EvidenceDisposition, EvidenceFeed, EvidenceSource,
    EvidenceState as WorkbenchEvidenceState, EvidenceWorkbenchError, EvidenceWorkbenchReceipt,
    Freshness, QualifiedEvidenceSet, CONTRACT_VERSION as EVIDENCE_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as EVIDENCE_WORKBENCH_FEATURE_ID,
    PRECLINICAL_BOUNDARY as EVIDENCE_WORKBENCH_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as EVIDENCE_WORKBENCH_SCHEMA_VERSION,
};
pub use federated_continual_context_research_workbench::{
    compile_federated_continual_context_workbench,
    federated_continual_context_research_workbench_manifest, FederatedContextWorkbenchError,
    FederatedContextWorkbenchPeer, FederatedContextWorkbenchReceipt,
    FederatedContextWorkbenchRequest, FederatedContextWorkbenchVerdict,
    CONTENT_TYPE as FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_CONTEXT_RESEARCH_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_CONTEXT_RESEARCH_WORKBENCH_FEATURE_ID,
};
pub use federated_ingestion::{
    operate_federated_ingestion, FederatedIngestionError, FederatedIngestionReceipt,
    HarmonizedResearchObject as FederatedHarmonizedResearchObject, IngestionDisposition,
    ModalityArtifact, ModalityState, RawModalityBundle as FederatedRawModalityBundle,
    CONTRACT_VERSION as FEDERATED_INGESTION_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_INGESTION_FEATURE_ID,
    PRECLINICAL_BOUNDARY as FEDERATED_INGESTION_PRECLINICAL_BOUNDARY,
    SCHEMA_VERSION as FEDERATED_INGESTION_SCHEMA_VERSION,
};
pub use knobs::{MissingGeneratorKnob, REUSED_GENERATOR_KNOBS};
pub use query::{QueryShape, QUERY_SCHEMA_VERSION};
pub use slice::{BlockedProperty, CheckOutcome, SliceReport, StructuralCheck, VerticalSlice};
pub use structure::{profile, DependencyClosure, Neighbourhood, StructuralProfile, TemporalSplit};
pub use underdetermined::{AbstentionStep, UnderdeterminationProfile};

/// The blueprint modules this crate implements.
pub const IMPLEMENTED_BLUEPRINT_MODULES: [&str; 2] = ["38.02", "38.08"];

/// The §38 worlds this crate does not build.
///
/// Named individually rather than as a count. A missing capability that is stated is a limitation;
/// one that is implied to exist is a lie, and fourteen unbuilt worlds behind the phrase "reference
/// bioworlds" would be the second thing.
pub const UNBUILT_BLUEPRINT_WORLDS: [&str; 14] = [
    "38.01 radiogenomic site leakage",
    "38.03 single-cell batch confounding and rare state",
    "38.04 methylation-histology discordance",
    "38.05 sample swap and multimodal lineage",
    "38.06 CRISPR dependency and copy-number artifact",
    "38.07 BBB exposure and target engagement",
    "38.09 scientific figure reproduction and claim trace",
    "38.10 OncoWeave multimodal research board",
    "38.11 pediatric DMG target translation",
    "38.12 pediatric LGG longitudinal growth",
    "38.13 extent of resection causal inference",
    "38.14 spatial tumor microenvironment hypothesis",
    "38.15 federated external site validation",
    "38.16 prospective blind reveal discovery",
];
pub mod knowledge_workflow_fabric;
pub mod resource_discovery_copilot;
pub use knowledge_workflow_fabric::{
    compile_knowledge_workflow, knowledge_workflow_manifest, KnowledgeObservation6,
    KnowledgeWorkflowDisposition, KnowledgeWorkflowError, KnowledgeWorkflowReceipt7,
    KnowledgeWorkflowRequest5, CONTRACT_VERSION as KNOWLEDGE_WORKFLOW_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_WORKFLOW_FEATURE_ID,
};
pub use resource_discovery_copilot::{
    qualify_resources, resource_discovery_manifest, QualifiedResourceSet6, ResourceCandidate6,
    ResourceDiscoveryDisposition, ResourceDiscoveryError, ResourceNeed5,
    CONTRACT_VERSION as RESOURCE_DISCOVERY_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as RESOURCE_DISCOVERY_COPILOT_FEATURE_ID,
};
