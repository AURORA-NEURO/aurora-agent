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
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const ORACLE_KIND: &str = "deterministic_split_integrity_v1";

pub fn evaluate(values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
    let mut witnesses = Vec::new();

    witnesses.extend(identity_witnesses(values)?);
    witnesses.extend(site_witnesses(values));
    witnesses.extend(temporal_witnesses(values)?);
    witnesses.extend(preprocessing_witnesses(values));

    Ok(OracleVerdict::new(ORACLE_KIND, witnesses))
}

fn object<'a>(values: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a Map<String, Value>> {
    values.get(key).and_then(Value::as_object)
}

/// One alias shared by subjects that were split apart.
///
/// The reverse index is built in the document order of `subject_aliases` so that the `subjects`
/// list in the witness matches the reference byte for byte, then iterated in sorted alias order.
fn identity_witnesses(
    values: &BTreeMap<String, Value>,
) -> Result<Vec<LeakageWitness>, FiberError> {
    let Some(aliases) = object(values, "subject_aliases") else {
        return Ok(Vec::new());
    };
    let split = object(values, "split_assignment");

    let mut reverse: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (subject, names) in aliases {
        let Some(names) = names.as_array() else { continue };
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
fn site_witnesses(values: &BTreeMap<String, Value>) -> Vec<LeakageWitness> {
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
        let Some(assigned) = assigned.as_str() else { continue };
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
fn temporal_witnesses(
    values: &BTreeMap<String, Value>,
) -> Result<Vec<LeakageWitness>, FiberError> {
    let Some(cut) = values.get("training_decision_time") else {
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

fn preprocessing_witnesses(values: &BTreeMap<String, Value>) -> Vec<LeakageWitness> {
    let fit_across_everything = values
        .get("preprocess_fit_scope")
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
