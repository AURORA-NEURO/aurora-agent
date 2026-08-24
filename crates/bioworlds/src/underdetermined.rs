//! Reference world 2: longitudinal post-treatment uncertainty, built to underdetermine.
//!
//! Blueprint 38.02 asks for a world whose task is to "maintain calibrated competing hypotheses for
//! progression, treatment effect, and mixed disease", and names *abstain or issue probability
//! distribution* as a permitted action and *premature closure* as a metric. `bioprism-onco`
//! already models that differential — [`ChangeHypothesis`], [`DiscriminatingEvidence`],
//! [`ObservationStatus`] — so this world borrows its vocabulary instead of inventing one, and the
//! variable names below are derived from onco's own serde representation rather than retyped.
//!
//! # The gap this world addresses, and the part of it that stays open
//!
//! `crates/examples` records `underdetermined_abstention` as blocked: `OracleVerdict::abstain`
//! exists in `bioprism-section` but no path in `bioprism-fiber` constructs it. Construction now
//! lives behind injection — `bioprism_fiber::compile_with_oracle` judges a compile by any
//! `DecisionOracle`, and `bioprism-domain`'s rule oracles return abstaining verdicts through it —
//! yet the default compile's split-integrity oracle still derives status solely from whether the
//! witness list is empty.
//!
//! That blocker is about the *compiler*, and this crate cannot lift it — `bioprism-fiber` is not
//! in the dependency set. What a judge with an abstention path needs is an input that genuinely
//! underdetermines, and that input did not exist. This world is it, and [`AbstentionStep`] states
//! exactly what a judge of this world would still have to be built to do.
//!
//! The distinction the world is built around is the one that makes the property hard: **this is
//! not a world with missing evidence.** Every input to every hypothesis-support factor is
//! provided by a fact, and every one of them is readable at the decision cut. What is absent is
//! absent *on the record* — the four discriminating studies are present as facts whose value
//! declares [`ObservationStatus::NotCollected`], which is 30.02's rule that "never collected" and
//! "missing" are separate states. So a compiler can assemble a complete, closed, budget-fitting
//! context here and still have two live hypotheses. On the v0.1 oracle the witness list is empty,
//! which yields `valid` — and `valid` is the wrong answer, not a missing one.
//!
//! # The structural rule, and what it is not
//!
//! A hypothesis is **settled** when its support factor takes at least one input variable that is
//! tagged `discriminating_evidence` *and* whose fact declares an observed value; otherwise it is
//! **live**. Which study discriminates which hypothesis is a **declaration made by this world**,
//! encoded as factor inputs so it is readable from the document. `bioprism-onco` names
//! [`DiscriminatingEvidence`] as a *request* vocabulary and states no such mapping; nothing here
//! should be read as a claim that it does. The measurement values, the 44-day interval and the
//! criterion identifiers are illustrative. The one non-illustrative number is the 84-day window,
//! taken from [`TreatmentModality::post_treatment_window_days`], which documents itself as
//! conventional rather than blueprint-specified.
//!
//! # A cohort, not a case
//!
//! §38's section index states that these examples "are not patient cases". The measurements here
//! are therefore per-subject maps over a synthetic research cohort at 38.01's 80-200 scale, and
//! the hypotheses are cohort-level: the world asks what the *cohort's* post-treatment change
//! assessment is, which is a research question, not a diagnosis. `bioprism-onco`'s research
//! boundary (30.30) draws the same line in its types.
//!
//! # Not implemented
//!
//! No probability, no Brier score, no calibration and no belief revision: 38.02's metrics need a
//! scoring harness, and a world is facts, typed factors and events. There is no `most_likely`
//! accessor here for the same reason `bioprism-onco` refuses one — it would be used.

use crate::builder::{per_subject, subject_ids, BioWorld, WorldBuilder};
use crate::error::BioWorldError;
use crate::query::{tag_set, QueryShape};
use crate::structure::DependencyClosure;
use bioprism_onco::response::{
    ChangeHypothesis, DiscriminatingEvidence, ResponseCriterion, TreatmentModality,
};
use bioprism_onco::status::ObservationStatus;
use bioprism_worldgen::rng::SplitMix64;
use bioprism_worldgen::spec::{DistractorAttachment, TagStyle};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// The decision variable: a time-indexed research assessment, never a diagnosis (§38.02's
/// required final deliverable).
pub const TARGET: &str = "change_assessment_status";

