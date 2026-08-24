//! The structural knobs.
//!
//! Blueprint 43.39: "Generate controlled worlds that vary mathematical structure independently, so
//! the platform can test exactly when each representation and backend succeeds or fails."
//!
//! Each field below is a dimension along which a context strategy can be made to succeed or fail
//! *independently of the others*. The reference world sits at one corner of this space — hub
//! attachment, no relays, distinct tags — which is precisely why it fails to discriminate FIBER
//! from a tuned graph walk or a lexical retriever. See `docs/FINDINGS.md`.
//!
//! # The seven knobs added after `crates/bioworlds` enumerated them
//!
//! `crates/bioworlds` built four worlds by hand and recorded, in its `knobs` module, six
//! dimensions this crate could not express, each with the field that would close it. A seventh
//! arrived with `bioprism_fiber::policy`, which screens facts by their `policy` scope dimension
//! and had no world in the workspace that declared one. All seven are now fields:
//! [`WorldSpec::events`], [`WorldSpec::protected_variables`], [`WorldSpec::decision_time`],
//! [`WorldSpec::skeleton`], [`WorldSpec::hypotheses`], [`WorldSpec::declared_absent`] and
//! [`WorldSpec::policy`].
//!
//! Every one of them defaults to the behaviour the generator had before it existed, and the
//! defaults are load-bearing rather than polite: `fixtures/generated/` holds two generated worlds
//! as committed bytes, `crates/bioworlds` reads one of them to state a claim about the family, and
//! a spec serialised before this change must still deserialise. A knob that moved the default
//! world would invalidate all three at once.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Where distractor factors attach to the decisive dependency chain.
///
/// This is the knob that decides whether a *separating depth* exists for neighbourhood traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistractorAttachment {
    /// Attach to `cohort_id`, a decisive leaf. Distractors then sit two hops *beyond* the decisive
    /// facts, so some depth reaches the decisive set without them. The reference world's shape.
    Hub,
    /// Attach near the target, above the relay chain. Distractors are then at most as far from the
    /// target as the decisive facts, so no depth admits the decisive set while excluding them.
    NearTarget,
}

/// How distractor facts are tagged and named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagStyle {
    /// Distractors are tagged `exploratory`. A lexical retriever scoring the query's protected
    /// tags separates them trivially. The reference world's shape.
    Distinct,
    /// Distractors carry tags that *tokenise* to the protected vocabulary — `identity_summary`,
    /// `split_summary` — without being protected tags themselves.
    ///
    /// This defeats lexical scoring without touching protected-closure semantics: closure matches
    /// whole tags, so `identity_summary` is correctly not protected, while BM25 tokenises it to
    /// `identity` + `summary` and scores it against the query.
    Camouflaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageMechanism {
    Identity,
    Site,
    Temporal,
    Preprocessing,
}

impl LeakageMechanism {
    pub const ALL: [LeakageMechanism; 4] = [
        LeakageMechanism::Identity,
        LeakageMechanism::Site,
        LeakageMechanism::Temporal,
        LeakageMechanism::Preprocessing,
    ];
}

/// The marker tag a protected variable's fact carries.
pub const PROTECTED_TAG: &str = "protected";

/// The tag the external-confirmation skeleton's two lab facts carry.
///
/// Pointedly absent from [`PROTECTED_VOCABULARY`]. That absence is the whole construction: it is
/// what lets the temporal cut withhold `central_lab_confirmation` without touching the mandatory
/// closure of 43.13, which is the separation `crates/examples` records as blocked.
pub const ASSAY_TAG: &str = "assay_result";

/// A lab value released before the decision cut. The control for [`CENTRAL_LAB_CONFIRMATION`].
///
/// Without it, "event-managed and unprotected" and "withheld" would be indistinguishable in the
/// generated world and the claim would be about tagging rather than about time.
pub const LOCAL_LAB_VALUE: &str = "local_lab_value";

/// A lab confirmation assayed before the cut and released after it. The variable a spec withholds.
pub const CENTRAL_LAB_CONFIRMATION: &str = "central_lab_confirmation";

/// The variable a declared exclusion between competing hypotheses produces.
pub const HYPOTHESIS_EXCLUSION: &str = "hypothesis_exclusion";

/// The decision cut every generated query has carried since the crate existed.
pub const REFERENCE_DECISION_TIME: &str = "2025-01-01T00:00:00Z";

