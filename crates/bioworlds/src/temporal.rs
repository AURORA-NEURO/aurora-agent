//! Reference world 8: trial eligibility and the temporal evidence firewall.
//!
//! Blueprint 38.08 asks for a world that evaluates protocol criteria "using only evidence
//! available at a historical screening time", and 43.09 fixes the mechanism: availability is a
//! partially ordered event structure in which `event_time` and `availability_time` are separate
//! fields. This world exists to close one specific, written-down gap.
//!
//! # The gap
//!
//! `crates/examples` records `non_protected_temporal_withholding` as blocked, and states the
//! obstacle precisely:
//!
//! > in the generated family every event-managed variable (`training_decision_time`,
//! > `split_assignment`, `preprocess_fit_scope`) is also protected, so an early cut cannot
//! > withhold evidence without simultaneously breaking the closure; separating them needs an
//! > event over a non-protected variable that the target depends on.
//!
//! That is the entire specification for this world, and it is met literally.
//! [`CENTRAL_LAB_CONFIRMATION`] is produced by an event whose `availability_time` falls after the
//! screening cut, carries the tag `assay_result` which is *not* in the query's protected
//! vocabulary, and is an input to `factor.lab_window_check`, which is an input to the target. So
//! a decision taken at the cut cannot read it, and the protected closure is untouched: every
//! protected fact in this world is either unmanaged by any event or governed by an event released
//! at or before the cut. Withholding and closure violation are, here, two separable failures.
//!
//! `PROTOCOL_AMENDMENT_TEXT` is a second instance with a different flavour — a protocol amendment
//! issued and released after screening — because one instance could be an accident of tagging and
//! two make the pattern legible. §38.08 names *amendment date* as a mutation family.
//!
//! # What this crate can and cannot show
//!
//! It can show the structure: reachable, event-managed, unreleased, unprotected. It cannot show a
//! compiled verdict — `bioprism-fiber` is not in the dependency set, on purpose. The claim made
//! here is therefore "this world makes the blocked property *exercisable*", not "this world
//! demonstrates the property". Wiring it to the compiler is the next step and belongs to whoever
//! owns that dependency.
//!
//! # Vocabulary
//!
//! §38.08 names artefact classes — protocol and amendments, timelines of reports and labs,
//! release timestamps — and no concrete criteria, assays or thresholds. The criterion names,
//! window lengths and lab identifiers below are therefore **illustrative**: they exercise the
//! machinery and are not domain knowledge. The recognisably clinical world in this crate is
//! [`crate::underdetermined`], which uses `bioprism-onco`'s own vocabulary.
//!
//! # Not implemented
//!
//! No criteria parser, no interval arithmetic, no free-text artefacts, and no oracle: §38.08's
//! Oracle Mesh has no wire representation in `fiber-world/0.1`, and a world here is facts, typed
//! factors and events.

use crate::builder::{per_subject, subject_ids, BioWorld, WorldBuilder};
use crate::error::BioWorldError;
use crate::query::{tag_set, QueryShape};
use bioprism_worldgen::rng::SplitMix64;
use bioprism_worldgen::spec::{DistractorAttachment, TagStyle};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// The decision variable: a criterion ledger, never an enrolment recommendation (§38.08's
/// required final deliverable).
pub const TARGET: &str = "eligibility_criterion_status";

/// The variable the firewall withholds. Reachable from the target, event-managed, unprotected.
pub const CENTRAL_LAB_CONFIRMATION: &str = "central_lab_confirmation";

/// A second withheld, unprotected variable: the text of a post-screening amendment.
pub const PROTOCOL_AMENDMENT_TEXT: &str = "protocol_amendment_text";

/// The control: event-managed and unprotected like the two above, but *released* before the cut.
/// Without it, "event-managed and unprotected" and "withheld" would be indistinguishable in this
/// world, and the claim would be about tagging rather than about time.
pub const LOCAL_LAB_VALUE: &str = "local_lab_value";

/// The tag distractor facts carry.
pub const DISTRACTOR_TAG: &str = "exploratory";

/// The screening cut. Deliberately equal to `event.screening_open`'s availability time: `<=` is
/// readable, so the protected releases land exactly on the boundary and the world still has a
/// complete closure.
pub const SCREENING_CUT: &str = "2025-03-01T00:00:00Z";

/// Tags that tokenise into the protected vocabulary without being protected tags.
///
/// The technique is `bioprism_worldgen::spec::TagStyle::Camouflaged`, reused rather than
/// reinvented; the vocabulary differs because this world's protected tags differ. Closure matches
/// whole tags, so `protocol_summary` is correctly outside it, while a tokenising retriever scores
/// it against a query naming `protocol`.
const CAMOUFLAGE_TAGS: [&str; 6] = [
    "protocol_summary",
    "time_summary",
    "policy_summary",
    "identity_summary",
    "treatment_summary",
    "evidence_summary",
];