/// The variable the confirmation study would supply. Withheld at the cut and unprotected, so this
/// world carries a second instance of the §38.08 pattern in a different clinical shape.
pub const CONFIRMATION_MEASUREMENT: &str = "confirmation_measurement";

/// Kind marking a factor whose output is a hypothesis's support.
pub const HYPOTHESIS_SUPPORT_RULE: &str = "hypothesis_support_rule";

/// Kind marking the factor declaring the hypotheses mutually exclusive.
pub const MUTUAL_EXCLUSION_RULE: &str = "mutual_exclusion_rule";

/// Tag marking a fact that would discriminate between hypotheses.
pub const DISCRIMINATING_EVIDENCE_TAG: &str = "discriminating_evidence";

pub const DISTRACTOR_TAG: &str = "exploratory";

/// The index study, and therefore the cut.
pub const ASSESSMENT_CUT: &str = "2025-04-24T00:00:00Z";

/// The three hypotheses this world keeps live, in onco's vocabulary.
pub const HYPOTHESES: [ChangeHypothesis; 3] = [
    ChangeHypothesis::Progression,
    ChangeHypothesis::TreatmentEffect,
    ChangeHypothesis::MixedProcess,
];

/// The four studies the world declares as discriminating, in onco's vocabulary.
pub const DISCRIMINATING: [DiscriminatingEvidence; 4] = [
    DiscriminatingEvidence::PerfusionMri,
    DiscriminatingEvidence::AminoAcidPet,
    DiscriminatingEvidence::Histopathology,
    DiscriminatingEvidence::IntervalFollowUp,
];

const CAMOUFLAGE_TAGS: [&str; 6] = [
    "measurement_summary",
    "treatment_summary",
    "criterion_summary",
    "clinical_summary",
    "evidence_summary",
    "discriminating_summary",
];

/// The serde name of a unit variant, used as a variable name.
///
/// Deriving names rather than retyping them means the world and `bioprism-onco` cannot drift: a
/// rename in onco changes this world's variables, and the tests that assert the correspondence
/// fail loudly instead of the world quietly speaking a stale vocabulary.
pub fn vocabulary_name<T: Serialize>(value: &T, subject: &str) -> Result<String, BioWorldError> {
    match serde_json::to_value(value) {
        Ok(Value::String(name)) => Ok(name),
        _ => Err(BioWorldError::VocabularyNotNameable {
            subject: subject.to_string(),
        }),
    }
}

/// The variable carrying a hypothesis's support, e.g. `hypothesis_progression_support`.
pub fn hypothesis_variable(hypothesis: &ChangeHypothesis) -> Result<String, BioWorldError> {
    let name = vocabulary_name(hypothesis, "ChangeHypothesis")?;
    Ok(format!("hypothesis_{name}_support"))
}

/// The variable carrying a discriminating study, e.g. `perfusion_mri`.
pub fn evidence_variable(evidence: &DiscriminatingEvidence) -> Result<String, BioWorldError> {
    vocabulary_name(evidence, "DiscriminatingEvidence")
}

/// The structural knobs, reusing `bioprism-worldgen`'s enums unchanged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostTreatmentSpec {
    pub world_id: String,
    pub subjects: usize,
    pub distractors: usize,
    pub relay_depth: usize,
    pub attachment: DistractorAttachment,
    pub tag_style: TagStyle,
    pub seed: u64,
    /// Which discriminating studies were actually run. Every study *not* listed still appears as
    /// a fact, declaring [`ObservationStatus::NotCollected`] — the world never omits the record of
    /// an absence.
    pub collected: Vec<DiscriminatingEvidence>,
}

impl PostTreatmentSpec {
    /// The shipped world: nothing discriminating was collected, so three hypotheses stay live.
    pub fn underdetermined() -> Self {
        PostTreatmentSpec {
            world_id: "onco-world-post-treatment-underdetermination-v1".into(),
            subjects: 96,
            distractors: 120,
            relay_depth: 3,
            attachment: DistractorAttachment::NearTarget,
            tag_style: TagStyle::Camouflaged,
            seed: 20_260_808,
            collected: Vec::new(),
        }
    }

