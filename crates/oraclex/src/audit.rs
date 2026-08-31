//! Oracle audit, independence, and separation of duties (31.17).
//!
//! Most of 31.17 is a description of what an organisation does: convene reviewers, publish
//! limitations, run inter-laboratory comparisons, hold a quality-management system. None of that is a
//! predicate over an artifact and none of it is here.
//!
//! Three things in 31.17 *are* predicates, and they are the module:
//!
//! 1. **"Separate benchmark authoring from final hidden grading."** A role assignment is an artifact
//!    and the constraint is a check on it. [`separation`] runs it and returns a
//!    [`Witness::RoleConflict`] naming the party and the two roles.
//! 2. **The worked case.** "A sponsor may contribute a dataset but cannot unilaterally define the
//!    hidden oracle or suppress negative benchmark findings." Both halves are decidable:
//!    [`unilateral_control`] over the assignment, [`publication_integrity`] over the finding list.
//! 3. **Release gate 6.** "A second reviewer reproduces the reference output." Two content hashes
//!    either agree or they do not; [`independent_reproduction`] compares them.
//!
//! # Why the reproduction check is not a boolean
//!
//! A reproduction that was never attempted and a reproduction that disagreed are different states,
//! and a `bool` merges them into `false`. [`independent_reproduction`] takes `Option<&ContentHash>`
//! for the replicate and answers [`Determination::Unresolved`] when it is absent, naming the
//! reproduction as the missing evidence. This is the same shape as the rest of the crate for the same
//! reason.
//!
//! # Not implemented
//!
//! No sandbox and no access control. 31.17's "audit evaluator code and data access" and 31.05's
//! "oracle code executes in an isolated grader environment" are runtime isolation properties, and
//! `bioprism-oracle` already records that a library running in the caller's process cannot provide
//! them. Nothing here changes that; this crate records who *should* have access, not who does.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::verdict::{Determination, Unresolved, Witness};

/// A duty in the evaluation pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Wrote the benchmark cases.
    BenchmarkAuthor,
    /// Defines and runs the grading nobody else sees.
    HiddenGrader,
    /// Contributed the data.
    DataSponsor,
    /// Reviews the result without a stake in it.
    IndependentReviewer,
    /// Owns a system being evaluated.
    EvaluatedSystemOwner,
}

impl Role {
    pub const ALL: [Role; 5] = [
        Role::BenchmarkAuthor,
        Role::HiddenGrader,
        Role::DataSponsor,
        Role::IndependentReviewer,
        Role::EvaluatedSystemOwner,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::BenchmarkAuthor => "benchmark_author",
            Role::HiddenGrader => "hidden_grader",
            Role::DataSponsor => "data_sponsor",
            Role::IndependentReviewer => "independent_reviewer",
            Role::EvaluatedSystemOwner => "evaluated_system_owner",
        }
    }
}

/// Role pairs no single party may hold, each traced to the sentence that forbids it.
///
/// Four pairs, and every one of them cites 31.17 rather than a policy this crate invented. The list
/// is a `const` so a reader can check it against the blueprint by eye, and so the conflict test can
/// assert its length rather than trusting the implementation to have covered them all.
pub const INCOMPATIBLE: [(Role, Role, &str); 4] = [
    (
        Role::BenchmarkAuthor,
        Role::HiddenGrader,
        "31.17 required function: separate benchmark authoring from final hidden grading",
    ),
    (
        Role::DataSponsor,
        Role::HiddenGrader,
        "31.17 worked case: a sponsor cannot unilaterally define the hidden oracle",
    ),
    (
        Role::EvaluatedSystemOwner,
        Role::HiddenGrader,
        "31.05 independence: an oracle sharing an owner with the evaluated system is circular",
    ),
    (
        Role::EvaluatedSystemOwner,
        Role::IndependentReviewer,
        "31.17 purpose: conflict-of-interest control over review",
    ),
];

/// Who holds which duties.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleAssignment {
    held: BTreeMap<String, BTreeSet<Role>>,
}

impl RoleAssignment {
    pub fn new() -> Self {
        RoleAssignment::default()
    }

    pub fn assign(mut self, party: impl Into<String>, role: Role) -> Self {
        self.held.entry(party.into()).or_default().insert(role);
        self
    }

    pub fn parties(&self) -> impl Iterator<Item = &str> {
        self.held.keys().map(String::as_str)
    }

    pub fn roles_of(&self, party: &str) -> BTreeSet<Role> {
        self.held.get(party).cloned().unwrap_or_default()
    }

    pub fn holders_of(&self, role: Role) -> BTreeSet<&str> {
        self.held
            .iter()
            .filter(|(_, roles)| roles.contains(&role))
            .map(|(party, _)| party.as_str())
            .collect()
    }
}

