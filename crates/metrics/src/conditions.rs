//! The conditions a score was produced under — blueprint 33.01's stratification key, as data.
//!
//! 33.01 states the key literally:
//!
//! ```text
//! system version × architecture version × model version × pack version
//! × parent world × decision family × biological scale × modality
//! × disease entity × site/platform × population/time stratum
//! × oracle tier × budget × mutation family
//! ```
//!
//! and then: "The hub may aggregate only after preserving these coordinates." A number that has
//! lost its coordinates has not merely lost precision; it has lost the statement it was making.
//! [`MeasurementConditions`] is that statement, carried alongside every score in this crate.
//!
//! # Why every field is a three-state
//!
//! The temptation with conditions is `Option<String>` and `a == b`. Under that encoding two
//! measurements that each failed to record a pack version compare equal, and an unlabelled
//! comparison passes by default — which is the single easiest way to publish a rank between two
//! systems that were never evaluated on the same thing. So the field type is [`Condition`], and
//! [`Condition::Unrecorded`] on either side blocks a comparison rather than matching.
//!
//! This is a deliberate divergence from `bioprism_standards::comparable`, which lets two
//! *unbound* measurements through by default. The cases are genuinely different: an uncoded
//! millimetre is still a millimetre, whereas an unrecorded pack version means nobody knows which
//! instrument produced the number. Where a caller has out-of-band knowledge that two silences are
//! the same silence, [`crate::comparability::ComparabilityPolicy`] lets them waive a named
//! dimension, and the waiver is printed in the report.
//!
//! # Not implemented
//!
//! No condition *registry*, and no validation that a pack version exists. This crate holds the
//! coordinates and compares them; resolving them against a store is the hub's job. No scope
//! algebra either — `bioprism-scope` owns intersection and transport of scope keys, and a partial
//! reimplementation here would be a second, disagreeing definition of the same thing.

use crate::error::{ScoreIncomparability, UnrecordedSide};
use bioprism_atlas::{CapabilityId, OracleTier};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// The fourteen coordinates 33.01 requires every metric to be keyed by, in the blueprint's order.
///
/// Present as data so a caller can check a [`Stratum`] against the specification instead of
/// remembering it. Four of them have dedicated fields on [`MeasurementConditions`] because they
/// change what a comparison *means* rather than merely which slice it covers; the rest live in the
/// open [`Stratum`] map, since fixing them as struct fields would freeze a schema the store has
/// not chosen. `bioprism-atlas` declines the same key for the same reason.
pub const STRATIFICATION_KEY: &[&str] = &[
    "system version",
    "architecture version",
    "model version",
    "pack version",
    "parent world",
    "decision family",
    "biological scale",
    "modality",
    "disease entity",
    "site/platform",
    "population/time stratum",
    "oracle tier",
    "budget",
    "mutation family",
];

/// A recorded coordinate, or a stated absence.
///
/// The absence is a variant rather than a `None` so that it reads as a claim — "this was not
/// recorded" — in both the type and the JSON. `{"state":"unrecorded"}` in a stored condition is a
/// finding; a missing key would be an accident.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Condition<T> {
    Recorded { value: T },
    Unrecorded,
}

impl<T> Condition<T> {
    pub fn recorded(value: T) -> Self {
        Condition::Recorded { value }
    }

    pub fn is_recorded(&self) -> bool {
        matches!(self, Condition::Recorded { .. })
    }

    /// The value, or `None`. Named `recorded_value` rather than `unwrap_or_default` on purpose:
    /// there is no default for a coordinate nobody wrote down.
    pub fn recorded_value(&self) -> Option<&T> {
        match self {
            Condition::Recorded { value } => Some(value),
            Condition::Unrecorded => None,
        }
    }
}

/// Compares one condition dimension, returning the blocking reason if there is one.
///
/// The `Unrecorded` arms are the point of the function: they block, and they say which side was
/// silent, because "we did not record ours" and "they did not record theirs" are different repairs.
pub fn compare_condition<T: PartialEq + fmt::Display>(
    dimension: &str,
    left: &Condition<T>,
    right: &Condition<T>,
    differ: impl FnOnce(String, String) -> ScoreIncomparability,
) -> Result<(), ScoreIncomparability> {
    match (left, right) {
        (Condition::Recorded { value: a }, Condition::Recorded { value: b }) => {
            if a == b {
                Ok(())
            } else {
                Err(differ(a.to_string(), b.to_string()))
            }
        }
        (Condition::Unrecorded, Condition::Unrecorded) => {
            Err(ScoreIncomparability::ConditionUnrecorded {
                dimension: dimension.to_string(),
                side: UnrecordedSide::Both,
            })
        }
        (Condition::Unrecorded, Condition::Recorded { .. }) => {
            Err(ScoreIncomparability::ConditionUnrecorded {
                dimension: dimension.to_string(),
                side: UnrecordedSide::Left,
            })
        }
        (Condition::Recorded { .. }, Condition::Unrecorded) => {
            Err(ScoreIncomparability::ConditionUnrecorded {
                dimension: dimension.to_string(),
                side: UnrecordedSide::Right,
            })
        }
    }
}