    /// The control: the identical skeleton with perfusion collected.
    ///
    /// It exists so underdetermination can be shown to be a property of the *evidence set* rather
    /// than of the factor graph. If the live-hypothesis count did not move, the count would be
    /// measuring the skeleton and would mean nothing.
    pub fn resolved_control() -> Self {
        PostTreatmentSpec {
            world_id: "onco-world-post-treatment-resolved-control-v1".into(),
            collected: vec![DiscriminatingEvidence::PerfusionMri],
            ..PostTreatmentSpec::underdetermined()
        }
    }

    pub fn is_at_cohort_scale(&self) -> bool {
        (80..=200).contains(&self.subjects)
    }

    fn was_collected(&self, evidence: &DiscriminatingEvidence) -> bool {
        self.collected.contains(evidence)
    }
}

/// Builds the world. Deterministic in the spec; no clock, no system RNG.
pub fn build(spec: &PostTreatmentSpec) -> Result<BioWorld, BioWorldError> {
    let subjects = subject_ids(spec.subjects);
    let mut rng = SplitMix64::new(spec.seed);
    let criterion = ResponseCriterion::high_grade_2010();
    let modality = TreatmentModality::ChemoRadiotherapy;
    let window_days = modality.post_treatment_window_days();
    let modality_name = vocabulary_name(&modality, "TreatmentModality")?;
    let compartment_name = vocabulary_name(&criterion.compartment, "Compartment")?;
    let not_collected = vocabulary_name(&ObservationStatus::NotCollected, "ObservationStatus")?;

    let mut builder = WorldBuilder::new(
        spec.world_id.clone(),
        format!(
            "Longitudinal post-treatment uncertainty (38.02): relay_depth={}, attachment={:?}, tags={:?}, distractors={}, subjects={}, collected_discriminating_evidence={}. \
             Every hypothesis-support input is present and readable at the cut; what is absent is declared absent, so the evidence is complete and still does not settle the verdict.",
            spec.relay_depth,
            spec.attachment,
            spec.tag_style,
            spec.distractors,
            spec.subjects,
            spec.collected.len()
        ),
        "PT-UNCERTAINTY-001",
    );

    builder
        .fact(
            "fact.response_criterion",
            "response_criterion",
            json!({
                "id": criterion.id,
                "version": criterion.version,
                "compartment": compartment_name,
                "progression_increase": criterion.progression_increase,
                "confirmation_interval_days": criterion.confirmation_interval_days,
            }),
            &["criterion", "protected"],
            &["criteria/response_criteria.json"],
        )
        .fact(
            "fact.baseline_measurement",
            "baseline_enhancing_diameter_product",
            per_subject(&subjects, |index, _| {
                json!({ "acquired": "2025-03-05", "bidimensional_product_mm2": 380 + (index % 9) * 10 })
            }),
            &["measurement", "protected"],
            &["imaging/baseline_report.json"],
        )
        .fact(
            "fact.index_measurement",
            "index_enhancing_diameter_product",
            per_subject(&subjects, |index, _| {
                json!({ "acquired": "2025-04-24", "bidimensional_product_mm2": 560 + (index % 9) * 10 })
            }),
            &["measurement", "protected"],
            &["imaging/index_report.json"],
        )
        .fact(
            "fact.new_lesion_status",
            "new_lesion_status",
            per_subject(&subjects, |_, _| {
                json!({ "observed": true, "new_lesion_present": false })
            }),
            &["measurement", "protected"],
            &["imaging/index_report.json"],
        )
        .fact(
            "fact.treatment_timeline",
            "treatment_timeline",
            per_subject(&subjects, |_, _| {
                json!({ "modality": modality_name, "completed": "2025-03-11" })
            }),
            &["treatment", "protected"],
            &["timeline/treatment.json"],
        )
        .fact(
            "fact.days_since_treatment",
            "days_since_treatment_end",
            per_subject(&subjects, |_, _| json!(44)),
            &["treatment", "protected"],
            &["timeline/treatment.json"],
        )
        .fact(
            "fact.post_treatment_window",
            "post_treatment_window_days",
            json!(window_days),
            &["treatment", "protected"],
            &["criteria/post_treatment_window.md"],
        )
        .fact(
            "fact.corticosteroid",
            "corticosteroid_dose_trend",
            per_subject(&subjects, |_, _| {
                json!({ "observed": true, "trend": "unchanged" })
            }),
            &["treatment", "protected"],
            &["timeline/corticosteroid.json"],
        )
        .fact(
            "fact.clinical_trend",
            "clinical_trend",
            per_subject(&subjects, |_, _| {
                json!({ "observed": true, "trend": "stable" })
            }),
            &["clinical", "protected"],
            &["timeline/clinical_assessment.json"],
        )
        .fact(
            "fact.declared_sampling_limitation",
            "declared_sampling_limitation",
            json!("single_region_sampling_would_not_exclude_a_mixed_process"),
            &["negative_evidence", "protected"],
            &["limitations/sampling.md"],
        );

    for evidence in DISCRIMINATING {
        let variable = evidence_variable(&evidence)?;
        let value = if spec.was_collected(&evidence) {
            json!({ "observed": true, "acquired": "2025-04-24", "reading": "recorded" })
        } else {
            json!({ "observed": false, "status": not_collected })
        };
        builder.fact(
            &format!("fact.{variable}"),
            &variable,
            value,
            &[DISCRIMINATING_EVIDENCE_TAG, "protected"],
            &["imaging/advanced_study_ledger.json"],
        );
    }

    builder.fact(
        "fact.confirmation",
        CONFIRMATION_MEASUREMENT,
        per_subject(&subjects, |_, _| {
            json!({ "scheduled": "2025-05-12", "bidimensional_product_mm2": null })
        }),
        &["confirmation"],
        &["imaging/confirmation_schedule.json"],
    );

    let perfusion = evidence_variable(&DiscriminatingEvidence::PerfusionMri)?;
    let pet = evidence_variable(&DiscriminatingEvidence::AminoAcidPet)?;
    let histopathology = evidence_variable(&DiscriminatingEvidence::Histopathology)?;
    let follow_up = evidence_variable(&DiscriminatingEvidence::IntervalFollowUp)?;

    let progression = hypothesis_variable(&ChangeHypothesis::Progression)?;
    let treatment_effect = hypothesis_variable(&ChangeHypothesis::TreatmentEffect)?;
    let mixed = hypothesis_variable(&ChangeHypothesis::MixedProcess)?;

    builder
        .factor(
            "factor.measurement_delta",
            &[
                "baseline_enhancing_diameter_product",
                "index_enhancing_diameter_product",
                "response_criterion",
            ],
            &["enhancing_change_fraction"],
            "deterministic_rule",
            &["protected"],
            1.0,
        )
        .factor(
            "factor.progression_support",
            &[
                "enhancing_change_fraction",
                "response_criterion",
                "new_lesion_status",
                &perfusion,
                &pet,
            ],
            &[&progression],
            HYPOTHESIS_SUPPORT_RULE,
            &["hypothesis"],
            1.0,
        )
        .factor(
            "factor.treatment_effect_support",
            &[
                "enhancing_change_fraction",
                "days_since_treatment_end",
                "post_treatment_window_days",
                "treatment_timeline",
                "corticosteroid_dose_trend",
                &perfusion,
                &histopathology,
            ],
            &[&treatment_effect],
            HYPOTHESIS_SUPPORT_RULE,
            &["hypothesis"],
            1.0,
        )
        .factor(
            "factor.mixed_process_support",
            &[
                "enhancing_change_fraction",
                "days_since_treatment_end",
                "clinical_trend",
                &histopathology,
                "declared_sampling_limitation",
            ],
            &[&mixed],
            HYPOTHESIS_SUPPORT_RULE,
            &["hypothesis"],
            1.0,
        );

    let exclusion_output = if spec.relay_depth == 0 {
        "live_hypothesis_set".to_string()
    } else {
        "live_hypothesis_set_r0".to_string()
    };
    builder.factor(
        "factor.hypothesis_exclusion",
        &[&progression, &treatment_effect, &mixed],
        &[&exclusion_output],
        MUTUAL_EXCLUSION_RULE,
        &["protected"],
        0.5,
    );

    if spec.relay_depth > 0 {
        let mut previous = exclusion_output;
        for step in 1..=spec.relay_depth {
            let output = if step == spec.relay_depth {
                "live_hypothesis_set".to_string()
            } else {
                format!("live_hypothesis_set_r{step}")
            };
            builder.factor(
                &format!("factor.relay.live_hypothesis_set.{step}"),
                &[&previous],
                &[&output],
                "relay_rule",
                &["relay"],
                0.05,
            );
            previous = output;
        }
    }

    builder
        .factor(
            "factor.discrimination_request",
            &[
                &perfusion,
                &pet,
                &histopathology,
                &follow_up,
                "live_hypothesis_set",
            ],
            &["discriminating_evidence_request"],
            "evidence_request_rule",
            &["protected"],
            0.3,
        )
        .factor(
            "factor.confirmation_check",
            &[
                CONFIRMATION_MEASUREMENT,
                "response_criterion",
                "index_enhancing_diameter_product",
            ],
            &["confirmation_status"],
            "deterministic_rule",
            &["protected"],
            1.0,
        )
        .factor(
            "factor.change_assessment",
            &[
                "live_hypothesis_set",
                "discriminating_evidence_request",
                "confirmation_status",
                "days_since_treatment_end",
                "post_treatment_window_days",
            ],
            &[TARGET],
            "decision_rule",
            &["target"],
            0.2,
        );

    let attachment = match spec.attachment {
        DistractorAttachment::Hub => "response_criterion".to_string(),
        DistractorAttachment::NearTarget => "live_hypothesis_set".to_string(),
    };

    for index in 0..spec.distractors {
        let (fact_id, variable, tags) = match spec.tag_style {
            TagStyle::Distinct => (
                format!("fact.explore.{index:04}"),
                format!("exploratory_block_{index:04}"),
                vec![DISTRACTOR_TAG.to_string()],
            ),
            TagStyle::Camouflaged => {
                let tag = *rng.pick(&CAMOUFLAGE_TAGS);
                (
                    format!("fact.{tag}.{index:04}"),
                    format!("{tag}_block_{index:04}"),
                    vec![tag.to_string(), DISTRACTOR_TAG.to_string()],
                )
            }
        };
        let subject = rng.pick(&subjects).clone();
        let tag_refs: Vec<&str> = tags.iter().map(String::as_str).collect();
        builder.fact(
            &fact_id,
            &variable,
            json!({ "subject": subject, "score": rng.below(1000) as f64 / 1000.0 }),
            &tag_refs,
            &["analysis/longitudinal_exploratory.ipynb"],
        );
        builder.factor(
            &format!("factor.summary.{index:04}"),
            &[&attachment, &variable],
            &[&format!("{variable}_summary")],
            "exploratory_summary",
            &[DISTRACTOR_TAG],
            0.1,
        );
    }

    builder
        .event(
            "event.baseline_study",
            "2025-03-05T00:00:00Z",
            "2025-03-06T00:00:00Z",
            &[],
            &["baseline_enhancing_diameter_product"],
        )
        .event(
            "event.treatment_completion",
            "2025-03-11T00:00:00Z",
            "2025-03-11T00:00:00Z",
            &["event.baseline_study"],
            &["treatment_timeline", "days_since_treatment_end"],
        )
        .event(
            "event.index_study",
            "2025-04-24T00:00:00Z",
            "2025-04-24T00:00:00Z",
            &["event.treatment_completion"],
            &["index_enhancing_diameter_product", "new_lesion_status"],
        )
        .event(
            "event.confirmation_study",
            "2025-05-12T00:00:00Z",
            "2025-05-19T00:00:00Z",
            &["event.index_study"],
            &[CONFIRMATION_MEASUREMENT],
        );

    builder.build()
}

