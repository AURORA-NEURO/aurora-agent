//! Structural knobs `bioprism-worldgen` does not have.
//!
//! Blueprint 43.39 makes structure a parameter so that "under which structures does each strategy
//! succeed?" is an empirical question. `bioprism-worldgen` implements three of those parameters —
//! distractor attachment, relay depth, tag camouflage — and this crate reuses all three unchanged
//! rather than shipping a second vocabulary for them.
//!
//! Everything below is a knob the worlds in this crate needed and `worldgen` cannot express, so it
//! is built here as world-specific construction instead. Written as an enumeration rather than a
//! paragraph in a report, because a paragraph is not something a later contributor can iterate
//! over, and because `crates/examples` has already shown that the list of things a platform cannot
//! do is the half of the catalogue that goes missing.
//!
//! This is a statement about `WorldSpec`'s surface, not a complaint: each entry names the field
//! that would have to exist, so the list doubles as a change proposal.

use serde::{Deserialize, Serialize};

/// One dimension of structural variation `bioprism_worldgen::spec::WorldSpec` cannot express.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingGeneratorKnob {
    /// Which variables an event governs.
    EventManagedVariableSet,
    /// Which facts carry the protected vocabulary, independently of which are event-managed.
    ProtectionSetIndependentOfEvents,
    /// Where the decision cut falls relative to the releases.
    ReleaseScheduleAndCutPlacement,
    /// A decisive skeleton other than the five split-integrity checks.
    AlternativeDecisiveSkeleton,
    /// More than one terminal hypothesis, with a declared exclusion between them.
    CompetingHypothesisTerminals,
    /// Evidence recorded as *declared absent* rather than simply omitted.
    DeclaredAbsenceRecords,
}

impl MissingGeneratorKnob {
    pub const ALL: [MissingGeneratorKnob; 6] = [
        MissingGeneratorKnob::EventManagedVariableSet,
        MissingGeneratorKnob::ProtectionSetIndependentOfEvents,
        MissingGeneratorKnob::ReleaseScheduleAndCutPlacement,
        MissingGeneratorKnob::AlternativeDecisiveSkeleton,
        MissingGeneratorKnob::CompetingHypothesisTerminals,
        MissingGeneratorKnob::DeclaredAbsenceRecords,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            MissingGeneratorKnob::EventManagedVariableSet => "event_managed_variable_set",
            MissingGeneratorKnob::ProtectionSetIndependentOfEvents => {
                "protection_set_independent_of_events"
            }
            MissingGeneratorKnob::ReleaseScheduleAndCutPlacement => {
                "release_schedule_and_cut_placement"
            }
            MissingGeneratorKnob::AlternativeDecisiveSkeleton => "alternative_decisive_skeleton",
            MissingGeneratorKnob::CompetingHypothesisTerminals => "competing_hypothesis_terminals",
            MissingGeneratorKnob::DeclaredAbsenceRecords => "declared_absence_records",
        }
    }

    /// What is missing, stated against the generator's actual code.
    pub const fn gap(self) -> &'static str {
        match self {
            MissingGeneratorKnob::EventManagedVariableSet => {
                "generate_world emits exactly two events with hard-coded produces lists (training_decision_time, split_assignment, preprocess_fit_scope; future_label_value); WorldSpec has no field naming which variables an event governs"
            }
            MissingGeneratorKnob::ProtectionSetIndependentOfEvents => {
                "the protected tag on each fact is a literal in generate_world, and every event-managed variable happens to carry it; there is no knob that makes a variable event-managed without also making it protected, which is the exact obstacle crates/examples records against non_protected_temporal_withholding"
            }
            MissingGeneratorKnob::ReleaseScheduleAndCutPlacement => {
                "availability_time and the query's decision_time are literals in generate_world and generate_query; WorldSpec carries no cut, so the cut cannot be moved relative to the releases and no spec in the family withholds anything"
            }
            MissingGeneratorKnob::AlternativeDecisiveSkeleton => {
                "the CHECKS table and the split_integrity_status target are consts; a world with a different decision needs a different generator, which is why this crate builds worlds directly rather than parameterising that one"
            }
            MissingGeneratorKnob::CompetingHypothesisTerminals => {
                "factor.claim_support produces a single terminal and there is no way to declare two terminals mutually exclusive, so no spec in the family can leave a verdict open"
            }
            MissingGeneratorKnob::DeclaredAbsenceRecords => {
                "fact.negative_duplicates is a single hard-coded negative-evidence fact; nothing parameterises which evidence is recorded as declared-absent, which is what separates an underdetermined world from one that is merely missing data"
            }
        }
    }

    /// The field that would close it.
    pub const fn proposed_field(self) -> &'static str {
        match self {
            MissingGeneratorKnob::EventManagedVariableSet => {
                "WorldSpec::events: Vec<EventSpec { id, event_time, availability_time, produces }>"
            }
            MissingGeneratorKnob::ProtectionSetIndependentOfEvents => {
                "WorldSpec::protected_variables: BTreeSet<String>, applied instead of the literal protected tags"
            }
            MissingGeneratorKnob::ReleaseScheduleAndCutPlacement => {
                "WorldSpec::decision_time: String, threaded into generate_query"
            }
            MissingGeneratorKnob::AlternativeDecisiveSkeleton => {
                "WorldSpec::skeleton: Skeleton, with the current CHECKS table as one variant"
            }
            MissingGeneratorKnob::CompetingHypothesisTerminals => {
                "WorldSpec::hypotheses: Vec<String> plus an exclusion factor over them"
            }
            MissingGeneratorKnob::DeclaredAbsenceRecords => {
                "WorldSpec::declared_absent: Vec<String>, emitting facts whose value carries an ObservationStatus"
            }
        }
    }
}

/// The three knobs `worldgen` *does* have, reused here unchanged.
///
/// Recorded so the report is symmetric: a list of gaps with no list of what was reused reads as if
/// nothing was.
pub const REUSED_GENERATOR_KNOBS: [&str; 3] = [
    "bioprism_worldgen::spec::DistractorAttachment",
    "bioprism_worldgen::spec::TagStyle",
    "bioprism_worldgen::rng::SplitMix64",
];
