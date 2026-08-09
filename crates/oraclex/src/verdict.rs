//! The four answers, with the two abstentions made expensive to produce.
//!
//! `bioprism-oracle` already has [`Position`], a four-valued enum covering 31.01's
//! `supported | contradicted | unresolved | not-evaluable`. An enum discriminant is cheap: nothing
//! stops a caller writing `Position::Unresolved` and moving on, and nothing downstream can tell an
//! abstention that named its gap from one that shrugged.
//!
//! [`Determination`] is the same four answers with the payload each one owes.
//!
//! * [`Unresolved`] cannot be constructed without at least one [`Missing`]. `crates/bioworlds`
//!   built a world that genuinely underdetermines its question and found a v0.1 oracle answering
//!   `valid` on it — a wrong answer rather than a missing one. The fix is not a better threshold;
//!   it is making the honest answer available and the dishonest one harder to reach.
//! * [`NotEvaluable`] cannot be constructed without a reason. "Out of scope" that does not say
//!   which scope is indistinguishable from "the code path fell through".
//! * [`Contradicted`] cannot be constructed without a [`Witness`]. `crates/section` establishes the
//!   standard: a witness is a concrete checkable object, not a score.
//!
//! Only [`Support`] is cheap to build, and that asymmetry is deliberate — support is the claim this
//! workspace most wants to be suspicious of.
//!
//! # What this type deliberately lacks
//!
//! No `Default`, no `unwrap_or_supported`, no `is_valid` that folds the abstentions in with
//! support. [`Determination::is_supported`] is false for both abstentions, and
//! [`Determination::decided`] exists so a caller who genuinely needs "did anything conclude" has
//! to ask for it by that name.

use std::collections::BTreeSet;

use bioprism_oracle::{EvidenceTier, Finding, Position};
use serde::{Deserialize, Serialize};

use crate::error::OracleXError;

/// A named gap in the evidence: the thing that, if supplied, would let the check decide.
///
/// `crates/choreography`'s adjudication rule requires exactly this shape — its
/// `Ruling::Unresolved` "names the evidence that is missing", with deliberately no rule that
/// defaults to the higher-authority party. The same rule holds here for a reference standard: two
/// oracles disagreeing is a finding with a witness, and a mesh that silently picks one has
/// destroyed it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Missing {
    /// What is absent, in the vocabulary of the check that wanted it.
    pub evidence: String,
    /// Why this check cannot proceed without it.
    pub because: String,
}

impl Missing {
    pub fn new(evidence: impl Into<String>, because: impl Into<String>) -> Self {
        Missing {
            evidence: evidence.into(),
            because: because.into(),
        }
    }
}

/// A concrete object a reader could check by hand.
///
/// Distinct from `bioprism_oracle::Finding`, which is shaped for artifact and schema defects. These
/// are the witnesses the reference-standard planes produce: a fingerprint that does not match, a
/// control that was never run, a date reconciled against a hierarchy that does not rank its
/// sources. [`Witness::to_finding`] projects onto the oracle crate's type and says what it loses.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "witness", rename_all = "snake_case")]
pub enum Witness {
    /// Two artifacts joined by an identifier carry incompatible identity evidence (31.05).
    IdentityConflict {
        left: String,
        right: String,
        joined_on: String,
        conflicting_evidence: String,
    },
    /// A control the claim's plane requires was not run (31.08).
    ControlAbsent { claim: String, control: String },
    /// Two sources give different values for one field and the declared hierarchy ranks them
    /// (31.12). The witness records the loser so the reconciliation is auditable.
    SourceOverridden {
        field: String,
        kept: String,
        kept_source: String,
        dropped: String,
        dropped_source: String,
    },
    /// A declared relation and the observed behaviour disagree (32.05, 32.21).
    RelationViolated {
        relation: String,
        expected: String,
        observed: String,
    },
    /// One party holds two roles that the separation-of-duties rule keeps apart (31.17).
    RoleConflict {
        party: String,
        holds: String,
        and: String,
    },
    /// A quantity was used in a place demanding a different dimension (32.16).
    DimensionError {
        pointer: String,
        expected: String,
        found: String,
    },
    /// An artifact was treated as a completed result when its producer did not complete (32.14).
    IncompleteTreatedAsComplete { step: String, outcome: String },
    /// A record that should not have crossed a boundary did (32.19).
    EgressViolation { field: String, boundary: String },
}

