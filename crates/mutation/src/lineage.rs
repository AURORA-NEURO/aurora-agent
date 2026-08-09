//! Generation, validation, deduplication and lineage.
//!
//! Blueprint 32 and Gate 3 of the critical path. An instance is admitted only when its declared
//! metamorphic relation actually held under the oracle — the mutation does not get to assert its
//! own validity — and only when it is not a byte-duplicate of something already generated.
//!
//! Both counts are reported. A generator that silently drops failures and duplicates produces an
//! impressive instance count and an unfalsifiable one.

use crate::apply::{apply, Mutation};
use crate::error::{MutationError, RejectionReason};
use crate::relation::PostconditionResult;
use bioprism_fiber::oracle;
use bioprism_ids::ContentHash;
use bioprism_section::OracleVerdict;
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// The name a world calls itself, for an error a caller can act on.
///
/// A world document is not required to carry a `world_id`, so this falls back rather than failing:
/// a missing label must not turn the failure a caller needs to see into a different failure about
/// the label.
pub(crate) fn label(world: &Value) -> String {
    world
        .get("world_id")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string()
}

/// Digest of a world's *semantic content*, ignoring its label.
///
/// `world_id` and `description` are metadata the generator assigns; two worlds with identical
/// facts, factors and events are the same benchmark however they are named. Hashing the whole
/// document would let a generator defeat deduplication simply by renaming, which is exactly the
/// instance-count inflation the diversity accounting exists to prevent.
pub fn content_digest(world: &Value) -> Result<String, MutationError> {
    let content = serde_json::json!({
        "facts": world.get("facts").cloned().unwrap_or(Value::Null),
        "factors": world.get("factors").cloned().unwrap_or(Value::Null),
        "events": world.get("events").cloned().unwrap_or(Value::Null),
    });
    ContentHash::of_value(&content)
        .map(|digest| digest.as_str().to_string())
        .map_err(|source| MutationError::NotAddressable {
            world_id: label(world),
            source,
        })
}

/// Evaluates the oracle over every fact in a world.
///
/// Deliberately not the compiled selection: validating a mutation must not depend on the context
/// policy, or the family would only be valid for whichever compiler generated it.
pub fn verdict_of(world: &Value) -> Result<OracleVerdict, MutationError> {
    let world_id = label(world);
    let parsed = World::from_json(world.clone()).map_err(|source| MutationError::DoesNotLoad {
        world_id: world_id.clone(),
        source,
    })?;
    let values: BTreeMap<String, Value> = parsed
        .facts
        .iter()
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values).map_err(|source| MutationError::NotEvaluable { world_id, source })
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub id: String,
    pub parent_id: String,
    pub mutation_id: String,
    pub family: String,
    /// Digest of the world's semantic content, excluding its label.
    pub world_sha256: String,
    pub status: String,
    pub witnesses: Vec<String>,
}

impl Instance {
    /// The oracle signature: what this instance actually tests.
    pub fn signature(&self) -> String {
        format!("{}|{}", self.status, self.witnesses.join(","))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rejection {
    pub mutation_id: String,
    pub family: String,
    pub reason: RejectionReason,
}

#[derive(Debug, Clone, Default)]
pub struct Family {
    pub parent_id: String,
    pub parent_sha256: String,
    pub accepted: Vec<Instance>,
    /// Instances whose declared relation did not hold, with what the oracle actually did.
    pub rejected: Vec<Rejection>,
    /// Mutations that produced a world byte-identical to one already generated.
    pub duplicates: Vec<String>,
    /// The generated worlds, keyed by instance id, for callers that want to persist them.
    pub worlds: BTreeMap<String, Value>,
}

impl Family {
    pub fn accepted_count(&self) -> usize {
        self.accepted.len()
    }

    /// Fraction of attempted mutations that survived validation.
    pub fn yield_rate(&self) -> f64 {
        let attempted = self.accepted.len() + self.rejected.len() + self.duplicates.len();
        if attempted == 0 {
            return 0.0;
        }
        self.accepted.len() as f64 / attempted as f64
    }
}

fn rejected(mutation: &Mutation, reason: RejectionReason) -> Rejection {
    Rejection {
        mutation_id: mutation.id.clone(),
        family: mutation.family(),
        reason,
    }
}

/// Applies each mutation to `parent`, validating and deduplicating.
///
/// Two classes of failure abort the run: anything wrong with the parent, and any world at all
/// that cannot be content-addressed. Everything else a single mutation can do wrong is recorded in
/// [`Family::rejected`] and generation continues, so one bad operator cannot cost a caller the
/// rest of the family — which is also why the return type carries only the first two.
pub fn generate(parent: &Value, mutations: &[Mutation]) -> Result<Family, MutationError> {
    let parent_verdict = verdict_of(parent)?;
    let parent_id = label(parent);
    let parent_sha256 = content_digest(parent)?;

    let mut family = Family {
        parent_id: parent_id.clone(),
        parent_sha256: parent_sha256.clone(),
        ..Default::default()
    };
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(parent_sha256);

    for mutation in mutations {
        let mutated = match apply(parent, mutation) {
            Ok(world) => world,
            Err(error) => {
                family
                    .rejected
                    .push(rejected(mutation, RejectionReason::from_apply(&error)));
                continue;
            }
        };

        let digest = content_digest(&mutated)?;
        if !seen.insert(digest.clone()) {
            family.duplicates.push(mutation.id.clone());
            continue;
        }

        // A descendant with no content digest is the same fatal case as a parent with none:
        // nothing downstream can deduplicate against it, so it aborts rather than being recorded
        // as one more rejection among worlds that were properly checked.
        let verdict = match verdict_of(&mutated) {
            Ok(verdict) => verdict,
            Err(MutationError::DoesNotLoad { source, .. }) => {
                family.rejected.push(rejected(
                    mutation,
                    RejectionReason::DescendantDoesNotLoad {
                        detail: source.to_string(),
                    },
                ));
                continue;
            }
            Err(MutationError::NotEvaluable { source, .. }) => {
                family.rejected.push(rejected(
                    mutation,
                    RejectionReason::DescendantNotEvaluable {
                        detail: source.to_string(),
                    },
                ));
                continue;
            }
            Err(fatal @ MutationError::NotAddressable { .. }) => return Err(fatal),
        };

        match mutation.relation.check(&parent_verdict, &verdict) {
            PostconditionResult::Held => {
                let id = format!("{}#{}", parent_id, mutation.id);
                family.accepted.push(Instance {
                    id: id.clone(),
                    parent_id: parent_id.clone(),
                    mutation_id: mutation.id.clone(),
                    family: mutation.family(),
                    world_sha256: digest,
                    status: verdict.status.as_str().to_string(),
                    witnesses: verdict
                        .witness_kinds()
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                });
                family.worlds.insert(id, mutated);
            }
            PostconditionResult::Violated { expected, observed } => {
                family.rejected.push(rejected(
                    mutation,
                    RejectionReason::PostconditionViolated { expected, observed },
                ));
            }
        }
    }

    Ok(family)
}