/// The five criterion checks. Fixed, so that structural variation is the only thing a spec moves.
const CHECKS: [(&str, &str, &[&str]); 5] = [
    (
        "factor.identity_check",
        "identity_criterion_status",
        &["subject_registry", "screening_decision_time"],
    ),
    (
        "factor.lab_window_check",
        "lab_criterion_status",
        &[
            LOCAL_LAB_VALUE,
            CENTRAL_LAB_CONFIRMATION,
            "lab_window_days",
            "screening_decision_time",
        ],
    ),
    (
        "factor.prior_treatment_check",
        "prior_treatment_criterion_status",
        &["prior_treatment_record", "criterion_table"],
    ),
    (
        "factor.protocol_version_check",
        "protocol_criterion_status",
        &["protocol_version", PROTOCOL_AMENDMENT_TEXT],
    ),
    (
        "factor.consent_check",
        "consent_criterion_status",
        &["consent_scope"],
    ),
];

/// The structural knobs this world varies.
///
/// `attachment` and `tag_style` are `bioprism-worldgen`'s own enums, imported rather than
/// redeclared: the point of §38 is more worlds, not a second vocabulary for the same three ideas.
/// What is *not* here, and what `worldgen` also lacks, is enumerated in [`crate::knobs`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalFirewallSpec {
    pub world_id: String,
    pub subjects: usize,
    pub distractors: usize,
    pub relay_depth: usize,
    pub attachment: DistractorAttachment,
    pub tag_style: TagStyle,
    pub seed: u64,
}

impl TemporalFirewallSpec {
    /// The shipped world: distractors near the target, decisive facts behind a relay chain,
    /// camouflaged tags. The corner of the space `docs/FINDINGS.md` found separable.
    pub fn discriminating() -> Self {
        TemporalFirewallSpec {
            world_id: "onco-world-trial-eligibility-firewall-v1".into(),
            subjects: 96,
            distractors: 120,
            relay_depth: 3,
            attachment: DistractorAttachment::NearTarget,
            tag_style: TagStyle::Camouflaged,
            seed: 20_260_808,
        }
    }

    /// The control: the reference world's corner — hub attachment, no relays, distinct tags — with
    /// the identical temporal structure.
    ///
    /// It exists so the temporal claim can be shown to be independent of the discrimination claim.
    /// Its separating depth is expected to *exist*, which is the property `docs/FINDINGS.md`
    /// blames for the reference benchmark measuring nothing. Shipping it is the point; a section
    /// that only shipped its favourable world would be the thing §38 is meant to prevent.
    pub fn reference_shaped() -> Self {
        TemporalFirewallSpec {
            world_id: "onco-world-trial-eligibility-firewall-control-v1".into(),
            subjects: 96,
            distractors: 120,
            relay_depth: 0,
            attachment: DistractorAttachment::Hub,
            tag_style: TagStyle::Distinct,
            seed: 20_260_808,
        }
    }

    /// The cohort scale 38.01 specifies, 80–200 subjects.
    pub fn is_at_cohort_scale(&self) -> bool {
        (80..=200).contains(&self.subjects)
    }
}