/// Which way is better on a scale, and in what unit.
///
/// 33.01's release gate reads "Direction and units are unambiguous", and half of section 33's
/// metric list is a cost — latency, tissue consumption, coordination overhead, downstream regret.
/// A comparison that assumes higher-is-better silently inverts every one of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    HigherIsBetter,
    LowerIsBetter,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::HigherIsBetter => "higher_is_better",
            Direction::LowerIsBetter => "lower_is_better",
        }
    }

    /// Whether `candidate` is better than `reference` under this direction.
    pub fn is_better(self, candidate: f64, reference: f64) -> bool {
        match self {
            Direction::HigherIsBetter => candidate > reference,
            Direction::LowerIsBetter => candidate < reference,
        }
    }
}

/// The rule that turned outcomes into a number.
///
/// Two scores computed by different rules are incomparable even when both range over `[0, 1]` and
/// both come from the same trials: a pass rate and a Brier score disagree about what 0.2 means, and
/// they disagree about which way is up.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ScoringRule {
    pub name: String,
    pub direction: Direction,
    pub unit: String,
}

impl ScoringRule {
    pub fn new(name: impl Into<String>, direction: Direction, unit: impl Into<String>) -> Self {
        ScoringRule {
            name: name.into(),
            direction,
            unit: unit.into(),
        }
    }

    /// The pass rate `bioprism_atlas::Measurement::score` computes: passes over evaluable trials,
    /// dimensionless, higher is better.
    pub fn atlas_pass_rate() -> Self {
        ScoringRule::new(
            "atlas pass rate",
            Direction::HigherIsBetter,
            "fraction of evaluable trials",
        )
    }
}

impl fmt::Display for ScoringRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{} {}]",
            self.name,
            self.unit,
            self.direction.as_str()
        )
    }
}

/// The resource envelope a score was earned inside — 33.16's half of the picture.
///
/// Carried as a condition rather than as a metric because 33.16's own worked interpretation is
/// about comparability: "A marginal accuracy gain that costs ten times more ... is visible on the
/// Pareto frontier rather than hidden in rank." A score at ten times the budget is not a better
/// score, it is a different measurement.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Budget {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wall_clock_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<u64>,
}

impl Budget {
    pub fn labelled(label: impl Into<String>) -> Self {
        Budget {
            label: label.into(),
            tokens: None,
            wall_clock_ms: None,
            tool_calls: None,
        }
    }

    pub fn with_tokens(mut self, tokens: u64) -> Self {
        self.tokens = Some(tokens);
        self
    }

    pub fn with_wall_clock_ms(mut self, ms: u64) -> Self {
        self.wall_clock_ms = Some(ms);
        self
    }

    pub fn with_tool_calls(mut self, calls: u64) -> Self {
        self.tool_calls = Some(calls);
        self
    }
}

impl fmt::Display for Budget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The open part of 33.01's key: everything that slices the population rather than redefining the
/// measurement.
///
/// A dimension absent from one side is [`Condition::Unrecorded`] on that side, not a wildcard. That
/// is the same rule as everywhere else here: the absence of a coordinate is not permission to
/// assume it matched.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stratum(BTreeMap<String, Condition<String>>);

impl Stratum {
    pub fn new() -> Self {
        Stratum(BTreeMap::new())
    }

    pub fn with(mut self, dimension: impl Into<String>, value: impl Into<String>) -> Self {
        self.0
            .insert(dimension.into(), Condition::recorded(value.into()));
        self
    }

    /// Declares that a dimension was considered and deliberately not recorded. Distinct from
    /// leaving it out, in exactly the way an atlas hole is distinct from a missing cell.
    pub fn with_unrecorded(mut self, dimension: impl Into<String>) -> Self {
        self.0.insert(dimension.into(), Condition::Unrecorded);
        self
    }

    pub fn get(&self, dimension: &str) -> Condition<&str> {
        match self.0.get(dimension) {
            Some(Condition::Recorded { value }) => Condition::recorded(value.as_str()),
            _ => Condition::Unrecorded,
        }
    }

    pub fn dimensions(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Which of 33.01's fourteen coordinates this stratum, plus the dedicated fields listed in
    /// `covered_elsewhere`, still leaves unrecorded.
    pub fn missing_from_stratification_key(&self, covered_elsewhere: &[&str]) -> Vec<&'static str> {
        STRATIFICATION_KEY
            .iter()
            .copied()
            .filter(|dimension| {
                !covered_elsewhere.contains(dimension) && !self.get(dimension).is_recorded()
            })
            .collect()
    }

    fn compare(&self, other: &Stratum) -> Result<(), ScoreIncomparability> {
        let mut dimensions: Vec<&str> = self.dimensions().collect();
        for dimension in other.dimensions() {
            if !dimensions.contains(&dimension) {
                dimensions.push(dimension);
            }
        }
        dimensions.sort_unstable();
        for dimension in dimensions {
            compare_condition(
                dimension,
                &self.get(dimension),
                &other.get(dimension),
                |left, right| ScoreIncomparability::DifferentStratum {
                    dimension: dimension.to_string(),
                    left,
                    right,
                },
            )?;
        }
        Ok(())
    }
}