/// The query shape: a change assessment compiled at the index study.
///
/// `confirmation` is not protected, so the confirmation study is withheld by time alone. The
/// discriminating-evidence facts *are* protected, which is deliberate: their declared absence is
/// evidence about the state of the record, and a closure that dropped it would let a compiler
/// present an underdetermined case as a settled one without ever seeing what it lacked.
pub fn query(spec: &PostTreatmentSpec) -> QueryShape {
    QueryShape {
        query_id: format!("{}-change-assessment", spec.world_id),
        targets: vec![TARGET.to_string()],
        protected_tags: tag_set(&[
            "criterion",
            "measurement",
            "treatment",
            "clinical",
            DISCRIMINATING_EVIDENCE_TAG,
            "negative_evidence",
            "protected",
        ]),
        decision_time: ASSESSMENT_CUT.to_string(),
        max_facts: 64,
        max_tokens: 6000,
        role: "research-auditor".into(),
        policy: vec!["research-only".into()],
    }
}

/// What the world's structure says about whether the verdict is settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnderdeterminationProfile {
    pub world_id: String,
    pub hypothesis_variables: Vec<String>,
    /// Hypotheses no observed discriminating study bears on.
    pub live_hypotheses: Vec<String>,
    pub settled_hypotheses: Vec<String>,
    /// Factors declaring the hypotheses mutually exclusive. Without one, several simultaneously
    /// supported hypotheses would be a conjunction, not an ambiguity.
    pub exclusion_factors: Vec<String>,
    pub declared_unobserved_discriminating_evidence: Vec<String>,
    pub observed_discriminating_evidence: Vec<String>,
    /// Support-factor inputs no fact provides and no factor produces. Non-empty means the world
    /// is *missing* evidence, which is a different and much weaker condition.
    pub unresolvable_support_inputs: Vec<String>,
    /// Support-factor inputs unreadable at the cut. Non-empty means the ambiguity is temporal.
    pub support_inputs_withheld_at_cut: Vec<String>,
}

