//! A scripted stand-in for a semantic judge (31.14).
//!
//! **This is not a language model and does not call one.** It replays a transcript: a map from
//! evidence subject to a position and a confidence, decided in advance by whoever wrote the test.
//! It exists so that the override rules of the ladder can be exercised deterministically, which a
//! real judge could never do — a test whose outcome depends on a sampled generation is not a test
//! of the combination rule.
//!
//! Its manifest is the honest part. It sits at [`EvidenceTier::Judge`], establishes only
//! [`Plane::Policy`], and disclaims the other seven. 31.14's worked case is precisely this
//! boundary: "A judge may score whether a plan acknowledges confounding, but cannot override a
//! deterministic finding that the cohort split leaks patients." The first clause is
//! [`Plane::Policy`]; the second is what [`crate::combine`] enforces.
//!
//! Not implemented, and needed before any real judge is admitted to a mesh: blinding of system
//! identity, multiple judge families, calibration against experts, and style- and verbosity-bias
//! detection — all four are required functions in 31.14. A judge without them is an unmeasured
//! instrument, and the ladder's refusal to let it outrank anything grounded is the only protection
//! this crate offers in the meantime.

use std::collections::BTreeMap;

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::{Confidence, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::{OracleId, OracleManifest, OracleRef, OracleVersion, UncertaintyModel};
use crate::oracle::Oracle;
use crate::plane::Plane;
use crate::time::ValidityWindow;

/// A judge whose answers are fixed in advance, keyed by [`Evidence::subject`].
pub struct MockJudgeOracle {
    manifest: OracleManifest,
    rubric: String,
    scripted: BTreeMap<String, ScriptedOpinion>,
}

struct ScriptedOpinion {
    position: Position,
    confidence: Confidence,
    remark: String,
}

impl MockJudgeOracle {
    /// Builds a judge at [`EvidenceTier::Judge`], establishing only [`Plane::Policy`].
    pub fn new(
        id: impl Into<String>,
        version: OracleVersion,
        validity: ValidityWindow,
        rubric: impl Into<String>,
    ) -> Result<Self, OracleError> {
        let manifest = OracleManifest::new(
            OracleRef::new(OracleId::parse(id)?, version),
            EvidenceTier::Judge,
            [Plane::Policy],
            [],
            validity,
        )?
        .disclaiming_the_rest()
        .with_uncertainty_model(UncertaintyModel::AcceptableSet)
        .with_failure_mode("scripted; it replays a transcript and generalises to nothing")
        .with_failure_mode("system identity is not blinded, so 31.14's bias controls are absent")
        .with_failure_mode("never calibrated against experts, so its confidence is uncalibrated");

        Ok(MockJudgeOracle {
            manifest,
            rubric: rubric.into(),
            scripted: BTreeMap::new(),
        })
    }

    /// Scripts an opinion for one subject.
    pub fn scripting(
        mut self,
        subject: impl Into<String>,
        position: Position,
        confidence: Confidence,
        remark: impl Into<String>,
    ) -> Self {
        self.scripted.insert(
            subject.into(),
            ScriptedOpinion {
                position,
                confidence,
                remark: remark.into(),
            },
        );
        self
    }

    pub fn manifest_mut(&mut self) -> &mut OracleManifest {
        &mut self.manifest
    }

    pub fn rubric(&self) -> &str {
        &self.rubric
    }
}

impl Oracle for MockJudgeOracle {
    fn manifest(&self) -> &OracleManifest {
        &self.manifest
    }

    /// An unscripted subject abstains rather than guessing. 31.14's required functions include
    /// "allow abstention and appeal", and a judge that answers every question it is asked is
    /// exactly the false-certainty failure its metrics list.
    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError> {
        let Some(opinion) = self.scripted.get(&evidence.subject) else {
            return Ok(Judgement::from_manifest(
                &self.manifest,
                &evidence.at,
                Position::NotEvaluable,
                Confidence::CERTAIN,
            )
            .with_rationale(format!(
                "no scripted opinion for subject {:?} under rubric {:?}",
                evidence.subject, self.rubric
            )));
        };

        Ok(Judgement::from_manifest(
            &self.manifest,
            &evidence.at,
            opinion.position,
            opinion.confidence,
        )
        .with_findings([Finding::Remark {
            rubric: self.rubric.clone(),
            text: opinion.remark.clone(),
        }])
        .with_rationale(format!("scripted opinion under rubric {:?}", self.rubric)))
    }
}
