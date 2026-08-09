//! World and query generation.
//!
//! Emits `fiber-world/0.1` and `fiber-query/0.1` documents. Generation is a pure function of the
//! [`WorldSpec`], including its seed, so a spec is a reproducible reference to a world.
//!
//! The decisive skeleton is fixed — the same eleven facts and five checks the reference world
//! uses, so the deterministic oracle applies unchanged — and only the *structure around it* varies:
//! how far the decisive facts sit from the target, where distractors attach, and whether their
//! tags are lexically distinguishable.

use crate::rng::SplitMix64;
use crate::spec::{DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};
use serde_json::{json, Map, Value};

const CHECKS: [(&str, &str, &[&str]); 5] = [
    (
        "factor.identity_check",
        "identity_leakage",
        &[
            "cohort_id",
            "subject_aliases",
            "split_assignment",
            "declared_duplicate_screen",
        ],
    ),
    (
        "factor.site_check",
        "site_leakage",
        &["site_assignment", "scanner_assignment", "split_assignment"],
    ),
    (
        "factor.temporal_check",
        "temporal_leakage",
        &[
            "label_source_time",
            "training_decision_time",
            "specimen_dates",
            "split_assignment",
        ],
    ),
    (
        "factor.preprocessing_check",
        "preprocessing_leakage",
        &["preprocess_fit_scope", "split_assignment"],
    ),
    ("factor.policy_check", "policy_validity", &["data_policy"]),
];

const CAMOUFLAGE_TAGS: [&str; 7] = [
    "identity_summary",
    "split_summary",
    "site_summary",
    "specimen_summary",
    "preprocessing_summary",
    "policy_summary",
    "time_summary",
];

pub struct Generated {
    pub world: Value,
    pub query: Value,
}

pub fn generate(spec: &WorldSpec) -> Generated {
    Generated {
        world: generate_world(spec),
        query: generate_query(spec),
    }
}

fn subject_ids(spec: &WorldSpec) -> Vec<String> {
    (1..=spec.subjects).map(|n| format!("S{n:03}")).collect()
}

fn fact(id: &str, provides: &str, value: Value, tags: &[&str], provenance: &[&str]) -> Value {
    json!({
        "id": id,
        "provides": provides,
        "value": value,
        "scope": { "cohort": "RG-GEN-001" },
        "tags": tags,
        "provenance": provenance,
    })
}

fn factor(id: &str, inputs: &[&str], outputs: &[&str], kind: &str, tags: &[&str], cost: f64) -> Value {
    json!({
        "id": id,
        "inputs": inputs,
        "outputs": outputs,
        "kind": kind,
        "scope": { "cohort": "RG-GEN-001" },
        "tags": tags,
        "cost": cost,
    })
}

