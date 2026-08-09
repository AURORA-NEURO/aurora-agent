//! Set-valued combination (40.21).
//!
//! 40.21's output is a "combined verdict distribution" and an "oracle disagreement and appeal
//! record", not a winner. This module is the rule that produces them, and it is defined by what
//! it refuses to do:
//!
//! * **no majority vote.** Three judges agreeing do not outweigh one checksum, and two schema
//!   oracles agreeing do not make a third one's contradiction disappear.
//! * **no averaging.** Two same-tier oracles at `Supported` and `Contradicted` yield the *set*
//!   `{Supported, Contradicted}` and [`OracleStatus::Underdetermined`], with both positions and
//!   both oracles retained in a [`Disagreement`]. There is no midpoint between "the split leaks
//!   patients" and "it does not", and inventing one is how a benchmark stops measuring anything.
//! * **no silent suppression.** When a weaker oracle's position loses to a stronger rung, the
//!   attempt is recorded as a [`SuppressedOverride`] naming the rule that stopped it. The
//!   11.11 invariant is only credible if you can see it fire.
//!
//! # The procedure
//!
//! 1. Partition judgements into admissible and inadmissible (31.16). Inadmissible ones are kept
//!    and reported; they contribute nothing.
//! 2. Set aside every [`Position::NotEvaluable`] judgement. Those oracles declared this evidence
//!    outside their scope, so they do not get to set the deciding tier.
//! 3. Take the strongest tier among the rest. That is the *deciding tier*, and only judgements on
//!    it can form the acceptable set.
//! 4. The acceptable set is the set of non-abstaining positions at the deciding tier. If every
//!    judgement at that tier abstained, the abstentions themselves become the set — "I cannot
//!    tell" is an answer when it is the only answer, and noise when it is not.
//! 5. Every judgement below the deciding tier whose position is not in the acceptable set is
//!    recorded as a suppressed override.
//! 6. If the acceptable set holds more than one position, emit a [`Disagreement`].
//!
//! # Why `NotEvaluable` and `Unresolved` are handled differently
//!
//! Both are abstentions, and 31.01 says neither may be "scored as a hidden negative". But they
//! abstain for different reasons and the difference decides who gets to speak. `NotEvaluable`
//! means *this oracle does not apply here* — a schema oracle handed a free-text hypothesis — and
//! such an oracle must not occupy the top rung and thereby block a weaker oracle that does apply.
//! `Unresolved` means *this oracle applies and cannot decide*, which is a substantive finding at
//! its rung: a deterministic checker that genuinely cannot tell does block a judge from deciding
//! in its place. Step 2 encodes the first; step 4 encodes the second.

use std::collections::BTreeSet;

use bioprism_section::{LeakageWitness, OracleStatus, OracleVerdict};
use serde::{Deserialize, Serialize};

use crate::disagreement::Disagreement;
use crate::judgement::{Confidence, ConfidenceEnvelope, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::OracleRef;
use crate::plane::Plane;
use crate::time::UtcTimestamp;

/// Which rule stopped a weaker judgement from displacing a stronger one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideRule {
    /// The 11.11 invariant, and 40.21 invariant 2: "Model judges never overwrite deterministic
    /// failures." A nondeterministic tier attempted to displace a grounded one.
    NondeterministicOverGrounded,
    /// General ladder precedence: a strictly weaker rung attempted to displace a stronger one.
    LowerTierOverHigher,
}

impl OverrideRule {
    pub fn as_str(self) -> &'static str {
        match self {
            OverrideRule::NondeterministicOverGrounded => "nondeterministic_over_grounded",
            OverrideRule::LowerTierOverHigher => "lower_tier_over_higher",
        }
    }
}

/// A recorded, refused override attempt.
///
/// `attempted_confidence` is here so the record answers the obvious objection out loud: yes, the
/// judge was certain, and no, that did not matter. 31.01's failure containment says a weak oracle
/// "cannot be promoted to definitive truth merely because it is convenient", and confidence is the
/// most convenient promotion route there is.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuppressedOverride {
    pub oracle: OracleRef,
    pub attempted_position: Position,
    pub attempted_tier: EvidenceTier,
    pub attempted_confidence: Confidence,
    pub deciding_tier: EvidenceTier,
    pub deciding_positions: BTreeSet<Position>,
    pub rule: OverrideRule,
}

