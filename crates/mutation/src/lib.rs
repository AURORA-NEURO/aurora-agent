//! Metamorphic mutation with executable postconditions.
//!
//! Implements blueprint 03.08 (Mutation IR), 32 (the biological mutation and stress program) and
//! Gate 3 of the critical path.
//!
//! The flywheel this closes: an audited parent world becomes a family of validated instances, each
//! carrying the metamorphic relation it satisfies, ready to be promoted into a regression pack.
//! Two constraints keep it honest — a mutation is admitted only when the oracle confirms its
//! declared relation, and the family reports **effective diversity** rather than instance count,
//! because a million paraphrases are not a million benchmarks.

pub mod apply;
pub mod diversity;
pub mod error;
pub mod federated_publication_release_copilot;
pub mod federated_continual_bounded_evolution_assurance;
pub mod lineage;
pub mod knowledge_representation_federated_control_plane;
pub mod federated_resource_discovery_control_plane;

pub mod relation;
pub mod evolution_integrity_support;
pub mod local_evolution_integrity_inference;
pub mod multimodal_evolution_integrity_inference;
pub mod throughput_evolution_integrity_inference;
pub mod federated_continual_evolution_integrity_inference;
pub mod local_evolution_integrity_contract_model;
pub mod multimodal_evolution_integrity_contract_model;
pub mod throughput_evolution_integrity_contract_model;
pub mod federated_continual_evolution_integrity_contract_model;
pub mod local_evolution_integrity_research_copilot;
pub mod multimodal_evolution_integrity_research_copilot;
pub mod throughput_evolution_integrity_research_copilot;
pub mod federated_continual_evolution_integrity_research_copilot;
pub mod local_evolution_integrity_workflow_fabric;
pub mod multimodal_evolution_integrity_workflow_fabric;
pub mod throughput_evolution_integrity_workflow_fabric;
pub mod federated_continual_evolution_integrity_workflow_fabric;

pub use apply::{apply, ApplyError, Mutation, MutationKind};
pub use diversity::{measure, Diversity};
pub use error::{MutationError, RejectionReason};
pub use federated_publication_release_copilot::{
    compile_mutation_publication_release, mutation_publication_release_manifest,
    MutationPublicationReleaseReceipt9, PublicationReleaseDisposition,
    PublicationReleaseError, PublicationReleaseRequest6, ValidatedResearchRun4,
    CONTENT_TYPE as MUTATION_PUBLICATION_CONTENT_TYPE,
    CONTRACT_VERSION as MUTATION_PUBLICATION_CONTRACT_VERSION,
    FEATURE_ID as MUTATION_PUBLICATION_FEATURE_ID,
    INPUT_SCHEMA as MUTATION_PUBLICATION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MUTATION_PUBLICATION_OUTPUT_SCHEMA,
};
pub use federated_continual_bounded_evolution_assurance::{
    assure_mutation_federated_bounded_evolution,
    mutation_federated_bounded_evolution_manifest,
    MutationEvolutionEvidenceState, MutationEvolutionProposal7,
    MutationEvolutionRequest8, MutationEvolutionReceipt10,
    MutationEvolutionReceipt10Artifact, MutationFederatedEvolutionError,
    CONTENT_TYPE as MUTATION_FEDERATED_EVOLUTION_CONTENT_TYPE,
    CONTRACT_VERSION as MUTATION_FEDERATED_EVOLUTION_CONTRACT_VERSION,
    FEATURE_ID as MUTATION_FEDERATED_EVOLUTION_FEATURE_ID,
    INPUT_SCHEMA as MUTATION_FEDERATED_EVOLUTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MUTATION_FEDERATED_EVOLUTION_OUTPUT_SCHEMA,
};
pub use lineage::{generate, verdict_of, Family, Instance, Rejection};
pub use relation::{Mechanism, PostconditionResult, Relation};
pub use evolution_integrity_support::*;
pub use local_evolution_integrity_inference::*;
pub use multimodal_evolution_integrity_inference::*;
pub use throughput_evolution_integrity_inference::*;
pub use federated_continual_evolution_integrity_inference::*;
pub use local_evolution_integrity_contract_model::*;
pub use multimodal_evolution_integrity_contract_model::*;
pub use throughput_evolution_integrity_contract_model::*;
pub use federated_continual_evolution_integrity_contract_model::*;
pub use local_evolution_integrity_research_copilot::*;
pub use multimodal_evolution_integrity_research_copilot::*;
pub use throughput_evolution_integrity_research_copilot::*;
pub use federated_continual_evolution_integrity_research_copilot::*;
pub use local_evolution_integrity_workflow_fabric::*;
pub use multimodal_evolution_integrity_workflow_fabric::*;
pub use throughput_evolution_integrity_workflow_fabric::*;
pub use federated_continual_evolution_integrity_workflow_fabric::*;
pub use knowledge_representation_federated_control_plane::{
    mutation_knowledge_federated_control_manifest,
    operate_mutation_knowledge_federated_control,
    MutationKnowledgeCandidate, MutationKnowledgeDecision,
    MutationKnowledgeFederatedControlError, MutationKnowledgeFederatedControlRequest,
    MutationKnowledgeFederatedReceipt,
    FEATURE_ID as MUTATION_KNOWLEDGE_FEDERATED_CONTROL_FEATURE_ID,
    FEATURE_VERSION as MUTATION_KNOWLEDGE_FEDERATED_CONTROL_VERSION,
};
pub use federated_resource_discovery_control_plane::{
    operate_mutation_federated_resource_discovery,
    operate_mutation_federated_resource_discovery_json,
    validate_mutation_federated_resource_discovery_json,
    mutation_federated_resource_discovery_manifest,
    EndpointStatus, MutationPeerResourceSummary4, MutationResourceDiscoveryError,
    MutationResourceEndpoint4, MutationResourceNeed4, QualifiedResource8,
    QualifiedResourceSet8, ResourceEvidenceState,
    CONTENT_TYPE as MUTATION_RESOURCE_DISCOVERY_CONTENT_TYPE,
    CONTRACT_VERSION as MUTATION_RESOURCE_DISCOVERY_CONTRACT_VERSION,
    FEATURE_ID as MUTATION_RESOURCE_DISCOVERY_FEATURE_ID,
    INPUT_SCHEMA as MUTATION_RESOURCE_DISCOVERY_INPUT_SCHEMA,
    OUTPUT_SCHEMA as MUTATION_RESOURCE_DISCOVERY_OUTPUT_SCHEMA,
};

/// A standard suite covering every relation kind.
///
/// Eight relations against the MVP's requirement of five, chosen so each *kind* of postcondition
/// is exercised: four invariances, and one removal per leakage mechanism.
pub fn standard_suite() -> Vec<Mutation> {
    let mut suite = vec![
        Mutation::new(
            "rename-subjects",
            MutationKind::RenameSubjects {
                prefix: "X".into(),
            },
        ),
        Mutation::new("reorder-facts", MutationKind::ReorderFacts { seed: 7 }),
        Mutation::new(
            "add-distractors",
            MutationKind::AddDistractors {
                count: 25,
                seed: 11,
            },
        ),
        Mutation::new("camouflage-tags", MutationKind::CamouflageTags),
    ];
    for mechanism in Mechanism::ALL {
        suite.push(Mutation::new(
            format!("remove-{}", mechanism.as_str()),
            MutationKind::RemoveLeakage { mechanism },
        ));
    }
    suite
}