/// What a score is *about*: one capability, or a whole grid summarised.
///
/// Kept in the conditions rather than beside them because the commonest illegitimate comparison is
/// not two systems under different packs — it is one number about verification set against another
/// number about safety, on the same axis of the same chart.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "subject", rename_all = "snake_case")]
pub enum Subject {
    Capability {
        capability: CapabilityId,
    },
    /// A summary over many capabilities. The label is the grid's, and two grids with different
    /// labels are different subjects even when their cells coincide.
    Grid {
        label: String,
    },
}

impl Subject {
    pub fn capability(capability: CapabilityId) -> Self {
        Subject::Capability { capability }
    }

    pub fn grid(label: impl Into<String>) -> Self {
        Subject::Grid {
            label: label.into(),
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Subject::Capability { capability } => write!(f, "capability {capability}"),
            Subject::Grid { label } => write!(f, "grid {label}"),
        }
    }
}

/// Everything that has to match before two numbers may be set side by side.
///
/// None of the condition fields carries `#[serde(default)]`, and [`Condition`] has no `Default`
/// impl. A conditions document must therefore *state* each coordinate, `{"state":"unrecorded"}`
/// included, and a document that simply omits `pack_version` fails to deserialize rather than
/// quietly acquiring an unrecorded one. The distinction is small and it is the crate's whole
/// subject: an absence that was declared is a finding, an absence that was defaulted is an
/// accident. [`Stratum`] is the exception, because an empty stratum is a well-formed statement —
/// it is a map of declared coordinates, not a fixed set of them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeasurementConditions {
    pub subject: Subject,
    pub scoring_rule: ScoringRule,
    pub ontology_version: Condition<String>,
    pub pack_version: Condition<String>,
    /// What the system was allowed to read. Two systems evaluated against different evidence bases
    /// were asked different questions, however identical the trials.
    pub evidence_base: Condition<String>,
    /// The weakest oracle admitted into the denominator, not the strongest observed.
    pub oracle_floor: Condition<OracleTier>,
    pub budget: Condition<Budget>,
    #[serde(default)]
    pub stratum: Stratum,
}

impl MeasurementConditions {
    /// Everything unrecorded except the subject and the scoring rule, which are never optional.
    ///
    /// This constructor exists to be *inconvenient in the right direction*: a caller who builds
    /// conditions this way and compares two of them gets a refusal naming the first unrecorded
    /// dimension, which is the honest description of what they have.
    pub fn new(subject: Subject, scoring_rule: ScoringRule) -> Self {
        MeasurementConditions {
            subject,
            scoring_rule,
            ontology_version: Condition::Unrecorded,
            pack_version: Condition::Unrecorded,
            evidence_base: Condition::Unrecorded,
            oracle_floor: Condition::Unrecorded,
            budget: Condition::Unrecorded,
            stratum: Stratum::new(),
        }
    }

    pub fn with_ontology_version(mut self, version: impl Into<String>) -> Self {
        self.ontology_version = Condition::recorded(version.into());
        self
    }

    pub fn with_pack_version(mut self, version: impl Into<String>) -> Self {
        self.pack_version = Condition::recorded(version.into());
        self
    }

    pub fn with_evidence_base(mut self, base: impl Into<String>) -> Self {
        self.evidence_base = Condition::recorded(base.into());
        self
    }

    pub fn with_oracle_floor(mut self, tier: OracleTier) -> Self {
        self.oracle_floor = Condition::recorded(tier);
        self
    }

    pub fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = Condition::recorded(budget);
        self
    }

    pub fn with_stratum(mut self, stratum: Stratum) -> Self {
        self.stratum = stratum;
        self
    }

    /// Re-points the same conditions at a different subject, for the common case of a grid whose
    /// cells were all measured under one configuration.
    pub fn about(&self, subject: Subject) -> Self {
        let mut cloned = self.clone();
        cloned.subject = subject;
        cloned
    }

    /// The coordinates of 33.01's key that are still unrecorded here.
    ///
    /// An empty result does not mean the evaluation was good; it means the *labelling* is complete,
    /// which is the precondition for the rest of this crate to say anything at all.
    pub fn unrecorded_coordinates(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.pack_version.is_recorded() {
            missing.push("pack version");
        }
        if !self.oracle_floor.is_recorded() {
            missing.push("oracle tier");
        }
        if !self.budget.is_recorded() {
            missing.push("budget");
        }
        missing.extend(self.stratum.missing_from_stratification_key(&[
            "pack version",
            "oracle tier",
            "budget",
        ]));
        missing.sort_unstable();
        missing.dedup();
        missing
    }

    pub(crate) fn compare_stratum(&self, other: &Self) -> Result<(), ScoreIncomparability> {
        self.stratum.compare(&other.stratum)
    }
}
