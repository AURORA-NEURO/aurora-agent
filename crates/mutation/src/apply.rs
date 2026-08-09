//! The transformations.
//!
//! Each mutation edits the world document and declares the metamorphic relation it should satisfy.
//! Applying a mutation never checks that relation — that is [`crate::lineage`]'s job, deliberately
//! separated so the transformation cannot mark its own homework.

use crate::relation::{Mechanism, Relation};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MutationKind {
    /// Bijective relabelling of subject identifiers. Nothing about the split changes.
    RenameSubjects { prefix: String },
    /// Permute document order. Tests that the compiler does not depend on it.
    ReorderFacts { seed: u64 },
    /// Append irrelevant evidence attached to the identity hub.
    AddDistractors { count: usize, seed: u64 },
    /// Retag existing distractors so their tags tokenise into the protected vocabulary.
    CamouflageTags,
    /// Repair one leakage mechanism.
    RemoveLeakage { mechanism: Mechanism },
    /// Introduce one leakage mechanism.
    InjectLeakage { mechanism: Mechanism },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mutation {
    pub id: String,
    pub kind: MutationKind,
    pub relation: Relation,
}

impl Mutation {
    pub fn new(id: impl Into<String>, kind: MutationKind) -> Self {
        let relation = match &kind {
            MutationKind::RenameSubjects { .. }
            | MutationKind::ReorderFacts { .. }
            | MutationKind::AddDistractors { .. }
            | MutationKind::CamouflageTags => Relation::PreservesVerdict,
            MutationKind::RemoveLeakage { mechanism } => Relation::RemovesWitness {
                kind: mechanism.witness_kind().to_string(),
            },
            MutationKind::InjectLeakage { mechanism } => Relation::AddsWitness {
                kind: mechanism.witness_kind().to_string(),
            },
        };
        Mutation {
            id: id.into(),
            kind,
            relation,
        }
    }

    pub fn mechanism(&self) -> Option<Mechanism> {
        match &self.kind {
            MutationKind::RemoveLeakage { mechanism } | MutationKind::InjectLeakage { mechanism } => {
                Some(*mechanism)
            }
            _ => None,
        }
    }

    /// A short label for the lineage graph.
    pub fn family(&self) -> String {
        match &self.kind {
            MutationKind::RenameSubjects { .. } => "rename".into(),
            MutationKind::ReorderFacts { .. } => "reorder".into(),
            MutationKind::AddDistractors { .. } => "distractors".into(),
            MutationKind::CamouflageTags => "camouflage".into(),
            MutationKind::RemoveLeakage { mechanism } => format!("remove-{}", mechanism.as_str()),
            MutationKind::InjectLeakage { mechanism } => format!("inject-{}", mechanism.as_str()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error("world is malformed: {0}")]
    Malformed(&'static str),

    #[error("mutation is not applicable: {0}")]
    NotApplicable(String),
}

fn facts_mut(world: &mut Value) -> Result<&mut Vec<Value>, ApplyError> {
    world
        .get_mut("facts")
        .and_then(Value::as_array_mut)
        .ok_or(ApplyError::Malformed("missing facts array"))
}

fn fact_value_mut<'a>(world: &'a mut Value, id: &str) -> Option<&'a mut Value> {
    world
        .get_mut("facts")?
        .as_array_mut()?
        .iter_mut()
        .find(|fact| fact.get("id").and_then(Value::as_str) == Some(id))?
        .get_mut("value")
}

fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

pub fn apply(world: &Value, mutation: &Mutation) -> Result<Value, ApplyError> {
    let mut mutated = world.clone();
    match &mutation.kind {
        MutationKind::RenameSubjects { prefix } => rename_subjects(&mut mutated, prefix)?,
        MutationKind::ReorderFacts { seed } => reorder_facts(&mut mutated, *seed)?,
        MutationKind::AddDistractors { count, seed } => {
            add_distractors(&mut mutated, *count, *seed)?
        }
        MutationKind::CamouflageTags => camouflage_tags(&mut mutated)?,
        MutationKind::RemoveLeakage { mechanism } => set_leakage(&mut mutated, *mechanism, false)?,
        MutationKind::InjectLeakage { mechanism } => set_leakage(&mut mutated, *mechanism, true)?,
    }

    if let Some(id) = mutated.get("world_id").and_then(Value::as_str) {
        let renamed = format!("{id}+{}", mutation.id);
        mutated["world_id"] = json!(renamed);
    }
    Ok(mutated)
}

/// Relabels subject identifiers consistently across every map key and string value.
///
/// Aliases that are not subject identifiers — `ALT-77` and the like — are left alone, which is
/// what preserves the identity-leakage witness: the shared alias still binds two subjects.
fn rename_subjects(world: &mut Value, prefix: &str) -> Result<(), ApplyError> {
    fn rename_token(token: &str, prefix: &str) -> Option<String> {
        token
            .strip_prefix('S')
            .filter(|rest| rest.len() == 3 && rest.chars().all(|c| c.is_ascii_digit()))
            .map(|rest| format!("{prefix}{rest}"))
    }

    fn walk(value: &mut Value, prefix: &str) {
        match value {
            Value::String(text) => {
                if let Some(renamed) = rename_token(text, prefix) {
                    *text = renamed;
                }
            }
            Value::Array(items) => items.iter_mut().for_each(|item| walk(item, prefix)),
            Value::Object(map) => {
                let mut rebuilt = Map::new();
                for (key, mut child) in std::mem::take(map) {
                    walk(&mut child, prefix);
                    let key = rename_token(&key, prefix).unwrap_or(key);
                    rebuilt.insert(key, child);
                }
                *map = rebuilt;
            }
            _ => {}
        }
    }

    for fact in facts_mut(world)? {
        if let Some(value) = fact.get_mut("value") {
            walk(value, prefix);
        }
    }
    Ok(())
}

fn reorder_facts(world: &mut Value, seed: u64) -> Result<(), ApplyError> {
    let facts = facts_mut(world)?;
    let mut state = seed;
    // Fisher-Yates, seeded, so the permutation is reproducible from the mutation id.
    for index in (1..facts.len()).rev() {
        let swap = (splitmix(&mut state) % (index as u64 + 1)) as usize;
        facts.swap(index, swap);
    }
    Ok(())
}

fn add_distractors(world: &mut Value, count: usize, seed: u64) -> Result<(), ApplyError> {
    let mut state = seed;
    let existing = world
        .get("facts")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    let mut new_facts = Vec::new();
    let mut new_factors = Vec::new();
    for offset in 0..count {
        let index = existing + offset;
        let variable = format!("mutation_block_{index:06}");
        new_facts.push(json!({
            "id": format!("fact.mutation.{index:06}"),
            "provides": variable,
            "value": { "score": (splitmix(&mut state) % 1000) as f64 / 1000.0 },
            "scope": { "cohort": "RG-DEMO-001" },
            "tags": ["exploratory"],
            "provenance": ["mutation/added.json"],
        }));
        new_factors.push(json!({
            "id": format!("factor.mutation.{index:06}"),
            "inputs": ["cohort_id", variable],
            "outputs": [format!("{variable}_summary")],
            "kind": "exploratory_summary",
            "scope": { "cohort": "RG-DEMO-001" },
            "tags": ["exploratory"],
            "cost": 0.1,
        }));
    }

    facts_mut(world)?.extend(new_facts);
    world
        .get_mut("factors")
        .and_then(Value::as_array_mut)
        .ok_or(ApplyError::Malformed("missing factors array"))?
        .extend(new_factors);
    Ok(())
}

fn camouflage_tags(world: &mut Value) -> Result<(), ApplyError> {
    const CAMOUFLAGE: [&str; 4] = [
        "identity_summary",
        "split_summary",
        "site_summary",
        "specimen_summary",
    ];
    let mut index = 0usize;
    let mut touched = 0usize;

    for fact in facts_mut(world)? {
        let is_exploratory = fact
            .get("tags")
            .and_then(Value::as_array)
            .map(|tags| tags.iter().any(|tag| tag.as_str() == Some("exploratory")))
            .unwrap_or(false);
        if !is_exploratory {
            continue;
        }
        let replacement = CAMOUFLAGE[index % CAMOUFLAGE.len()];
        index += 1;
        touched += 1;
        fact["tags"] = json!([replacement, "exploratory"]);
    }

    if touched == 0 {
        return Err(ApplyError::NotApplicable(
            "world has no exploratory facts to camouflage".into(),
        ));
    }
    Ok(())
}

/// Turns one leakage mechanism on or off by editing the fact it depends on.
fn set_leakage(world: &mut Value, mechanism: Mechanism, present: bool) -> Result<(), ApplyError> {
    match mechanism {
        Mechanism::Preprocessing => {
            let value = fact_value_mut(world, "fact.preprocess_fit").ok_or_else(|| {
                ApplyError::NotApplicable("world has no fact.preprocess_fit".into())
            })?;
            *value = json!(if present {
                "all_subjects_before_split"
            } else {
                "train_only_after_split"
            });
        }
        Mechanism::Site => {
            let split = world
                .get("facts")
                .and_then(Value::as_array)
                .and_then(|facts| {
                    facts
                        .iter()
                        .find(|fact| fact.get("id").and_then(Value::as_str) == Some("fact.split"))
                })
                .and_then(|fact| fact.get("value"))
                .and_then(Value::as_object)
                .cloned()
                .ok_or_else(|| ApplyError::NotApplicable("world has no fact.split".into()))?;

            let value = fact_value_mut(world, "fact.site")
                .ok_or_else(|| ApplyError::NotApplicable("world has no fact.site".into()))?;
            let mut sites = Map::new();
            for (subject, arm) in &split {
                // Without leakage every subject shares one site; with it, the site is perfectly
                // predicted by the split arm.
                let site = if present && arm.as_str() != Some("train") {
                    "B"
                } else {
                    "A"
                };
                sites.insert(subject.clone(), json!(site));
            }
            *value = Value::Object(sites);
        }
        Mechanism::Temporal => {
            let value = fact_value_mut(world, "fact.label_source").ok_or_else(|| {
                ApplyError::NotApplicable("world has no fact.label_source".into())
            })?;
            let subjects: Vec<String> = value
                .as_object()
                .map(|map| map.keys().cloned().collect())
                .unwrap_or_default();
            let mut times = Map::new();
            for (index, subject) in subjects.iter().enumerate() {
                let recorded = if present && index + 1 == subjects.len() {
                    "2025-06-01"
                } else {
                    "2024-11-15"
                };
                times.insert(subject.clone(), json!(recorded));
            }
            *value = Value::Object(times);
        }
        Mechanism::Identity => {
            let value = fact_value_mut(world, "fact.subject_aliases").ok_or_else(|| {
                ApplyError::NotApplicable("world has no fact.subject_aliases".into())
            })?;
            let subjects: Vec<String> = value
                .as_object()
                .map(|map| map.keys().cloned().collect())
                .unwrap_or_default();
            let mut aliases = Map::new();
            for (index, subject) in subjects.iter().enumerate() {
                let mut names = vec![json!(subject)];
                let shared = index == 0 || index + 1 == subjects.len();
                if present && shared {
                    names.push(json!("ALT-77"));
                }
                aliases.insert(subject.clone(), Value::Array(names));
            }
            *value = Value::Object(aliases);
        }
    }
    Ok(())
}
