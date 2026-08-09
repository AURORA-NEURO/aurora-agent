//! World and query generation.
//!
//! Emits `fiber-world/0.1` and `fiber-query/0.1` documents. Generation is a pure function of the
//! [`WorldSpec`], including its seed, so a spec is a reproducible reference to a world.
//!
//! The decisive skeleton was fixed — the same eleven facts and five checks the reference world
//! uses, so the deterministic oracle applies unchanged — and only the *structure around it*
//! varied: how far the decisive facts sit from the target, where distractors attach, and whether
//! their tags are lexically distinguishable. [`WorldSpec::skeleton`] now selects among skeletons
//! and the rest of [`crate::spec`]'s new fields move the release schedule, the protected set, the
//! cut, the declared absences and the policy requirements, each defaulting to the value that was
//! previously a literal in this file.
//!
//! # Byte stability
//!
//! `fixtures/generated/` holds two of these worlds as committed bytes and `crates/bioworlds` reads
//! one of them to make a claim about the family. A default [`WorldSpec`] must therefore emit the
//! document it emitted before any knob existed, down to key order and tag order. Every branch
//! below is written so the default arm produces the literal it replaced;
//! `tests/generator_knobs.rs` checks that against the committed files rather than against itself.

use crate::rng::SplitMix64;
use crate::spec::{
    CheckSpec, DeclaredAbsence, DistractorAttachment, EventSpec, LeakageMechanism, Skeleton,
    TagStyle, WorldSpec, ASSAY_TAG, CENTRAL_LAB_CONFIRMATION, HYPOTHESIS_EXCLUSION,
    LOCAL_LAB_VALUE, PROTECTED_TAG, PROTECTED_VOCABULARY,
};
use serde_json::{json, Map, Value};

const CAMOUFLAGE_TAGS: [&str; 7] = [
    "identity_summary",
    "split_summary",
    "site_summary",
    "specimen_summary",
    "preprocessing_summary",
    "policy_summary",
    "time_summary",
];

/// The scope dimension a fact binds to name the clauses a reader must hold (43.33).
const POLICY_SCOPE_DIMENSION: &str = "policy";

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

/// The tags a core fact carries, given whether the spec protects its variable.
///
/// A protected variable's fact carries its topic tag and the [`PROTECTED_TAG`] marker, which is
/// the literal pair this generator used to hard-code. An unprotected one carries `open` instead,
/// and the two differ for a reason worth stating: dropping only the marker would leave a fact
/// tagged `split`, which [`PROTECTED_VOCABULARY`] names, so the closure would pull it straight back
/// in and [`WorldSpec::protected_variables`] would look like a knob that does nothing.
///
/// `open` is passed per fact rather than derived from `topic` because `fact.cohort` and
/// `fact.future_label` are already outside the vocabulary and have to keep the tags they have.
fn tags_for<'a>(spec: &WorldSpec, variable: &str, topic: &'a str, open: &'a str) -> Vec<&'a str> {
    if spec.protects(variable) {
        vec![topic, PROTECTED_TAG]
    } else {
        vec![open]
    }
}

/// The `data_policy` fact's value.
///
/// A single clause renders as a bare string and several render as a list. Both are legal —
/// `bioprism_fiber::policy` reads either — and the scalar form is what the reference world and
/// every world this crate has generated carry, so preserving it is what keeps a default spec's
/// bytes unmoved.
fn governing_value(spec: &WorldSpec) -> Value {
    match spec.policy.governing.as_slice() {
        [only] => json!(only),
        many => json!(many),
    }
}

/// Binds `policy` scope dimensions onto the facts whose variables declare a requirement.
///
/// Applied after the facts are built rather than threaded through [`fact`], so that a spec
/// declaring no requirement — the default — emits the same single-dimension scope object it always
/// has, with no branch inside the constructor to get wrong.
fn bind_policy_requirements(spec: &WorldSpec, facts: &mut [Value]) {
    if spec.policy.requirements.is_empty() {
        return;
    }
    for document in facts.iter_mut() {
        let Some(variable) = document
            .get("provides")
            .and_then(Value::as_str)
            .map(str::to_string)
        else {
            continue;
        };
        let Some(clauses) = spec.policy.requirements.get(&variable) else {
            continue;
        };
        if let Some(scope) = document.get_mut("scope").and_then(Value::as_object_mut) {
            scope.insert(POLICY_SCOPE_DIMENSION.to_string(), json!(clauses));
        }
    }
}