impl UnderdeterminationProfile {
    /// Whether the evidence is complete, readable, and still leaves the verdict open.
    ///
    /// All four conjuncts matter. Two live hypotheses with a missing input is a retrieval failure;
    /// two live hypotheses with no exclusion factor is a mixed state, which 30.07 requires be
    /// reportable in its own right; and two live hypotheses behind a temporal cut is §38.08's
    /// property, not this one.
    pub fn is_underdetermined(&self) -> bool {
        self.live_hypotheses.len() >= 2
            && !self.exclusion_factors.is_empty()
            && self.unresolvable_support_inputs.is_empty()
            && self.support_inputs_withheld_at_cut.is_empty()
    }

    /// Whether exactly one hypothesis survives, i.e. the evidence settles it.
    pub fn is_determined(&self) -> bool {
        self.live_hypotheses.len() == 1
    }
}

/// Reads the underdetermination structure off a world.
///
/// Generic over any world: it reads declared factor kinds and tags, not this module's constants
/// applied by name, so a consumer can point it at a world this crate did not build.
pub fn analyse(world: &BioWorld, query: &QueryShape) -> Result<UnderdeterminationProfile, BioWorldError> {
    let inner = world.world();
    let profile = crate::structure::profile(world, query, DISTRACTOR_TAG)?;
    let withheld: BTreeSet<&str> = profile
        .temporal
        .withheld
        .iter()
        .map(String::as_str)
        .collect();

    let observed: BTreeSet<String> = inner
        .facts
        .iter()
        .filter(|fact| fact.has_tag(DISCRIMINATING_EVIDENCE_TAG))
        .filter(|fact| fact.value.get("observed").and_then(Value::as_bool) == Some(true))
        .map(|fact| fact.provides.as_str().to_string())
        .collect();
    let unobserved: BTreeSet<String> = inner
        .facts
        .iter()
        .filter(|fact| fact.has_tag(DISCRIMINATING_EVIDENCE_TAG))
        .filter(|fact| fact.value.get("observed").and_then(Value::as_bool) != Some(true))
        .map(|fact| fact.provides.as_str().to_string())
        .collect();
    let provided: BTreeSet<&str> = inner.facts.iter().map(|f| f.provides.as_str()).collect();
    let produced: BTreeSet<&str> = inner
        .factors
        .iter()
        .flat_map(|f| f.outputs.iter().map(|o| o.as_str()))
        .collect();

    let mut hypothesis_variables = Vec::new();
    let mut live = Vec::new();
    let mut settled = Vec::new();
    let mut unresolvable = BTreeSet::new();
    let mut withheld_inputs = BTreeSet::new();

    for factor in inner.factors.iter().filter(|f| f.kind == HYPOTHESIS_SUPPORT_RULE) {
        for output in &factor.outputs {
            hypothesis_variables.push(output.as_str().to_string());
            let is_settled = factor
                .inputs
                .iter()
                .any(|input| observed.contains(input.as_str()));
            if is_settled {
                settled.push(output.as_str().to_string());
            } else {
                live.push(output.as_str().to_string());
            }
        }
        for input in &factor.inputs {
            let name = input.as_str();
            if !provided.contains(name) && !produced.contains(name) {
                unresolvable.insert(name.to_string());
            }
            if withheld.contains(name) {
                withheld_inputs.insert(name.to_string());
            }
        }
    }

    hypothesis_variables.sort();
    live.sort();
    settled.sort();

    Ok(UnderdeterminationProfile {
        world_id: world.id().to_string(),
        hypothesis_variables,
        live_hypotheses: live,
        settled_hypotheses: settled,
        exclusion_factors: inner
            .factors
            .iter()
            .filter(|f| f.kind == MUTUAL_EXCLUSION_RULE)
            .map(|f| f.id.as_str().to_string())
            .collect(),
        declared_unobserved_discriminating_evidence: unobserved.into_iter().collect(),
        observed_discriminating_evidence: observed.into_iter().collect(),
        unresolvable_support_inputs: unresolvable.into_iter().collect(),
        support_inputs_withheld_at_cut: withheld_inputs.into_iter().collect(),
    })
}