/// Builds the world.
///
/// Deterministic in the spec, including the seed: no clock is read and no system RNG is used, so
/// the same spec always yields byte-identical bytes and the shipped fixture is a meaningful
/// artefact rather than a snapshot of one machine.
pub fn build(spec: &TemporalFirewallSpec) -> Result<BioWorld, BioWorldError> {
    let subjects = subject_ids(spec.subjects);
    let mut rng = SplitMix64::new(spec.seed);

    let mut builder = WorldBuilder::new(
        spec.world_id.clone(),
        format!(
            "Trial-eligibility temporal firewall (38.08): relay_depth={}, attachment={:?}, tags={:?}, distractors={}, subjects={}. \
             The withheld variables are non-protected by construction, so a screening-time cut separates temporal withholding from a protected-closure violation.",
            spec.relay_depth, spec.attachment, spec.tag_style, spec.distractors, spec.subjects
        ),
        "TE-FIREWALL-001",
    );

    builder
        .fact(
            "fact.subject_registry",
            "subject_registry",
            per_subject(&subjects, |index, subject| {
                json!({ "subject": subject, "screening_order": index + 1 })
            }),
            &["identity", "protected"],
            &["manifest/screening_registry.csv"],
        )
        .fact(
            "fact.screening_cut",
            "screening_decision_time",
            json!(SCREENING_CUT),
            &["time", "protected"],
            &["protocol/screening_protocol.md"],
        )
        .fact(
            "fact.protocol_version",
            "protocol_version",
            json!("PROTO-7.2"),
            &["protocol", "protected"],
            &["protocol/PROTO-7.2.pdf"],
        )
        .fact(
            "fact.criterion_table",
            "criterion_table",
            json!({ "inclusion": 6, "exclusion": 11, "source": "PROTO-7.2 §4" }),
            &["protocol", "protected"],
            &["protocol/PROTO-7.2.pdf"],
        )
        .fact(
            "fact.lab_window",
            "lab_window_days",
            json!(28),
            &["protocol", "protected"],
            &["protocol/PROTO-7.2.pdf"],
        )
        .fact(
            "fact.consent_scope",
            "consent_scope",
            json!("research-only"),
            &["policy", "protected"],
            &["governance/consent_scope.md"],
        )
        .fact(
            "fact.prior_treatment",
            "prior_treatment_record",
            per_subject(&subjects, |index, _| {
                json!({ "prior_lines": index % 3, "documented": true })
            }),
            &["treatment_history", "protected"],
            &["manifest/prior_treatment.csv"],
        )
        .fact(
            "fact.missing_evidence_screen",
            "declared_missing_evidence_screen",
            json!("no_screen_for_undocumented_prior_lines_performed"),
            &["negative_evidence", "protected"],
            &["manifest/qc.md"],
        );

    builder
        .fact(
            "fact.local_lab",
            LOCAL_LAB_VALUE,
            per_subject(&subjects, |index, _| {
                json!({ "drawn": "2025-02-10", "value": 100 + (index % 40) })
            }),
            &["assay_result"],
            &["labs/local_panel_2025-02-10.csv"],
        )
        .fact(
            "fact.central_lab",
            CENTRAL_LAB_CONFIRMATION,
            per_subject(&subjects, |index, _| {
                json!({ "specimen_drawn": "2025-02-20", "confirmed": index % 5 != 0 })
            }),
            &["assay_result"],
            &["labs/central_confirmation_2025-04-10.csv"],
        )
        .fact(
            "fact.protocol_amendment",
            PROTOCOL_AMENDMENT_TEXT,
            json!({ "amendment": "PROTO-7.3", "changes_criterion": "lab window 28 -> 42 days" }),
            &["amendment"],
            &["protocol/PROTO-7.3-amendment.pdf"],
        );

    let mut ledger_inputs: Vec<String> = Vec::new();
    for (check_id, terminal, inputs) in CHECKS {
        let check_output = if spec.relay_depth == 0 {
            terminal.to_string()
        } else {
            format!("{terminal}_r0")
        };
        builder.factor(
            check_id,
            inputs,
            &[&check_output],
            "deterministic_rule",
            &["protected"],
            1.0,
        );

        if spec.relay_depth > 0 {
            let mut previous = check_output;
            for step in 1..=spec.relay_depth {
                let output = if step == spec.relay_depth {
                    terminal.to_string()
                } else {
                    format!("{terminal}_r{step}")
                };
                builder.factor(
                    &format!("factor.relay.{terminal}.{step}"),
                    &[&previous],
                    &[&output],
                    "relay_rule",
                    &["relay"],
                    0.05,
                );
                previous = output;
            }
        }
        ledger_inputs.push(terminal.to_string());
    }

    ledger_inputs.push("declared_missing_evidence_screen".to_string());
    let ledger_refs: Vec<&str> = ledger_inputs.iter().map(String::as_str).collect();
    builder.factor(
        "factor.criterion_ledger",
        &ledger_refs,
        &[TARGET],
        "decision_rule",
        &["target"],
        0.2,
    );

    let attachment = match spec.attachment {
        DistractorAttachment::Hub => "subject_registry".to_string(),
        DistractorAttachment::NearTarget => "identity_criterion_status".to_string(),
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
            &["analysis/screening_exploratory.ipynb"],
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
            "event.screening_open",
            "2025-03-01T00:00:00Z",
            "2025-03-01T00:00:00Z",
            &[],
            &["screening_decision_time", "protocol_version"],
        )
        .event(
            "event.local_lab_release",
            "2025-02-10T00:00:00Z",
            "2025-02-12T00:00:00Z",
            &[],
            &[LOCAL_LAB_VALUE],
        )
        .event(
            "event.central_lab_release",
            "2025-02-20T00:00:00Z",
            "2025-04-10T00:00:00Z",
            &["event.local_lab_release"],
            &[CENTRAL_LAB_CONFIRMATION],
        )
        .event(
            "event.protocol_amendment",
            "2025-03-14T00:00:00Z",
            "2025-03-21T00:00:00Z",
            &["event.screening_open"],
            &[PROTOCOL_AMENDMENT_TEXT],
        );

    builder.build()
}

/// The query shape §38.08 implies: a criterion ledger compiled at the historical screening time.
///
/// `assay_result` and `amendment` are pointedly absent from the protected vocabulary. That absence
/// is the whole construction: it is what lets the cut withhold `central_lab_confirmation` without
/// touching the closure.
pub fn query(spec: &TemporalFirewallSpec) -> QueryShape {
    QueryShape {
        query_id: format!("{}-eligibility-criterion-ledger", spec.world_id),
        targets: vec![TARGET.to_string()],
        protected_tags: tag_set(&[
            "identity",
            "time",
            "protocol",
            "policy",
            "treatment_history",
            "negative_evidence",
            "protected",
        ]),
        decision_time: SCREENING_CUT.to_string(),
        max_facts: 64,
        max_tokens: 6000,
        role: "research-auditor".into(),
        policy: vec!["research-only".into()],
    }
}