/// Why the verdict says what it says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "basis", rename_all = "snake_case")]
pub enum VerdictBasis {
    /// Decided by the judgements at `tier`.
    Decided { tier: EvidenceTier },
    /// Every judgement was inadmissible, or none was offered. 40.21 invariant 4: "Unknown remains
    /// a valid result."
    NoAdmissibleOracle,
    /// Oracles ran and were admissible, but every one of them declared this evidence outside its
    /// scope. Distinct from [`VerdictBasis::NoAdmissibleOracle`]: the mesh was healthy and simply
    /// has no oracle for this question, which is a gap in coverage rather than a gap in operations.
    NoApplicableOracle,
    /// Judgements existed but none reached the floor the policy requires for this decision.
    BelowPolicyFloor {
        best: EvidenceTier,
        required: EvidenceTier,
    },
}

/// An oracle that failed rather than judged (40.21, "oracle unavailable").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleFailure {
    pub oracle: OracleRef,
    pub error: String,
    pub retry: RetryClass,
}

/// 40.21's failure semantics require a retry classification on every typed failure.
///
/// Every error this crate raises is [`RetryClass::Permanent`]: they are all manifest or
/// configuration defects, and retrying a contradictory plane declaration will not help. The
/// transient class exists for providers that reach a network or a sandbox, which this crate does
/// not, and is never produced here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryClass {
    Transient,
    Permanent,
}

/// The declared combination policy of 40.21.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeshPolicy {
    /// The weakest rung permitted to decide. Defaults to [`EvidenceTier::Judge`], which admits
    /// everything; a gate that must not be passed on opinion alone raises it.
    pub minimum_deciding_tier: EvidenceTier,
}

impl MeshPolicy {
    pub fn new(minimum_deciding_tier: EvidenceTier) -> Self {
        MeshPolicy {
            minimum_deciding_tier,
        }
    }

    /// Requires grounded evidence — [`EvidenceTier::Execution`] or better — before any verdict is
    /// decided. Under this policy a mesh of judges returns "unknown", not an opinion.
    pub fn grounded_only() -> Self {
        MeshPolicy::new(EvidenceTier::Execution)
    }

