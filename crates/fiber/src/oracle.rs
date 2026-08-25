//! The deterministic split-integrity oracle.
//!
//! Blueprint 43.41 requires the first vertical slice to be judged by an *exact* oracle returning
//! leakage witnesses, not by a model's opinion. Four mechanisms are checked, each producing a
//! witness a human can verify by hand:
//!
//! * identity — one alias resolving to subjects that landed in different splits;
//! * site — each split drawing from exactly one, differing, site;
//! * temporal — a label derived from evidence that postdates the training cut;
//! * preprocessing — a transform fit across all subjects before the split was drawn.
//!
//! Two behaviours here are bug-compatible with the CPython reference and flagged as such:
//! temporal comparison is lexicographic on the raw strings rather than on parsed instants, and
//! a missing split assignment among aliased subjects is an error rather than a silent skip.

use crate::error::FiberError;
use bioprism_section::{LeakageWitness, OracleVerdict};
use bioprism_world::{Fact, World};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// A value map the oracle can read without owning it.
///
/// Every check below reaches the map through [`object`] or a single `get`, so the oracle never
/// needs a `Value` it can keep. Borrowing is what lets [`evaluate_facts`] judge a selection
/// without deep-cloning the selected evidence, which the minimizer does once per removal.
type ValueRefs<'a> = BTreeMap<&'a str, &'a Value>;

/// The six variables the reference oracle reads.
///
/// This list is an optimisation, not a contract: [`evaluate_facts`] uses it to skip facts whose
/// values no check can consult. Narrowing is sound only because [`evaluate_refs`] never iterates
/// the map — it looks up these six keys and nothing else — so a dropped entry is unobservable.
///
/// [`evaluate`] deliberately does *not* narrow. Keeping the general entry point reading the whole
/// map is what makes the two paths distinguishable, so the test
/// `narrowing_to_the_variables_the_oracle_reads_preserves_the_verdict` compares a full map against
/// a narrowed one and fails the moment a check learns to read a seventh variable that is not
/// listed here.
const INPUT_VARIABLES: [&str; 6] = [
    "label_source_time",
    "preprocess_fit_scope",
    "site_assignment",
    "split_assignment",
    "subject_aliases",
    "training_decision_time",
];

pub const ORACLE_KIND: &str = "deterministic_split_integrity_v1";

/// A deterministic oracle the compiler can run over the selected value map.
///
/// Blueprint 43.41's contract, held open: an implementation must be a pure function of the
/// value map, must return witnesses a human can check by hand rather than scores, and must
/// abstain ([`bioprism_section::OracleStatus::Underdetermined`]) rather than answer when it
/// cannot see the evidence its decision needs. The compiler treats the verdict as data;
/// nothing downstream branches on the concrete oracle type, so a domain oracle changes the
/// certificate's bytes only through the verdict it returns — which is the point.
pub trait DecisionOracle {
    /// The oracle kind recorded on every verdict this oracle produces.
    fn kind(&self) -> &str;
    fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError>;
}

/// The reference split-integrity oracle, as a [`DecisionOracle`].
///
/// [`crate::compile::compile`] uses this implementation unconditionally, which is what keeps
/// the reference certificate byte-identical across the three parity implementations. A caller
/// with a different decision question supplies its own oracle to
/// [`crate::compile::compile_with_oracle`] instead.
pub struct SplitIntegrityOracle;

impl DecisionOracle for SplitIntegrityOracle {
    fn kind(&self) -> &str {
        ORACLE_KIND
    }

    fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
        evaluate(values)
    }
}

pub fn evaluate(values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
    evaluate_refs(
        &values
            .iter()
            .map(|(key, value)| (key.as_str(), value))
            .collect(),
    )
}

/// Runs the reference oracle over facts directly, without building an owned value map.
///
/// The callers that judge a *selection* — the baseline panel, both prism passes, the mutation
/// lineage and the example walks — all reached the oracle by cloning every selected fact's value
/// into a fresh map. Nothing consumed those clones: the verdict borrows nothing from the map, and
/// the map died on the next line. Under `prism::minimize` that cost was quadratic, because
/// delta-debugging rebuilds the map once per attempted removal.
///
/// Where two facts provide the same variable the last one wins, matching what `collect` into a
/// map did before, so the caller's iteration order still decides.
pub fn evaluate_facts<'a>(
    facts: impl IntoIterator<Item = &'a Fact>,
) -> Result<OracleVerdict, FiberError> {
    let values: ValueRefs<'a> = facts
        .into_iter()
        .filter(|fact| INPUT_VARIABLES.contains(&fact.provides.as_str()))
        .map(|fact| (fact.provides.as_str(), &fact.value))
        .collect();
    evaluate_refs(&values)
}

/// Runs the reference oracle over the facts a selection names.
///
/// The baseline panel and both prism passes each resolved a set of fact ids against the world and
/// built the identical map to ask the identical question. Ids the world does not know are skipped,
/// exactly as before: a selection naming a fact that is not there is judged on what it does name,
/// because the alternative would turn a strategy's bookkeeping slip into an oracle refusal.
pub fn evaluate_selected(
    world: &World,
    facts: &BTreeSet<String>,
) -> Result<OracleVerdict, FiberError> {
    evaluate_facts(facts.iter().filter_map(|id| world.fact(id)))
}