/// The tags a generated query calls protected.
///
/// Fixed rather than derived from [`WorldSpec::protected_variables`], and the reason is worth
/// stating: the vocabulary and the protected set do not coincide in the reference shape.
/// `fact.cohort` carries `identity` without being in the set, and `fact.label_source` carries
/// `label_lineage`, which the vocabulary does not name, so it is protected only by the
/// [`PROTECTED_TAG`] marker. Deriving one from the other would have to break one of those two.
pub const PROTECTED_VOCABULARY: [&str; 10] = [
    "identity",
    "split",
    "site",
    "scanner",
    "time",
    "specimen",
    "preprocessing",
    "policy",
    "negative_evidence",
    "protected",
];

/// One release in the world's event structure (43.09).
///
/// Closes `event_managed_variable_set`. Before this existed `generate_world` emitted two events
/// with hard-coded `produces` lists, so no spec could put an event over a variable of its
/// choosing, and `crates/examples` recorded `non_protected_temporal_withholding` as blocked partly
/// for that reason.
///
/// `event_time` and `availability_time` are separate fields because 43.09 says they are: a
/// specimen assayed in December and released in April is not readable by a January decision, and
/// collapsing the two is the temporal-leakage defect the oracle exists to catch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSpec {
    pub id: String,
    /// When the thing happened.
    pub event_time: String,
    /// When a reader could first see it. The temporal cut reads this field and not the other.
    pub availability_time: String,
    #[serde(default)]
    pub causal_parents: Vec<String>,
    /// The variables this event governs. A variable no event produces was always available.
    pub produces: Vec<String>,
}

impl EventSpec {
    pub fn new(
        id: impl Into<String>,
        event_time: impl Into<String>,
        availability_time: impl Into<String>,
        causal_parents: &[&str],
        produces: &[&str],
    ) -> Self {
        EventSpec {
            id: id.into(),
            event_time: event_time.into(),
            availability_time: availability_time.into(),
            causal_parents: causal_parents.iter().map(|p| (*p).to_string()).collect(),
            produces: produces.iter().map(|p| (*p).to_string()).collect(),
        }
    }

    /// The two events the generator hard-coded before [`WorldSpec::events`] existed.
    ///
    /// Every variable here is also in [`WorldSpec::reference_protected_variables`] except
    /// `future_label_value`, which is exactly the collapse `crates/bioworlds` named: the family
    /// could not make a variable event-managed without also making it protected, unless that
    /// variable was one the target does not depend on.
    pub fn reference_release_schedule() -> Vec<EventSpec> {
        vec![
            EventSpec::new(
                "event.training",
                "2025-01-01T00:00:00Z",
                "2025-01-01T00:00:00Z",
                &[],
                &[
                    "training_decision_time",
                    "split_assignment",
                    "preprocess_fit_scope",
                ],
            ),
            EventSpec::new(
                "event.future_label",
                "2025-06-01T00:00:00Z",
                "2025-06-15T00:00:00Z",
                &["event.training"],
                &["future_label_value"],
            ),
        ]
    }

    /// The reference schedule plus two lab releases, one on each side of the cut.
    ///
    /// Pairs with [`Skeleton::ExternalConfirmation`], whose sixth check consumes both variables.
    /// `event.central_lab_release` is assayed in December and released in April, so a decision
    /// taken at [`REFERENCE_DECISION_TIME`] cannot read it; `event.local_lab_release` is released
    /// before the cut and can, which is what makes the withholding a statement about the release
    /// schedule rather than about the tag vocabulary.
    pub fn lab_release_schedule() -> Vec<EventSpec> {
        let mut schedule = EventSpec::reference_release_schedule();
        schedule.push(EventSpec::new(
            "event.local_lab_release",
            "2024-11-20T00:00:00Z",
            "2024-11-22T00:00:00Z",
            &[],
            &[LOCAL_LAB_VALUE],
        ));
        schedule.push(EventSpec::new(
            "event.central_lab_release",
            "2024-12-20T00:00:00Z",
            "2025-04-10T00:00:00Z",
            &["event.local_lab_release"],
            &[CENTRAL_LAB_CONFIRMATION],
        ));
        schedule
    }
}

