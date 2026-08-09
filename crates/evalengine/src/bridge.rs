//! Provenance, content addressing, and the oracle-verdict boundary.
//!
//! Two blueprint invariants from 07.01 need machinery that lives outside this crate, and this
//! module is where they attach:
//!
//! - "every reported result is linked to an immutable run, task, benchmark-pack, architecture,
//!   model, evaluator, and environment version" — [`Provenance`], built on `bioprism-ids`' typed
//!   identifiers so that a run id and a world id cannot be swapped by accident;
//! - "changes to schemas and scoring semantics are versioned and cannot retroactively rewrite
//!   already published results" — [`digest`], which content-addresses a report over
//!   `bioprism-ids`' canonical byte form. Two reports with the same digest are the same report,
//!   and a schema bump changes every digest, which is the intended loud failure.
//!
//! # Why the verdict boundary is a conversion and not an inheritance
//!
//! `bioprism-section`'s [`OracleVerdict`] answers a question about a *claim*: did the split hold,
//! is there a leakage witness. It says nothing whatever about the run's reasoning, which is the
//! second axis of [`crate::score`]. So [`contribution_from_verdict`] requires the caller to supply
//! a [`Justification`] rather than inventing one, and a caller who has not examined the reasoning
//! and honestly says [`Justification::Unexamined`] gets [`Conclusion::JustificationUnexamined`] —
//! not a pass. A valid verdict is evidence that the outcome was right, and that is all it is.
//!
//! Leakage witnesses become [`EvidenceRef`]s and deliberately **not** vetoes. A
//! [`LeakageWitness`] in this workspace describes the *benchmark's* split integrity — the thing the
//! oracle is judging — whereas [`crate::score::VetoKind::DataLeakage`] describes an agent leaking
//! held-out data during a run. They share a word and nothing else, and collapsing them would let a
//! benchmark's own design flaw fail the agent.

use bioprism_ids::{ContentHash, QueryId, RunId, WorldId};
use bioprism_section::{OracleStatus, OracleVerdict};
use serde::{Deserialize, Serialize};

use crate::error::EvalError;
use crate::ladder::{Contribution, EvidenceRef, ScoreTier};
use crate::score::{Conclusion, Justification, Outcome, ResultScore};

/// The immutable objects a reported result points back at.
///
/// Typed rather than stringly, because 07.01's invariant is that "the platform distinguishes a
/// benchmark family, a parent task, a generated instance, an execution trial, and a scored result;
/// these identifiers are never conflated" — and a `String` field named `run` conflates them the
/// first time somebody copies the wrong variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub run: RunId,
    pub world: WorldId,
    pub query: QueryId,
    /// Evaluator implementation version, environment version, oracle version. Free-form because
    /// the set differs per backend, but never empty in a published report.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub versions: Vec<(String, String)>,
}

impl Provenance {
    pub fn new(run: RunId, world: WorldId, query: QueryId) -> Self {
        Provenance {
            run,
            world,
            query,
            versions: Vec::new(),
        }
    }

    pub fn with_version(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.versions.push((key.into(), value.into()));
        self
    }

    /// Whether this provenance names every version a replay would need.
    pub fn is_replayable(&self) -> bool {
        !self.versions.is_empty()
    }
}

/// Content-address any serializable report over canonical bytes.
///
/// Uses `bioprism-ids`' canonicalization so that a digest computed here matches one computed by a
/// consumer in another language — the cross-language replay property 43.26 requires of
/// certificates, applied to evaluation reports.
pub fn digest<T: Serialize>(value: &T) -> Result<ContentHash, EvalError> {
    let json = serde_json::to_value(value).map_err(|error| EvalError::NotCanonicalizable {
        detail: error.to_string(),
    })?;
    ContentHash::of_value(&json).map_err(|error| EvalError::NotCanonicalizable {
        detail: error.to_string(),
    })
}

/// Turn an oracle verdict into a tiered contribution.
///
/// The status maps onto the *outcome* axis only. The justification axis must be supplied, because
/// no oracle in this workspace examines the run's reasoning, and defaulting it to
/// [`Justification::Supported`] would manufacture full passes out of a leakage check.
pub fn contribution_from_verdict(
    verdict: &OracleVerdict,
    tier: ScoreTier,
    justification: Justification,
    evaluator: impl Into<String>,
) -> Contribution {
    let outcome = match verdict.status {
        OracleStatus::Valid => Outcome::Correct,
        OracleStatus::Invalid => Outcome::Incorrect,
        OracleStatus::Underdetermined => Outcome::Unknown,
    };
    let conclusion = ResultScore::new(outcome, justification).conclusion();

    let mut contribution = Contribution::new(tier, evaluator, conclusion)
        .with_note(format!("oracle `{}` returned {}", verdict.oracle_kind, verdict.status.as_str()));
    for witness in &verdict.witnesses {
        contribution = contribution.with_evidence(EvidenceRef::new("leakage_witness", witness.kind()));
    }
    contribution
}

