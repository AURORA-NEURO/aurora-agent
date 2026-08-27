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
pub mod lineage;
pub mod knowledge_representation_federated_control_plane;

pub mod relation;

pub use apply::{apply, ApplyError, Mutation, MutationKind};
pub use diversity::{measure, Diversity};
pub use error::{MutationError, RejectionReason};
pub use lineage::{generate, verdict_of, Family, Instance, Rejection};
pub use relation::{Mechanism, PostconditionResult, Relation};
pub use knowledge_representation_federated_control_plane::{
    mutation_knowledge_federated_control_manifest,
    operate_mutation_knowledge_federated_control,
    MutationKnowledgeCandidate, MutationKnowledgeDecision,
    MutationKnowledgeFederatedControlError, MutationKnowledgeFederatedControlRequest,
    MutationKnowledgeFederatedReceipt,
    FEATURE_ID as MUTATION_KNOWLEDGE_FEDERATED_CONTROL_FEATURE_ID,
    FEATURE_VERSION as MUTATION_KNOWLEDGE_FEDERATED_CONTROL_VERSION,
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