/// One check in a decisive skeleton: a factor, the terminal it produces, and what it reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckSpec {
    pub factor_id: &'static str,
    pub terminal: &'static str,
    pub inputs: &'static [&'static str],
}

const SPLIT_INTEGRITY_CHECKS: [CheckSpec; 5] = [
    CheckSpec {
        factor_id: "factor.identity_check",
        terminal: "identity_leakage",
        inputs: &[
            "cohort_id",
            "subject_aliases",
            "split_assignment",
            "declared_duplicate_screen",
        ],
    },
    CheckSpec {
        factor_id: "factor.site_check",
        terminal: "site_leakage",
        inputs: &["site_assignment", "scanner_assignment", "split_assignment"],
    },
    CheckSpec {
        factor_id: "factor.temporal_check",
        terminal: "temporal_leakage",
        inputs: &[
            "label_source_time",
            "training_decision_time",
            "specimen_dates",
            "split_assignment",
        ],
    },
    CheckSpec {
        factor_id: "factor.preprocessing_check",
        terminal: "preprocessing_leakage",
        inputs: &["preprocess_fit_scope", "split_assignment"],
    },
    CheckSpec {
        factor_id: "factor.policy_check",
        terminal: "policy_validity",
        inputs: &["data_policy"],
    },
];

const EXTERNAL_CONFIRMATION_CHECKS: [CheckSpec; 6] = [
    SPLIT_INTEGRITY_CHECKS[0],
    SPLIT_INTEGRITY_CHECKS[1],
    SPLIT_INTEGRITY_CHECKS[2],
    SPLIT_INTEGRITY_CHECKS[3],
    SPLIT_INTEGRITY_CHECKS[4],
    CheckSpec {
        factor_id: "factor.confirmation_check",
        terminal: "confirmation_validity",
        inputs: &[LOCAL_LAB_VALUE, CENTRAL_LAB_CONFIRMATION, "split_assignment"],
    },
];

/// Which decisive structure the world is built around.
///
/// Closes `alternative_decisive_skeleton`. The check table and the target were consts, so
/// `crates/bioworlds` built its worlds directly rather than parameterising this generator.
///
/// # Why there is no `Custom` variant
///
/// A `Custom { target, checks }` variant is easy to write and would be dishonest. The verdict on
/// every generated world comes from `bioprism_fiber::oracle`, which is
/// `deterministic_split_integrity_v1`: it recognises `subject_aliases`, `split_assignment`,
/// `site_assignment`, `label_source_time`, `training_decision_time` and `preprocess_fit_scope` and
/// nothing else. A world with a genuinely different decision would compile, return `Valid` with an
/// empty witness list, and look like a clean world rather than an unjudged one. Both variants here
/// therefore keep the split-integrity target and differ in what feeds it, and a decision the
/// oracle does not know needs an oracle before it needs a variant. That oracle now exists —
/// `bioprism-domain` declares rule oracles that judge a compile through
/// `bioprism_fiber::compile_with_oracle` — so the variant question is open again on its merits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Skeleton {
    /// The five split-integrity checks. The shape every generated world had before this knob.
    #[default]
    SplitIntegrity,
    /// The five checks plus a confirmation check reading two lab variables tagged [`ASSAY_TAG`].
    ///
    /// Those two facts are decisive — the target depends on them through
    /// `factor.confirmation_check` — and carry no tag in [`PROTECTED_VOCABULARY`]. Combined with
    /// [`EventSpec::lab_release_schedule`] this is the structure `non_protected_temporal_withholding`
    /// asks for: withholding real evidence with the mandatory closure intact.
    ExternalConfirmation,
}

impl Skeleton {
    pub fn checks(self) -> &'static [CheckSpec] {
        match self {
            Skeleton::SplitIntegrity => &SPLIT_INTEGRITY_CHECKS,
            Skeleton::ExternalConfirmation => &EXTERNAL_CONFIRMATION_CHECKS,
        }
    }

    /// The variable the query decides.
    pub fn target(self) -> &'static str {
        "split_integrity_status"
    }
}