/// The facts the skeleton adds beyond the twelve core ones.
///
/// [`Skeleton::ExternalConfirmation`] contributes two lab facts tagged [`ASSAY_TAG`], which no
/// generated query protects. Their values are computed from the subject index rather than from the
/// [`SplitMix64`] stream, so adding the skeleton does not shift the distractor draw and the two
/// knobs stay independent.
fn skeleton_facts(spec: &WorldSpec, subjects: &[String]) -> Vec<Value> {
    match spec.skeleton {
        Skeleton::SplitIntegrity => Vec::new(),
        Skeleton::ExternalConfirmation => {
            let local: Map<String, Value> = subjects
                .iter()
                .enumerate()
                .map(|(index, subject)| {
                    (
                        subject.clone(),
                        json!({ "drawn": "2024-11-20", "value": 100 + (index % 40) }),
                    )
                })
                .collect();
            let central: Map<String, Value> = subjects
                .iter()
                .enumerate()
                .map(|(index, subject)| {
                    (
                        subject.clone(),
                        json!({ "specimen_drawn": "2024-12-20", "confirmed": index % 3 != 0 }),
                    )
                })
                .collect();
            vec![
                fact(
                    "fact.local_lab",
                    LOCAL_LAB_VALUE,
                    Value::Object(local),
                    &tags_for(spec, LOCAL_LAB_VALUE, ASSAY_TAG, ASSAY_TAG),
                    &["labs/local_panel.csv"],
                ),
                fact(
                    "fact.central_lab",
                    CENTRAL_LAB_CONFIRMATION,
                    Value::Object(central),
                    &tags_for(spec, CENTRAL_LAB_CONFIRMATION, ASSAY_TAG, ASSAY_TAG),
                    &["labs/central_confirmation.csv"],
                ),
            ]
        }
    }
}

/// Whether some check already reads `variable`, and so the claim factor need not.
fn consumed_by_a_check(checks: &[CheckSpec], variable: &str) -> bool {
    checks
        .iter()
        .any(|check| check.inputs.contains(&variable))
}

fn event_document(event: &EventSpec) -> Value {
    json!({
        "id": event.id,
        "event_time": event.event_time,
        "availability_time": event.availability_time,
        "causal_parents": event.causal_parents,
        "produces": event.produces,
    })
}

/// The declared-absence fact for one record.
fn absence_fact(spec: &WorldSpec, record: &DeclaredAbsence) -> Value {
    let provenance: Vec<&str> = record.provenance.iter().map(String::as_str).collect();
    fact(
        &record.fact_id,
        &record.provides,
        json!(record.status),
        &tags_for(
            spec,
            &record.provides,
            "negative_evidence",
            "negative_evidence_record",
        ),
        &provenance,
    )
}