/// Whether any party holds an incompatible pair of duties.
pub fn separation(assignment: &RoleAssignment) -> Determination {
    if assignment.held.is_empty() {
        return Determination::not_evaluable("no roles have been assigned");
    }
    for (party, roles) in &assignment.held {
        for (left, right, _) in INCOMPATIBLE {
            if roles.contains(&left) && roles.contains(&right) {
                return Determination::contradicted(
                    EvidenceTier::Deterministic,
                    Witness::RoleConflict {
                        party: party.clone(),
                        holds: left.as_str().to_string(),
                        and: right.as_str().to_string(),
                    },
                );
            }
        }
    }
    Determination::supported(
        EvidenceTier::Deterministic,
        format!(
            "no party among {} holds an incompatible pair",
            assignment.held.len()
        ),
    )
}

/// Whether one party can act alone on a role that 31.17 requires oversight of.
///
/// "Unilaterally" is the blueprint's word and it is about the absence of a check, not about the
/// number of holders. A single hidden grader is normal; a single hidden grader with nobody assigned to
/// review them is what the worked case forbids.
pub fn unilateral_control(assignment: &RoleAssignment, role: Role) -> Determination {
    let holders = assignment.holders_of(role);
    if holders.is_empty() {
        return Determination::not_evaluable("nobody holds this role");
    }
    let reviewers: BTreeSet<&str> = assignment
        .holders_of(Role::IndependentReviewer)
        .into_iter()
        .filter(|reviewer| !holders.contains(reviewer))
        .collect();
    if reviewers.is_empty() {
        return Determination::Unresolved(Unresolved::of(
            "an independent reviewer who does not hold this role",
            format!(
                "{:?} hold {} with no separate reviewer, so the role is exercised unilaterally",
                holders,
                role.as_str()
            ),
        ));
    }
    Determination::supported(
        EvidenceTier::Deterministic,
        format!(
            "{} is held by {:?} and reviewed by {:?}",
            role.as_str(),
            holders,
            reviewers
        ),
    )
}

/// A benchmark finding and whether anyone kept it out of the report.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ReportedFinding {
    pub summary: String,
    /// Whether the finding reflects badly on a contributor. Recorded because 31.17's worked case is
    /// specifically about *negative* findings, and a suppression check that cannot see the sign of a
    /// finding cannot notice that only one sign disappears.
    pub negative: bool,
    /// The party that withheld it, if any.
    pub withheld_by: Option<String>,
}

impl ReportedFinding {
    pub fn positive(summary: impl Into<String>) -> Self {
        ReportedFinding {
            summary: summary.into(),
            negative: false,
            withheld_by: None,
        }
    }

    pub fn negative(summary: impl Into<String>) -> Self {
        ReportedFinding {
            summary: summary.into(),
            negative: true,
            withheld_by: None,
        }
    }

    pub fn withheld_by(mut self, party: impl Into<String>) -> Self {
        self.withheld_by = Some(party.into());
        self
    }
}

/// Whether the report contains everything that was found.
///
/// Any withheld finding is a contradiction. AGENTS.md states the workspace position without
/// qualification — "If a measurement disagrees with the thesis, that is the measurement we publish" —
/// and 31.17 turns it into an audit control.
pub fn publication_integrity(findings: &[ReportedFinding]) -> Determination {
    if findings.is_empty() {
        return Determination::not_evaluable("no findings were recorded");
    }
    match findings
        .iter()
        .find(|finding| finding.withheld_by.is_some())
    {
        Some(finding) => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RoleConflict {
                party: finding
                    .withheld_by
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                holds: "contributor".to_string(),
                and: format!(
                    "suppressor of a {} finding: {}",
                    if finding.negative {
                        "negative"
                    } else {
                        "positive"
                    },
                    finding.summary
                ),
            },
        ),
        None => Determination::supported(
            EvidenceTier::Deterministic,
            format!(
                "{} of {} findings are negative and all are published",
                findings.iter().filter(|f| f.negative).count(),
                findings.len()
            ),
        ),
    }
}

/// Release gate 6: a second reviewer reproduces the reference output.
///
/// Three answers, not two. `None` for the replicate is unresolved and says so; that is the state most
/// oracles are actually in, and reporting it as a failed reproduction would be as wrong as reporting
/// it as a passed one.
pub fn independent_reproduction(
    reference: &ContentHash,
    replicate: Option<&ContentHash>,
) -> Determination {
    match replicate {
        None => Determination::unresolved(
            "an independent recomputation of the reference output",
            "release gate 6 requires a second reviewer to reproduce it, and none has",
        ),
        Some(replicate) if replicate == reference => Determination::supported(
            EvidenceTier::Deterministic,
            format!("an independent run reproduced {}", reference.as_str()),
        ),
        Some(replicate) => Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "independent reproduction of the reference output".to_string(),
                expected: reference.as_str().to_string(),
                observed: replicate.as_str().to_string(),
            },
        ),
    }
}