/// Evidence recorded as *declared absent* rather than simply missing (43.28).
///
/// Closes `declared_absence_records`. `fact.negative_duplicates` was a single hard-coded negative
/// -evidence fact, and nothing parameterised which evidence a world records this way — which is
/// what separates an underdetermined world from one that is merely short of data.
///
/// # What the wire format cannot say
///
/// The proposal in `crates/bioworlds` asks for a value carrying an `ObservationStatus`.
/// `fiber-world/0.1` has no such type and no place to register one, so the status *is* the value:
/// a fact whose value is the string `no_duplicate_screen_performed` states that a screen was not
/// performed, and a reader distinguishes it from data by the `negative_evidence` tag. Inventing a
/// richer shape here would put a field on the wire that no schema declares and no pass reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredAbsence {
    pub fact_id: String,
    pub provides: String,
    /// The recorded absence, which is the fact's value.
    pub status: String,
    pub provenance: Vec<String>,
}

impl DeclaredAbsence {
    pub fn new(
        fact_id: impl Into<String>,
        provides: impl Into<String>,
        status: impl Into<String>,
        provenance: &[&str],
    ) -> Self {
        DeclaredAbsence {
            fact_id: fact_id.into(),
            provides: provides.into(),
            status: status.into(),
            provenance: provenance.iter().map(|p| (*p).to_string()).collect(),
        }
    }

    /// The one record the generator hard-coded before this knob existed.
    pub fn reference_records() -> Vec<DeclaredAbsence> {
        vec![DeclaredAbsence::new(
            "fact.negative_duplicates",
            "declared_duplicate_screen",
            "no_duplicate_screen_performed",
            &["manifest/qc.md"],
        )]
    }
}

/// What the corpus was released under, what the caller accepts, and what each fact requires (43.33).
///
/// The seventh knob, and the one `crates/examples` names against `policy_blocked_omission`:
/// `bioprism_fiber::policy` screens facts by their `policy` scope dimension and emits
/// `InfluenceClass::InaccessibleByPolicy`, but no world this crate generated declared a
/// requirement, so the pass had nothing to withhold and the class had no producer.
///
/// The three fields are the three places `fiber-world/0.1` and `fiber-query/0.1` can carry access
/// control without inventing schema: the world's `data_policy` variable, the query's `policy`
/// list, and a fact's `policy` scope dimension. A clause is an obligation and accepting it unlocks
/// evidence, so [`PolicySpec::accepted`] is what the caller undertakes, not a description of the
/// data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicySpec {
    /// The clause set the corpus declares it was released under: the `data_policy` fact's value.
    ///
    /// A caller may only accept clauses this list grants; anything else is
    /// `PolicyViolation::Conflict` and the compile refuses, because a grant no source issued
    /// cannot be honoured.
    #[serde(default = "PolicySpec::research_only")]
    pub governing: Vec<String>,
    /// The obligations the generated query accepts, and therefore the grants a compile holds.
    #[serde(default = "PolicySpec::research_only")]
    pub accepted: Vec<String>,
    /// Variable to the clauses a reader must hold before that variable's fact may be released.
    ///
    /// Bound on the fact's `policy` scope dimension, which `bioprism_scope` already registers as
    /// policy-class. Read as a *conjunction*: a fact released under two restrictions requires both.
    #[serde(default)]
    pub requirements: BTreeMap<String, Vec<String>>,
}

impl PolicySpec {
    fn research_only() -> Vec<String> {
        vec!["research-only".to_string()]
    }

    /// A requirement bound on one variable, with the clause added to the governing set.
    ///
    /// Both halves are needed for the requirement to be *satisfiable*: a clause the corpus never
    /// granted withholds the fact from every caller, since declaring it is refused as a conflict.
    /// A knob that could only produce unreachable evidence would not demonstrate a policy screen,
    /// it would demonstrate a dead end.
    pub fn requiring(variable: impl Into<String>, clause: impl Into<String>) -> Self {
        let clause = clause.into();
        let mut governing = PolicySpec::research_only();
        governing.push(clause.clone());
        PolicySpec {
            governing,
            accepted: PolicySpec::research_only(),
            requirements: BTreeMap::from([(variable.into(), vec![clause])]),
        }
    }

    /// The same world with the caller accepting `clause`, so the screen releases what it withheld.
    pub fn accepting(mut self, clause: impl Into<String>) -> Self {
        self.accepted.push(clause.into());
        self
    }
}