/// The one-line summary of the structural corner, for a human reading the document.
///
/// The skeleton is appended only when it is not the default. That is not tidiness: the two
/// committed fixtures carry the sentence this produces, so the default must render exactly the
/// four knobs it always rendered.
fn description(spec: &WorldSpec) -> String {
    let mut text = format!(
        "Generated structural family: relay_depth={}, attachment={:?}, tags={:?}, distractors={}",
        spec.relay_depth, spec.attachment, spec.tag_style, spec.distractors
    );
    if spec.skeleton != Skeleton::default() {
        text.push_str(&format!(", skeleton={:?}", spec.skeleton));
    }
    text
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
        fact("fact.cohort", "cohort_id", json!("RG-GEN-001"), &tags_for(spec, "cohort_id", "identity", "identity"), &["manifest/cohort.json"]),
        fact("fact.subject_aliases", "subject_aliases", Value::Object(aliases), &tags_for(spec, "subject_aliases", "identity", "identity_record"), &["manifest/subjects.csv"]),
        fact("fact.split", "split_assignment", Value::Object(split), &tags_for(spec, "split_assignment", "split", "split_record"), &["analysis/split.json"]),
        fact("fact.site", "site_assignment", Value::Object(sites), &tags_for(spec, "site_assignment", "site", "site_record"), &["manifest/sites.csv"]),
        fact("fact.scanner", "scanner_assignment", Value::Object(scanners), &tags_for(spec, "scanner_assignment", "scanner", "scanner_record"), &["manifest/scanners.csv"]),
        fact("fact.label_source", "label_source_time", Value::Object(label_times), &tags_for(spec, "label_source_time", "label_lineage", "label_lineage"), &["analysis/labels.json"]),
        fact("fact.decision_cut", "training_decision_time", json!("2025-01-01"), &tags_for(spec, "training_decision_time", "time", "time_record"), &["analysis/protocol.md"]),
        fact("fact.specimen_dates", "specimen_dates", Value::Object(specimen_dates), &tags_for(spec, "specimen_dates", "specimen", "specimen_record"), &["manifest/specimens.csv"]),
        fact("fact.preprocess_fit", "preprocess_fit_scope", json!(preprocess_scope), &tags_for(spec, "preprocess_fit_scope", "preprocessing", "preprocessing_record"), &["analysis/preprocess.md"]),
        fact("fact.policy", "data_policy", governing_value(spec), &tags_for(spec, "data_policy", "policy", "policy_record"), &["governance/policy.md"]),
    ];

    for record in &spec.declared_absent {
        facts.push(absence_fact(spec, record));
    }

    facts.push(fact("fact.future_label", "future_label_value", json!("recurrence"), &tags_for(spec, "future_label_value", "future_evidence", "future_evidence"), &["analysis/followup.json"]));
    facts.extend(skeleton_facts(spec, &subjects));

    let checks = spec.skeleton.checks();
    let mut factors = Vec::new();
    let mut terminals: Vec<String> = Vec::new();

    for check in checks {
        let (check_output, relay_start) = if spec.relay_depth == 0 {
            (check.terminal.to_string(), None)
        } else {
            (
                format!("{}_r0", check.terminal),
                Some(format!("{}_r0", check.terminal)),
            )
        };

        factors.push(factor(
            check.factor_id,
            check.inputs,
            &[&check_output],
            "deterministic_rule",
            &[PROTECTED_TAG],
            1.0,
        ));

        if let Some(start) = relay_start {
            let mut previous = start;
            for step in 1..=spec.relay_depth {
                let output = if step == spec.relay_depth {
                    check.terminal.to_string()
                } else {
                    format!("{}_r{step}", check.terminal)
                };
                factors.push(factor(
                    &format!("factor.relay.{}.{step}", check.terminal),
                    &[&previous],
                    &[&output],
                    "relay_rule",
                    &["relay"],
                    0.05,
                ));
                previous = output;
            }
        }

        terminals.push(check.terminal.to_string());
    }

    let terminal_refs: Vec<&str> = terminals.iter().map(String::as_str).collect();
    let mut claim_inputs = terminals.clone();

    for hypothesis in &spec.hypotheses {
        factors.push(factor(
            &format!("factor.hypothesis.{hypothesis}"),
            &terminal_refs,
            &[hypothesis],
            "hypothesis_rule",
            &["hypothesis"],
            0.2,
        ));
    }
    if !spec.hypotheses.is_empty() {
        let hypothesis_refs: Vec<&str> = spec.hypotheses.iter().map(String::as_str).collect();
        factors.push(factor(
            "factor.hypothesis_exclusion",
            &hypothesis_refs,
            &[HYPOTHESIS_EXCLUSION],
            "exclusion_rule",
            &["exclusion"],
            0.05,
        ));
    }

    for record in &spec.declared_absent {
        if !consumed_by_a_check(checks, &record.provides) {
            claim_inputs.push(record.provides.clone());
        }
    }
    if !spec.hypotheses.is_empty() {
        claim_inputs.push(HYPOTHESIS_EXCLUSION.to_string());
    }

    let claim_input_refs: Vec<&str> = claim_inputs.iter().map(String::as_str).collect();
    factors.push(factor(
        "factor.claim_support",
        &claim_input_refs,
        &[spec.skeleton.target()],
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

    bind_policy_requirements(spec, &mut facts);

    let events: Vec<Value> = spec.events.iter().map(event_document).collect();

    json!({
        "schema_version": "fiber-world/0.1",
        "world_id": spec.world_id,
        "description": description(spec),
        "facts": facts,
        "factors": factors,
        "events": events
    })
}

fn generate_query(spec: &WorldSpec) -> Value {
    json!({
        "schema_version": "fiber-query/0.1",
        "query_id": format!("{}-split-integrity", spec.world_id),
        "targets": [spec.skeleton.target()],
        "protected_tags": PROTECTED_VOCABULARY,
        "decision_time": spec.decision_time,
        "budgets": { "max_facts": 64, "max_tokens": 6000 },
        "role": "research-auditor",
        "policy": spec.policy.accepted,
        "distortion_tolerance": 0.0
    })
}