fn generate_world(spec: &WorldSpec) -> Value {
    let subjects = subject_ids(spec);
    let mut rng = SplitMix64::new(spec.seed);

    let half = subjects.len().div_ceil(2);
    let split: Map<String, Value> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| {
            let arm = if index < half { "train" } else { "test" };
            (subject.clone(), json!(arm))
        })
        .collect();

    // Identity leakage: one alias shared by a train subject and a test subject.
    let mut aliases = Map::new();
    for (index, subject) in subjects.iter().enumerate() {
        let mut names = vec![json!(subject)];
        if spec.has(LeakageMechanism::Identity) && (index == 0 || index == subjects.len() - 1) {
            names.push(json!("ALT-77"));
        }
        aliases.insert(subject.clone(), Value::Array(names));
    }

    // Site leakage: every training subject at one site, every test subject at another.
    let sites: Map<String, Value> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| {
            let site = if spec.has(LeakageMechanism::Site) {
                if index < half { "A" } else { "B" }
            } else {
                "A"
            };
            (subject.clone(), json!(site))
        })
        .collect();

    let scanners: Map<String, Value> = subjects
        .iter()
        .map(|subject| (subject.clone(), json!("SCN-1")))
        .collect();

    // Temporal leakage: a label derived from evidence recorded after the training cut.
    let label_times: Map<String, Value> = subjects
        .iter()
        .enumerate()
        .map(|(index, subject)| {
            let recorded = if spec.has(LeakageMechanism::Temporal) && index == subjects.len() - 1 {
                "2025-06-01"
            } else {
                "2024-11-15"
            };
            (subject.clone(), json!(recorded))
        })
        .collect();

    let specimen_dates: Map<String, Value> = subjects
        .iter()
        .map(|subject| (subject.clone(), json!("2024-10-01")))
        .collect();

    let preprocess_scope = if spec.has(LeakageMechanism::Preprocessing) {
        "all_subjects_before_split"
    } else {
        "train_only_after_split"
    };

    let mut facts = vec![
        fact("fact.cohort", "cohort_id", json!("RG-GEN-001"), &["identity"], &["manifest/cohort.json"]),
        fact("fact.subject_aliases", "subject_aliases", Value::Object(aliases), &["identity", "protected"], &["manifest/subjects.csv"]),
        fact("fact.split", "split_assignment", Value::Object(split), &["split", "protected"], &["analysis/split.json"]),
        fact("fact.site", "site_assignment", Value::Object(sites), &["site", "protected"], &["manifest/sites.csv"]),
        fact("fact.scanner", "scanner_assignment", Value::Object(scanners), &["scanner", "protected"], &["manifest/scanners.csv"]),
        fact("fact.label_source", "label_source_time", Value::Object(label_times), &["label_lineage", "protected"], &["analysis/labels.json"]),
        fact("fact.decision_cut", "training_decision_time", json!("2025-01-01"), &["time", "protected"], &["analysis/protocol.md"]),
        fact("fact.specimen_dates", "specimen_dates", Value::Object(specimen_dates), &["specimen", "protected"], &["manifest/specimens.csv"]),
        fact("fact.preprocess_fit", "preprocess_fit_scope", json!(preprocess_scope), &["preprocessing", "protected"], &["analysis/preprocess.md"]),
        fact("fact.policy", "data_policy", json!("research-only"), &["policy", "protected"], &["governance/policy.md"]),
        fact("fact.negative_duplicates", "declared_duplicate_screen", json!("no_duplicate_screen_performed"), &["negative_evidence", "protected"], &["manifest/qc.md"]),
        fact("fact.future_label", "future_label_value", json!("recurrence"), &["future_evidence"], &["analysis/followup.json"]),
    ];

    let mut factors = Vec::new();
    let mut claim_inputs: Vec<String> = Vec::new();

    for (check_id, terminal, inputs) in CHECKS {
        let (check_output, relay_start) = if spec.relay_depth == 0 {
            (terminal.to_string(), None)
        } else {
            (format!("{terminal}_r0"), Some(format!("{terminal}_r0")))
        };

        factors.push(factor(
            check_id,
            inputs,
            &[&check_output],
            "deterministic_rule",
            &["protected"],
            1.0,
        ));

        if let Some(start) = relay_start {
            let mut previous = start;
            for step in 1..=spec.relay_depth {
                let output = if step == spec.relay_depth {
                    terminal.to_string()
                } else {
                    format!("{terminal}_r{step}")
                };
                factors.push(factor(
                    &format!("factor.relay.{terminal}.{step}"),
                    &[&previous],
                    &[&output],
                    "relay_rule",
                    &["relay"],
                    0.05,
                ));
                previous = output;
            }
        }

        claim_inputs.push(terminal.to_string());
    }

    let claim_input_refs: Vec<&str> = claim_inputs.iter().map(String::as_str).collect();
    factors.push(factor(
        "factor.claim_support",
        &claim_input_refs,
        &["split_integrity_status"],
        "decision_rule",
        &["target"],
        0.2,
    ));

    let attachment = match spec.attachment {
        DistractorAttachment::Hub => "cohort_id".to_string(),
        DistractorAttachment::NearTarget => "identity_leakage".to_string(),
    };

    for index in 0..spec.distractors {
        let (fact_id, variable, tags) = match spec.tag_style {
            TagStyle::Distinct => (
                format!("fact.explore.{index:04}"),
                format!("exploratory_block_{index:04}"),
                vec!["exploratory".to_string()],
            ),
            TagStyle::Camouflaged => {
                let tag = *rng.pick(&CAMOUFLAGE_TAGS);
                (
                    format!("fact.{tag}.{index:04}"),
                    format!("{tag}_block_{index:04}"),
                    vec![tag.to_string(), "exploratory".to_string()],
                )
            }
        };

        let subject = rng.pick(&subjects).clone();
        let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
        facts.push(fact(
            &fact_id,
            &variable,
            json!({ "subject": subject, "score": rng.below(1000) as f64 / 1000.0 }),
            &tag_refs,
            &["analysis/exploratory.ipynb"],
        ));
        factors.push(factor(
            &format!("factor.summary.{index:04}"),
            &[&attachment, &variable],
            &[&format!("{variable}_summary")],
            "exploratory_summary",
            &["exploratory"],
            0.1,
        ));
    }

    json!({
        "schema_version": "fiber-world/0.1",
        "world_id": spec.world_id,
        "description": format!(
            "Generated structural family: relay_depth={}, attachment={:?}, tags={:?}, distractors={}",
            spec.relay_depth, spec.attachment, spec.tag_style, spec.distractors
        ),
        "facts": facts,
        "factors": factors,
        "events": [
            {
                "id": "event.training",
                "event_time": "2025-01-01T00:00:00Z",
                "availability_time": "2025-01-01T00:00:00Z",
                "causal_parents": [],
                "produces": ["training_decision_time", "split_assignment", "preprocess_fit_scope"]
            },
            {
                "id": "event.future_label",
                "event_time": "2025-06-01T00:00:00Z",
                "availability_time": "2025-06-15T00:00:00Z",
                "causal_parents": ["event.training"],
                "produces": ["future_label_value"]
            }
        ]
    })
}

fn generate_query(spec: &WorldSpec) -> Value {
    json!({
        "schema_version": "fiber-query/0.1",
        "query_id": format!("{}-split-integrity", spec.world_id),
        "targets": ["split_integrity_status"],
        "protected_tags": [
            "identity", "split", "site", "scanner", "time",
            "specimen", "preprocessing", "policy", "negative_evidence", "protected"
        ],
        "decision_time": "2025-01-01T00:00:00Z",
        "budgets": { "max_facts": 64, "max_tokens": 6000 },
        "role": "research-auditor",
        "policy": ["research-only"],
        "distortion_tolerance": 0.0
    })
}