impl Default for PolicySpec {
    fn default() -> Self {
        PolicySpec {
            governing: PolicySpec::research_only(),
            accepted: PolicySpec::research_only(),
            requirements: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSpec {
    pub world_id: String,
    /// Number of subjects in the cohort.
    pub subjects: usize,
    /// Number of irrelevant exploratory facts.
    pub distractors: usize,
    /// Relay factors inserted between each check factor and the target, pushing decisive facts
    /// further from the target without changing what they mean.
    pub relay_depth: usize,
    pub attachment: DistractorAttachment,
    pub tag_style: TagStyle,
    /// Which leakage defects to inject. An empty list produces a clean, valid split.
    pub leakage: Vec<LeakageMechanism>,
    pub seed: u64,

    /// The release schedule: which variables each event governs, and when a reader may see them.
    ///
    /// Defaults to [`EventSpec::reference_release_schedule`], the two events the generator used to
    /// hard-code.
    #[serde(default = "EventSpec::reference_release_schedule")]
    pub events: Vec<EventSpec>,

    /// Which variables' facts carry [`PROTECTED_TAG`], independently of which are event-managed.
    ///
    /// Closes `protection_set_independent_of_events`. The tag was a literal per fact and every
    /// event-managed variable happened to carry it, so an early cut could not withhold evidence
    /// without simultaneously breaking the closure — the recorded obstacle behind
    /// `non_protected_temporal_withholding`.
    ///
    /// Removing a variable from this set drops the marker tag *and* the topic tag the query
    /// protects, because a fact still tagged `split` would be pulled back into the closure by
    /// [`PROTECTED_VOCABULARY`] and the knob would appear to do nothing. The tag such a fact
    /// carries instead is declared per fact in `generate`, not derived, because two of the core
    /// facts are already outside the vocabulary and have to keep the tag they have.
    #[serde(default = "WorldSpec::reference_protected_variables")]
    pub protected_variables: BTreeSet<String>,

    /// Where the decision cut falls, relative to the releases in [`WorldSpec::events`].
    ///
    /// Closes `release_schedule_and_cut_placement`. Both this and the availability times were
    /// literals, so the cut could not move and no spec in the family withheld anything.
    #[serde(default = "WorldSpec::reference_decision_time")]
    pub decision_time: String,

    /// Which decisive structure the world is built around.
    #[serde(default)]
    pub skeleton: Skeleton,

    /// Competing terminal hypotheses, declared mutually exclusive.
    ///
    /// Closes `competing_hypothesis_terminals`. `factor.claim_support` produced a single terminal
    /// and there was no way to declare two of them exclusive, so no spec could leave a verdict
    /// open. Empty by default, which is the single-terminal shape.
    ///
    /// The world can now *state* the exclusion, as a factor of kind `exclusion_rule` reachable
    /// from the target. Nothing reads it: `bioprism_section::OracleVerdict` can represent
    /// abstention and no path in `bioprism-fiber` constructs it, so
    /// `underdetermined_abstention` stays blocked in the compiler. This knob moves the blocker off
    /// the world and onto the pass, which is where `crates/examples` already says it lives.
    #[serde(default)]
    pub hypotheses: Vec<String>,

    /// Evidence the world records as declared absent.
    ///
    /// Defaults to [`DeclaredAbsence::reference_records`], the single hard-coded
    /// `fact.negative_duplicates`. Records beyond those a check already consumes become inputs to
    /// the claim factor, so a declared absence is decisive rather than decorative.
    #[serde(default = "DeclaredAbsence::reference_records")]
    pub declared_absent: Vec<DeclaredAbsence>,

    /// Policy: what the corpus grants, what the caller accepts, what each fact requires.
    #[serde(default)]
    pub policy: PolicySpec,
}

impl WorldSpec {
    /// The variables whose facts carried [`PROTECTED_TAG`] before the knob existed.
    ///
    /// Ten of the twelve core facts. `cohort_id` and `future_label_value` are absent, which is why
    /// `future_label_value` was the family's only unprotected event-managed variable — and the
    /// target does not depend on it, so withholding it withheld nothing.
    pub fn reference_protected_variables() -> BTreeSet<String> {
        [
            "subject_aliases",
            "split_assignment",
            "site_assignment",
            "scanner_assignment",
            "label_source_time",
            "training_decision_time",
            "specimen_dates",
            "preprocess_fit_scope",
            "data_policy",
            "declared_duplicate_screen",
        ]
        .iter()
        .map(|v| (*v).to_string())
        .collect()
    }

    pub fn reference_decision_time() -> String {
        REFERENCE_DECISION_TIME.to_string()
    }

    /// The reference world's structural corner: hub attachment, no relays, distinct tags.
    pub fn reference_like(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-reference-like-v1".into(),
            subjects: 4,
            distractors,
            relay_depth: 0,
            attachment: DistractorAttachment::Hub,
            tag_style: TagStyle::Distinct,
            leakage: LeakageMechanism::ALL.to_vec(),
            seed: 20_260_808,
            events: EventSpec::reference_release_schedule(),
            protected_variables: WorldSpec::reference_protected_variables(),
            decision_time: WorldSpec::reference_decision_time(),
            skeleton: Skeleton::SplitIntegrity,
            hypotheses: Vec::new(),
            declared_absent: DeclaredAbsence::reference_records(),
            policy: PolicySpec::default(),
        }
    }

    /// A world built to discriminate.
    ///
    /// Distractors attach near the target and decisive facts sit behind a relay chain, so every
    /// depth that admits the decisive set also admits every distractor; and camouflaged tags deny
    /// a lexical retriever the shortcut of scoring the protected vocabulary.
    pub fn discriminating(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-discriminating-v1".into(),
            relay_depth: 3,
            attachment: DistractorAttachment::NearTarget,
            tag_style: TagStyle::Camouflaged,
            ..WorldSpec::reference_like(distractors)
        }
    }

    /// A world that withholds a decisive variable the closure does not protect.
    ///
    /// The deliverable for `non_protected_temporal_withholding`. `central_lab_confirmation` is an
    /// input to `factor.confirmation_check`, which feeds the target, and carries [`ASSAY_TAG`],
    /// which no query in this crate protects; the event releasing it becomes available months
    /// after [`WorldSpec::decision_time`]. A compile against this world therefore drops real
    /// evidence and still reports a complete protected closure — two failures that were previously
    /// impossible to separate in any world this crate could generate.
    ///
    /// `local_lab_value` is the control: identically tagged and identically event-managed, and
    /// released *before* the cut, so the withholding is demonstrably about the release schedule
    /// and not about the tag vocabulary.
    pub fn external_confirmation(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-external-confirmation-v1".into(),
            skeleton: Skeleton::ExternalConfirmation,
            events: EventSpec::lab_release_schedule(),
            ..WorldSpec::discriminating(distractors)
        }
    }