fn evaluate_refs(values: &ValueRefs<'_>) -> Result<OracleVerdict, FiberError> {
    let mut witnesses = Vec::new();

    witnesses.extend(identity_witnesses(values)?);
    witnesses.extend(site_witnesses(values));
    witnesses.extend(temporal_witnesses(values)?);
    witnesses.extend(preprocessing_witnesses(values));

    Ok(OracleVerdict::new(ORACLE_KIND, witnesses))
}

fn object<'a>(values: &ValueRefs<'a>, key: &str) -> Option<&'a Map<String, Value>> {
    values.get(key).copied().and_then(Value::as_object)
}

/// One alias shared by subjects that were split apart.
///
/// The reverse index is built in the document order of `subject_aliases` so that the `subjects`
/// list in the witness matches the reference byte for byte, then iterated in sorted alias order.
fn identity_witnesses(values: &ValueRefs<'_>) -> Result<Vec<LeakageWitness>, FiberError> {
    let Some(aliases) = object(values, "subject_aliases") else {
        return Ok(Vec::new());
    };
    let split = object(values, "split_assignment");

    let mut reverse: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (subject, names) in aliases {
        let Some(names) = names.as_array() else {
            continue;
        };
        for name in names {
            let Some(name) = name.as_str() else { continue };
            reverse
                .entry(name.to_string())
                .or_default()
                .push(subject.clone());
        }
    }

    let mut witnesses = Vec::new();
    for (alias, subjects) in reverse {
        let mut groups: BTreeSet<Option<&str>> = BTreeSet::new();
        for subject in &subjects {
            groups.insert(
                split
                    .and_then(|s| s.get(subject.as_str()))
                    .and_then(Value::as_str),
            );
        }
        if subjects.len() <= 1 || groups.len() <= 1 {
            continue;
        }
        if groups.contains(&None) {
            let present: Vec<String> = groups
                .iter()
                .filter_map(|g| g.map(str::to_string))
                .collect();
            return Err(FiberError::UnorderableSplitGroups { alias, present });
        }
        witnesses.push(LeakageWitness::IdentityLeakage {
            alias,
            subjects,
            splits: groups.into_iter().flatten().map(str::to_string).collect(),
        });
    }
    Ok(witnesses)
}

/// Each split draws from exactly one site, and those sites differ.
fn site_witnesses(values: &ValueRefs<'_>) -> Vec<LeakageWitness> {
    let (Some(site), Some(split)) = (
        object(values, "site_assignment"),
        object(values, "split_assignment"),
    ) else {
        return Vec::new();
    };
    if site.is_empty() || split.is_empty() {
        return Vec::new();
    }

    let mut by_split: BTreeMap<String, BTreeSet<Option<&str>>> = BTreeMap::new();
    for (subject, assigned) in split {
        let Some(assigned) = assigned.as_str() else {
            continue;
        };
        by_split
            .entry(assigned.to_string())
            .or_default()
            .insert(site.get(subject.as_str()).and_then(Value::as_str));
    }

    let clean: BTreeMap<String, Vec<String>> = by_split
        .into_iter()
        .map(|(split_name, sites)| {
            (
                split_name,
                sites.into_iter().flatten().map(str::to_string).collect(),
            )
        })
        .collect();

    let every_split_has_one_site = clean.values().all(|sites| sites.len() == 1);
    let distinct: BTreeSet<&Vec<String>> = clean.values().collect();

    if clean.len() > 1 && every_split_has_one_site && distinct.len() > 1 {
        vec![LeakageWitness::SiteLeakage {
            site_by_split: clean,
        }]
    } else {
        Vec::new()
    }
}

/// A label derived from evidence that postdates the training decision time.
///
/// The comparison is lexicographic on the raw timestamp strings, matching the reference. For the
/// zero-offset `...Z` form used throughout the packs this agrees with instant ordering; for
/// mixed offsets or differing precision it does not, which is recorded as a known limitation on
/// every certificate rather than silently corrected here.
fn temporal_witnesses(values: &ValueRefs<'_>) -> Result<Vec<LeakageWitness>, FiberError> {
    let Some(cut) = values.get("training_decision_time").copied() else {
        return Ok(Vec::new());
    };
    let Some(cut) = cut.as_str() else {
        return Ok(Vec::new());
    };
    if cut.is_empty() {
        return Ok(Vec::new());
    }

    let Some(label_times) = object(values, "label_source_time") else {
        return Ok(Vec::new());
    };

    let mut future: BTreeMap<String, String> = BTreeMap::new();
    for (subject, recorded) in label_times {
        let recorded = recorded.as_str().ok_or(FiberError::WrongQueryFieldType {
            field: "label_source_time",
            expected: "object of timestamp strings",
        })?;
        if recorded > cut {
            future.insert(subject.clone(), recorded.to_string());
        }
    }

    if future.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(vec![LeakageWitness::TemporalLeakage {
            decision_time: cut.to_string(),
            future_label_sources: future,
        }])
    }
}

fn preprocessing_witnesses(values: &ValueRefs<'_>) -> Vec<LeakageWitness> {
    let fit_across_everything = values
        .get("preprocess_fit_scope")
        .copied()
        .and_then(Value::as_str)
        .is_some_and(|scope| scope == "all_subjects_before_split");

    if fit_across_everything {
        vec![LeakageWitness::PreprocessingLeakage {
            detail: "preprocessing fit used all subjects before split".into(),
        }]
    } else {
        Vec::new()
    }
}