/// The conclusion a verdict yields when nobody examined the reasoning.
///
/// Never [`Conclusion::Pass`], for any verdict. A caller reaching for "the oracle said valid, so
/// it passed" should call this and see what they actually have.
pub fn unexamined_conclusion(verdict: &OracleVerdict) -> Conclusion {
    contribution_from_verdict(
        verdict,
        ScoreTier::Deterministic,
        Justification::Unexamined,
        "oracle",
    )
    .conclusion
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_section::LeakageWitness;
    use crate::ladder::{compose, UnknownPolicy};

    fn provenance() -> Provenance {
        Provenance::new(
            RunId::parse("run-1").expect("valid"),
            WorldId::parse("world-1").expect("valid"),
            QueryId::parse("query-1").expect("valid"),
        )
    }

    fn invalid_verdict() -> OracleVerdict {
        OracleVerdict::new(
            "fiber-split/0.1",
            vec![LeakageWitness::IdentityLeakage {
                alias: "ALT-77".to_string(),
                subjects: vec!["S001".to_string(), "S003".to_string()],
                splits: vec!["train".to_string(), "test".to_string()],
            }],
        )
    }

    #[test]
    fn a_valid_verdict_with_unexamined_reasoning_does_not_become_a_pass() {
        let verdict = OracleVerdict::new("fiber-split/0.1", vec![]);
        assert_eq!(verdict.status, OracleStatus::Valid);
        assert_eq!(
            unexamined_conclusion(&verdict),
            Conclusion::JustificationUnexamined
        );
        assert!(!unexamined_conclusion(&verdict).is_full_pass());
    }

    #[test]
    fn no_verdict_status_yields_a_full_pass_without_an_examined_justification() {
        for verdict in [
            OracleVerdict::new("fiber-split/0.1", vec![]),
            invalid_verdict(),
            OracleVerdict::abstain("fiber-split/0.1", vec![]),
        ] {
            assert!(!unexamined_conclusion(&verdict).is_full_pass());
        }
    }

    #[test]
    fn leakage_witnesses_become_evidence_rather_than_vetoes() {
        let contribution = contribution_from_verdict(
            &invalid_verdict(),
            ScoreTier::Deterministic,
            Justification::Supported,
            "split-oracle@1",
        );
        assert_eq!(contribution.conclusion, Conclusion::Fail);
        assert_eq!(contribution.evidence.len(), 1);
        assert_eq!(contribution.evidence[0].kind, "leakage_witness");
        assert!(contribution.veto.is_none());
    }

    #[test]
    fn an_underdetermined_verdict_composes_to_unknown_not_to_failure() {
        let verdict = OracleVerdict::abstain("fiber-split/0.1", vec![]);
        let scored = compose(
            "r1",
            &[contribution_from_verdict(
                &verdict,
                ScoreTier::Deterministic,
                Justification::Supported,
                "split-oracle@1",
            )],
            &UnknownPolicy::Block,
        )
        .expect("composes");
        assert_eq!(scored.conclusion, Conclusion::Unknown);
    }

    #[test]
    fn identical_reports_share_a_digest_and_altered_ones_do_not() {
        let first = digest(&provenance()).expect("hashable");
        let second = digest(&provenance()).expect("hashable");
        assert_eq!(first, second);

        let altered = digest(&provenance().with_version("evaluator", "2")).expect("hashable");
        assert_ne!(first, altered);
    }

    #[test]
    fn a_provenance_without_versions_is_not_replayable() {
        assert!(!provenance().is_replayable());
        assert!(provenance()
            .with_version("environment", "img-3")
            .is_replayable());
    }

    #[test]
    fn typed_identifiers_keep_a_run_from_being_passed_as_a_world() {
        let provenance = provenance();
        assert_eq!(provenance.run.as_str(), "run-1");
        assert_eq!(provenance.world.as_str(), "world-1");
        assert_eq!(RunId::KIND, "run");
        assert_ne!(RunId::KIND, WorldId::KIND);
    }
}