    /// A world whose decisive evidence includes a fact the caller is not entitled to read.
    ///
    /// The deliverable for `policy_blocked_omission`. `local_lab_value` requires the clause
    /// `consent-tier-2`, the corpus grants it, and the generated query accepts only
    /// `research-only`, so the policy screen withholds the fact and the certificate carries an
    /// `InaccessibleByPolicy` omission group. The variable is deliberately the one released
    /// *before* the cut, so the policy exclusion and the temporal exclusion land on different
    /// facts and each can be read on its own.
    ///
    /// Adding `consent-tier-2` to [`PolicySpec::accepted`] releases it, which is what makes this a
    /// policy decision rather than a missing fact.
    pub fn policy_restricted(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-policy-restricted-v1".into(),
            policy: PolicySpec::requiring(LOCAL_LAB_VALUE, "consent-tier-2"),
            ..WorldSpec::external_confirmation(distractors)
        }
    }

    pub fn with_world_id(mut self, id: impl Into<String>) -> Self {
        self.world_id = id.into();
        self
    }

    pub fn with_leakage(mut self, leakage: Vec<LeakageMechanism>) -> Self {
        self.leakage = leakage;
        self
    }

    /// Moves the cut without touching the release schedule, which is the experiment 43.09 implies.
    pub fn with_decision_time(mut self, at: impl Into<String>) -> Self {
        self.decision_time = at.into();
        self
    }

    /// Adds a variable to the protected set, turning a withholding into a closure violation.
    pub fn protecting(mut self, variable: impl Into<String>) -> Self {
        self.protected_variables.insert(variable.into());
        self
    }

    pub fn has(&self, mechanism: LeakageMechanism) -> bool {
        self.leakage.contains(&mechanism)
    }

    /// Whether `variable`'s fact carries [`PROTECTED_TAG`] and the topic tag the query protects.
    pub fn protects(&self, variable: &str) -> bool {
        self.protected_variables.contains(variable)
    }
}