/// Whether the target depends on every hypothesis variable.
///
/// An underdetermining world in which the target ignores the hypotheses would underdetermine
/// nothing; this is the check that the ambiguity is on the decision path.
pub fn target_depends_on_every_hypothesis(
    world: &BioWorld,
    profile: &UnderdeterminationProfile,
    target: &str,
) -> bool {
    let closure = DependencyClosure::of_target(world.world(), target);
    profile
        .hypothesis_variables
        .iter()
        .all(|variable| closure.depends_on(variable))
}

/// One thing a compiler would have to do to return `underdetermined` on this world.
///
/// Written as an enumeration rather than a paragraph so a later contributor can tick them off and
/// so [`AbstentionStep::ALL`] is testable. None of them is implemented here; this crate has no
/// compiler in its dependency set and constructs no verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstentionStep {
    /// Read `factor.hypothesis_exclusion` and recognise `mutual_exclusion_rule` as a constraint
    /// over its inputs rather than as one more relation to evaluate.
    InterpretTheMutualExclusionFactor,
    /// Evaluate each `hypothesis_support_rule` against the selected evidence.
    EvaluateEachHypothesisSupportFactor,
    /// Recognise that a discriminating input declaring `not_collected` neither supports nor
    /// refutes, instead of coercing the absence to a negative call.
    TreatDeclaredAbsenceAsNonEvidence,
    /// Count the surviving hypotheses and detect that more than one remains under the exclusion.
    DetectMoreThanOneLiveHypothesis,
    /// Construct `OracleVerdict::abstain` rather than deriving status from an empty witness list.
    ConstructTheAbstainVerdict,
    /// Populate the refinement frontier from `discriminating_evidence_request`, so the abstention
    /// names the study that would settle it instead of merely declining.
    PopulateTheRefinementFrontierFromTheEvidenceRequest,
}