impl Witness {
    /// The name under which this witness is aggregated.
    pub fn kind(&self) -> &'static str {
        match self {
            Witness::IdentityConflict { .. } => "identity_conflict",
            Witness::ControlAbsent { .. } => "control_absent",
            Witness::SourceOverridden { .. } => "source_overridden",
            Witness::RelationViolated { .. } => "relation_violated",
            Witness::RoleConflict { .. } => "role_conflict",
            Witness::DimensionError { .. } => "dimension_error",
            Witness::IncompleteTreatedAsComplete { .. } => "incomplete_treated_as_complete",
            Witness::EgressViolation { .. } => "egress_violation",
        }
    }

    /// Projects onto `bioprism_oracle::Finding` for consumers reading the mesh's shape.
    ///
    /// Lossy, and in one specific way worth stating: every variant here becomes
    /// `Finding::PropertyViolated`, so a downstream reader can tell *that* a named property failed
    /// and read the detail string, but cannot pattern-match the structure back out. Callers that
    /// need the structure should serialise [`Witness`] itself. Nothing in this crate consumes the
    /// projection, which is how it stays honest.
    pub fn to_finding(&self) -> Finding {
        Finding::PropertyViolated {
            property: self.kind().to_string(),
            pointer: String::new(),
            detail: self.detail(),
        }
    }

    fn detail(&self) -> String {
        match self {
            Witness::IdentityConflict {
                left,
                right,
                joined_on,
                conflicting_evidence,
            } => format!(
                "{left} and {right} were joined on {joined_on} but {conflicting_evidence} conflicts"
            ),
            Witness::ControlAbsent { claim, control } => {
                format!("claim '{claim}' requires control '{control}', which was not run")
            }
            Witness::SourceOverridden {
                field,
                kept,
                kept_source,
                dropped,
                dropped_source,
            } => format!(
                "{field}: kept {kept} from {kept_source}, dropped {dropped} from {dropped_source}"
            ),
            Witness::RelationViolated {
                relation,
                expected,
                observed,
            } => format!("relation {relation} expected {expected}, observed {observed}"),
            Witness::RoleConflict { party, holds, and } => {
                format!("{party} holds both {holds} and {and}")
            }
            Witness::DimensionError {
                pointer,
                expected,
                found,
            } => format!("{pointer} expected dimension {expected}, found {found}"),
            Witness::IncompleteTreatedAsComplete { step, outcome } => {
                format!("step {step} ended {outcome}; its output is not a result")
            }
            Witness::EgressViolation { field, boundary } => {
                format!("{field} crossed {boundary}")
            }
        }
    }
}

/// A supported determination, with the tier of the evidence that supports it.
///
/// The tier travels with the support because 31.05's independence analysis demotes an oracle that
/// shares data or code with what it evaluates, and a support whose tier is unrecorded cannot be
/// demoted later.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Support {
    pub tier: EvidenceTier,
    pub basis: String,
}

impl Support {
    pub fn new(tier: EvidenceTier, basis: impl Into<String>) -> Self {
        Support {
            tier,
            basis: basis.into(),
        }
    }
}

/// A contradicted determination. Cannot be built empty.
///
/// The `try_from` attribute matters as much as the constructor. A hand-written JSON document with an
/// empty witness list would otherwise walk straight past [`Contradiction::new`] and into the type,
/// and an invariant that only holds on the construction path is an invariant that holds until
/// somebody adds a deserializer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ContradictionRepr", into = "ContradictionRepr")]
pub struct Contradiction {
    pub tier: EvidenceTier,
    witnesses: Vec<Witness>,
}

#[derive(Serialize, Deserialize)]
struct ContradictionRepr {
    tier: EvidenceTier,
    witnesses: Vec<Witness>,
}

impl From<Contradiction> for ContradictionRepr {
    fn from(value: Contradiction) -> Self {
        ContradictionRepr {
            tier: value.tier,
            witnesses: value.witnesses,
        }
    }
}

impl TryFrom<ContradictionRepr> for Contradiction {
    type Error = OracleXError;

    fn try_from(value: ContradictionRepr) -> Result<Self, Self::Error> {
        Contradiction::new(value.tier, value.witnesses)
    }
}

impl Contradiction {
    pub fn new(
        tier: EvidenceTier,
        witnesses: impl IntoIterator<Item = Witness>,
    ) -> Result<Self, OracleXError> {
        let witnesses: Vec<Witness> = witnesses.into_iter().collect();
        if witnesses.is_empty() {
            return Err(OracleXError::ContradictionWithoutFinding);
        }
        Ok(Contradiction { tier, witnesses })
    }

    /// Convenience for the common single-witness case.
    pub fn of(tier: EvidenceTier, witness: Witness) -> Self {
        Contradiction {
            tier,
            witnesses: vec![witness],
        }
    }

    pub fn witnesses(&self) -> &[Witness] {
        &self.witnesses
    }
}

/// The check applies, ran, and did not settle the question. Cannot be built empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UnresolvedRepr", into = "UnresolvedRepr")]
pub struct Unresolved {
    missing: BTreeSet<Missing>,
}

#[derive(Serialize, Deserialize)]
struct UnresolvedRepr {
    missing: BTreeSet<Missing>,
}

impl From<Unresolved> for UnresolvedRepr {
    fn from(value: Unresolved) -> Self {
        UnresolvedRepr {
            missing: value.missing,
        }
    }
}

impl TryFrom<UnresolvedRepr> for Unresolved {
    type Error = OracleXError;

    fn try_from(value: UnresolvedRepr) -> Result<Self, Self::Error> {
        Unresolved::new(value.missing)
    }
}