    /// Combines judgements into a set-valued verdict. See the module docs for the procedure.
    pub fn combine(
        &self,
        subject: impl Into<String>,
        at: &UtcTimestamp,
        judgements: Vec<Judgement>,
    ) -> CombinedVerdict {
        let subject = subject.into();
        let (admissible, inadmissible): (Vec<Judgement>, Vec<Judgement>) =
            judgements.into_iter().partition(Judgement::is_admissible);

        if admissible.is_empty() {
            return CombinedVerdict::unknown(
                subject,
                at.clone(),
                VerdictBasis::NoAdmissibleOracle,
                inadmissible,
            );
        }

        let (applicable, out_of_scope): (Vec<Judgement>, Vec<Judgement>) = admissible
            .into_iter()
            .partition(|j| j.position != Position::NotEvaluable);

        let Some(best) = applicable.iter().map(|j| j.tier).max() else {
            let mut verdict = CombinedVerdict::unknown(
                subject,
                at.clone(),
                VerdictBasis::NoApplicableOracle,
                inadmissible,
            );
            verdict.withheld = out_of_scope;
            return verdict;
        };

        let admissible = applicable;

        if best < self.minimum_deciding_tier {
            let mut verdict = CombinedVerdict::unknown(
                subject,
                at.clone(),
                VerdictBasis::BelowPolicyFloor {
                    best,
                    required: self.minimum_deciding_tier,
                },
                inadmissible,
            );
            verdict.withheld = admissible;
            verdict.withheld.extend(out_of_scope);
            return verdict;
        }

        let (deciding, mut weaker): (Vec<Judgement>, Vec<Judgement>) =
            admissible.into_iter().partition(|j| j.tier == best);

        let committed: Vec<&Judgement> = deciding
            .iter()
            .filter(|j| !j.position.is_abstention())
            .collect();
        let acceptable: BTreeSet<Position> = if committed.is_empty() {
            deciding.iter().map(|j| j.position).collect()
        } else {
            committed.iter().map(|j| j.position).collect()
        };

        let confidence = if committed.is_empty() {
            ConfidenceEnvelope::over(deciding.iter().map(|j| j.confidence))
        } else {
            ConfidenceEnvelope::over(committed.iter().map(|j| j.confidence))
        };

        let suppressed: Vec<SuppressedOverride> = weaker
            .iter()
            .filter(|j| !j.position.is_abstention() && !acceptable.contains(&j.position))
            .map(|j| SuppressedOverride {
                oracle: j.oracle.clone(),
                attempted_position: j.position,
                attempted_tier: j.tier,
                attempted_confidence: j.confidence,
                deciding_tier: best,
                deciding_positions: acceptable.clone(),
                rule: if j.tier.is_nondeterministic() && best.is_grounded() {
                    OverrideRule::NondeterministicOverGrounded
                } else {
                    OverrideRule::LowerTierOverHigher
                },
            })
            .collect();

        let disagreements = if acceptable.len() > 1 {
            vec![Disagreement::between(best, &deciding)]
        } else {
            Vec::new()
        };

        weaker.extend(out_of_scope);

        CombinedVerdict {
            subject,
            at: at.clone(),
            acceptable,
            basis: VerdictBasis::Decided { tier: best },
            confidence,
            contributing: deciding,
            withheld: weaker,
            inadmissible,
            suppressed,
            disagreements,
            failures: Vec::new(),
        }
    }
}

impl Default for MeshPolicy {
    fn default() -> Self {
        MeshPolicy::new(EvidenceTier::Judge)
    }
}

/// The result of combining a mesh's judgements about one piece of evidence.
///
/// Every input is retained somewhere: judgements that decided the verdict in `contributing`,
/// admissible judgements below the deciding tier in `withheld`, expired or superseded ones in
/// `inadmissible`, refused overrides in `suppressed`, and oracles that crashed in `failures`.
/// Nothing is dropped, because 40.21 asks for an "evidence graph" and a graph with deleted nodes
/// is a summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CombinedVerdict {
    pub subject: String,
    pub at: UtcTimestamp,
    /// The set of acceptable positions. One element means decided; more than one means
    /// underdetermined with every position retained.
    pub acceptable: BTreeSet<Position>,
    pub basis: VerdictBasis,
    /// The range of confidence among the deciding judgements. Never a mean.
    pub confidence: Option<ConfidenceEnvelope>,
    pub contributing: Vec<Judgement>,
    /// Admissible judgements that sat below the deciding tier. They are evidence; they were not
    /// decisive.
    pub withheld: Vec<Judgement>,
    pub inadmissible: Vec<Judgement>,
    pub suppressed: Vec<SuppressedOverride>,
    pub disagreements: Vec<Disagreement>,
    pub failures: Vec<OracleFailure>,
}