impl AbstentionStep {
    pub const ALL: [AbstentionStep; 6] = [
        AbstentionStep::InterpretTheMutualExclusionFactor,
        AbstentionStep::EvaluateEachHypothesisSupportFactor,
        AbstentionStep::TreatDeclaredAbsenceAsNonEvidence,
        AbstentionStep::DetectMoreThanOneLiveHypothesis,
        AbstentionStep::ConstructTheAbstainVerdict,
        AbstentionStep::PopulateTheRefinementFrontierFromTheEvidenceRequest,
    ];

    /// Whether the step needs a change to `bioprism-fiber` rather than to a world.
    ///
    /// Every one of them still does. `bioprism_fiber::compile_with_oracle` now lets an injected
    /// oracle construct `OracleVerdict::abstain` — `bioprism-domain`'s rule oracles do exactly
    /// that — but that contract hands an oracle the compiled value map only, and abstaining on
    /// *this* world requires first reading its hypothesis and exclusion factors, which no
    /// injected oracle can see. Lifting these steps therefore still means changing the compiler
    /// contract rather than building another world.
    pub const fn requires_a_compiler_change(self) -> bool {
        true
    }

    pub const fn describe(self) -> &'static str {
        match self {
            AbstentionStep::InterpretTheMutualExclusionFactor => {
                "read the mutual_exclusion_rule factor as a constraint over its inputs, not as another relation to evaluate"
            }
            AbstentionStep::EvaluateEachHypothesisSupportFactor => {
                "evaluate every hypothesis_support_rule factor against the selected evidence"
            }
            AbstentionStep::TreatDeclaredAbsenceAsNonEvidence => {
                "treat a discriminating input declaring not_collected as neither support nor refutation"
            }
            AbstentionStep::DetectMoreThanOneLiveHypothesis => {
                "count surviving hypotheses under the exclusion and detect that more than one remains"
            }
            AbstentionStep::ConstructTheAbstainVerdict => {
                "construct OracleVerdict::abstain instead of deriving status from an empty witness list"
            }
            AbstentionStep::PopulateTheRefinementFrontierFromTheEvidenceRequest => {
                "populate the refinement frontier from discriminating_evidence_request so the abstention names the study that would settle it"
            }
        }
    }
}