impl Unresolved {
    pub fn new(missing: impl IntoIterator<Item = Missing>) -> Result<Self, OracleXError> {
        let missing: BTreeSet<Missing> = missing.into_iter().collect();
        if missing.is_empty() {
            return Err(OracleXError::UnresolvedWithoutMissingEvidence);
        }
        Ok(Unresolved { missing })
    }

    /// Convenience for the common single-gap case.
    pub fn of(evidence: impl Into<String>, because: impl Into<String>) -> Self {
        let mut set = BTreeSet::new();
        set.insert(Missing::new(evidence, because));
        Unresolved { missing: set }
    }

    pub fn missing(&self) -> &BTreeSet<Missing> {
        &self.missing
    }
}

/// The check does not apply here. Cannot be built without saying why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "NotEvaluableRepr", into = "NotEvaluableRepr")]
pub struct NotEvaluable {
    reason: String,
}

#[derive(Serialize, Deserialize)]
struct NotEvaluableRepr {
    reason: String,
}

impl From<NotEvaluable> for NotEvaluableRepr {
    fn from(value: NotEvaluable) -> Self {
        NotEvaluableRepr {
            reason: value.reason,
        }
    }
}

impl TryFrom<NotEvaluableRepr> for NotEvaluable {
    type Error = OracleXError;

    fn try_from(value: NotEvaluableRepr) -> Result<Self, Self::Error> {
        NotEvaluable::new(value.reason)
    }
}

impl NotEvaluable {
    pub fn new(reason: impl Into<String>) -> Result<Self, OracleXError> {
        let reason = reason.into();
        if reason.trim().is_empty() {
            return Err(OracleXError::NotEvaluableWithoutReason);
        }
        Ok(NotEvaluable { reason })
    }

    /// Convenience for call sites with a literal reason.
    pub fn of(reason: &'static str) -> Self {
        NotEvaluable {
            reason: reason.to_string(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// One check's answer about one artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "determination", rename_all = "snake_case")]
pub enum Determination {
    Supported(Support),
    Contradicted(Contradiction),
    Unresolved(Unresolved),
    NotEvaluable(NotEvaluable),
}

impl Determination {
    /// Projects onto `bioprism_oracle::Position` so a determination can enter the mesh.
    ///
    /// Lossy in the direction that matters: the [`Missing`] set and the [`Witness`] list are
    /// dropped. There is no inverse — see the absence of `from_position`.
    pub fn position(&self) -> Position {
        match self {
            Determination::Supported(_) => Position::Supported,
            Determination::Contradicted(_) => Position::Contradicted,
            Determination::Unresolved(_) => Position::Unresolved,
            Determination::NotEvaluable(_) => Position::NotEvaluable,
        }
    }

    /// True only for [`Determination::Supported`].
    ///
    /// Named `is_supported` rather than `is_valid` because "valid" is the word a caller reaches for
    /// when they are about to treat an abstention as a pass.
    pub fn is_supported(&self) -> bool {
        matches!(self, Determination::Supported(_))
    }

    /// Whether the check took a side at all.
    pub fn decided(&self) -> bool {
        matches!(
            self,
            Determination::Supported(_) | Determination::Contradicted(_)
        )
    }

    /// Whether the check declined to take a side.
    pub fn is_abstention(&self) -> bool {
        !self.decided()
    }

    /// The gaps this determination named, empty for every state but [`Determination::Unresolved`].
    pub fn missing(&self) -> BTreeSet<Missing> {
        match self {
            Determination::Unresolved(unresolved) => unresolved.missing().clone(),
            _ => BTreeSet::new(),
        }
    }

    /// The witnesses this determination carries, empty unless contradicted.
    pub fn witnesses(&self) -> &[Witness] {
        match self {
            Determination::Contradicted(contradiction) => contradiction.witnesses(),
            _ => &[],
        }
    }

    /// The tier of the evidence behind a decided answer. `None` for both abstentions, because an
    /// abstention has no tier — it is not weak evidence, it is the absence of evidence.
    pub fn tier(&self) -> Option<EvidenceTier> {
        match self {
            Determination::Supported(support) => Some(support.tier),
            Determination::Contradicted(contradiction) => Some(contradiction.tier),
            Determination::Unresolved(_) | Determination::NotEvaluable(_) => None,
        }
    }

    pub fn supported(tier: EvidenceTier, basis: impl Into<String>) -> Self {
        Determination::Supported(Support::new(tier, basis))
    }

    pub fn contradicted(tier: EvidenceTier, witness: Witness) -> Self {
        Determination::Contradicted(Contradiction::of(tier, witness))
    }

    pub fn unresolved(evidence: impl Into<String>, because: impl Into<String>) -> Self {
        Determination::Unresolved(Unresolved::of(evidence, because))
    }

    pub fn not_evaluable(reason: &'static str) -> Self {
        Determination::NotEvaluable(NotEvaluable::of(reason))
    }
}