impl CombinedVerdict {
    fn unknown(
        subject: impl Into<String>,
        at: UtcTimestamp,
        basis: VerdictBasis,
        inadmissible: Vec<Judgement>,
    ) -> Self {
        CombinedVerdict {
            subject: subject.into(),
            at,
            acceptable: BTreeSet::from([Position::NotEvaluable]),
            basis,
            confidence: None,
            contributing: Vec::new(),
            withheld: Vec::new(),
            inadmissible,
            suppressed: Vec::new(),
            disagreements: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// The deciding tier, or `None` when nothing decided.
    pub fn deciding_tier(&self) -> Option<EvidenceTier> {
        match self.basis {
            VerdictBasis::Decided { tier } => Some(tier),
            _ => None,
        }
    }

    /// Projects the acceptable set onto the three-valued status of `bioprism_section`.
    ///
    /// A set of size one projects to its element's status; anything else is
    /// [`OracleStatus::Underdetermined`], which is how a preserved disagreement reads to a
    /// consumer that only understands three values.
    pub fn status(&self) -> OracleStatus {
        let mut positions = self.acceptable.iter();
        match (positions.next(), positions.next()) {
            (Some(only), None) => only.to_status(),
            _ => OracleStatus::Underdetermined,
        }
    }

    pub fn is_underdetermined(&self) -> bool {
        self.status() == OracleStatus::Underdetermined
    }

    /// Whether any oracle tried and failed to override a stronger rung.
    pub fn has_suppressed_override(&self) -> bool {
        !self.suppressed.is_empty()
    }

    /// Whether the verdict rests entirely on nondeterministic evidence.
    ///
    /// Not an error — 31.14 permits judges to cover "dimensions that cannot be reduced to stronger
    /// oracles" — but a caller gating a release on this verdict should know that no artifact was
    /// actually checked.
    pub fn is_judge_only(&self) -> bool {
        !self.contributing.is_empty()
            && self
                .contributing
                .iter()
                .all(|j| j.tier.is_nondeterministic())
    }

    /// The planes this verdict establishes.
    ///
    /// Planes are orthogonal to the ladder: a schema oracle and a re-execution oracle are not in
    /// competition, they cover different ground. So this reads every admissible supporting
    /// judgement, not only the ones at the deciding tier — otherwise a mesh that checked both the
    /// bytes and the arithmetic would report having checked only the bytes.
    ///
    /// It returns nothing unless the verdict is decided as [`Position::Supported`]. A
    /// contradiction establishes nothing — finding a broken date proves the artifact wrong, not
    /// its biology right — and an underdetermined verdict establishes nothing by construction.
    pub fn establishes(&self) -> BTreeSet<Plane> {
        if self.status() != OracleStatus::Valid {
            return BTreeSet::new();
        }
        self.admissible()
            .filter(|j| j.position == Position::Supported)
            .flat_map(|j| j.establishes.iter().copied())
            .collect()
    }

    /// The planes at least one participating oracle explicitly disclaimed, and none established.
    ///
    /// This is 40.21 invariant 1 made readable at the mesh level: the list of things this verdict
    /// must not be quoted as evidence for.
    pub fn does_not_establish(&self) -> BTreeSet<Plane> {
        let established = self.establishes();
        self.admissible()
            .flat_map(|j| j.cannot_establish.iter().copied())
            .filter(|plane| !established.contains(plane))
            .collect()
    }

    /// Every admissible judgement, decisive or not.
    pub fn admissible(&self) -> impl Iterator<Item = &Judgement> {
        self.contributing.iter().chain(self.withheld.iter())
    }

    /// Every finding from the judgements that decided the verdict.
    ///
    /// Findings from `withheld` judgements are excluded: a suppressed judge's remark is on the
    /// record in [`CombinedVerdict::withheld`], but quoting it here would let it read as support
    /// for a verdict it argued against.
    pub fn findings(&self) -> Vec<&Finding> {
        self.contributing
            .iter()
            .flat_map(|j| j.findings.iter())
            .collect()
    }

    /// Projects onto `bioprism_section::OracleVerdict`.
    ///
    /// Lossy in the same three ways [`Judgement::to_verdict`] is, plus a fourth specific to
    /// combination: the disagreement records, the suppressed overrides, and the inadmissible
    /// judgements have nowhere to go in the older shape. A consumer that needs to know a judge
    /// was refused must read the [`CombinedVerdict`].
    pub fn to_verdict(&self, oracle_kind: impl Into<String>) -> OracleVerdict {
        let witnesses: Vec<LeakageWitness> = self
            .findings()
            .into_iter()
            .filter_map(|finding| match finding {
                Finding::Leakage { witness } => Some(witness.clone()),
                _ => None,
            })
            .collect();
        OracleVerdict {
            status: self.status(),
            witnesses,
            oracle_kind: oracle_kind.into(),
        }
    }
}
