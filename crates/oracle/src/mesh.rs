//! The oracle mesh: run heterogeneous oracles, combine under a declared policy (40.21).
//!
//! 40.21's execution path is: authorize the evaluator context, execute independent oracles,
//! validate outputs, combine under the declared policy, emit the verdict and evidence graph. This
//! type is steps two through five.
//!
//! Step one is not implemented. There is no authorization here, and no isolation: an oracle
//! registered in this mesh runs in-process with full access to whatever [`Evidence`] it is handed.
//! 40.21 requires "Oracle data are isolated" and 31.01 requires oracle code to execute "in an
//! isolated grader environment"; both are process- and deployment-level guarantees that a library
//! cannot make on its own, and pretending otherwise here would be worse than the gap.
//!
//! # A failing oracle does not fail the mesh
//!
//! If one oracle returns an [`OracleError`], the mesh records an [`OracleFailure`] and continues.
//! That follows 40.21's failure semantics, which list "oracle unavailable" as a condition to be
//! emitted as a typed event, and invariant 4, "Unknown remains a valid result". A mesh that
//! aborted on the first broken grader would make every other oracle's evidence unavailable
//! because of a defect unrelated to any of them.

use crate::combine::{CombinedVerdict, MeshPolicy, OracleFailure, RetryClass};
use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::Judgement;
use crate::oracle::Oracle;

/// A registered collection of oracles plus the policy under which their judgements combine.
pub struct OracleMesh {
    oracles: Vec<Box<dyn Oracle>>,
    policy: MeshPolicy,
}

impl OracleMesh {
    pub fn new(policy: MeshPolicy) -> Self {
        OracleMesh {
            oracles: Vec::new(),
            policy,
        }
    }

    /// Registers an oracle.
    ///
    /// Rejects a duplicate `kind` at the same version, because an observation whose author cannot
    /// be identified is not auditable, and because a mesh holding one oracle twice would count its
    /// position twice — the closest thing to a vote this design permits, arrived at by accident.
    /// The same `kind` at a *different* version is allowed: 31.15 lists version disagreement as a
    /// classification worth detecting, which requires being able to run both.
    pub fn register(&mut self, oracle: Box<dyn Oracle>) -> Result<(), OracleError> {
        let incoming = &oracle.manifest().oracle;
        if self
            .oracles
            .iter()
            .any(|existing| &existing.manifest().oracle == incoming)
        {
            return Err(OracleError::DuplicateOracle {
                oracle: incoming.to_string(),
            });
        }
        self.oracles.push(oracle);
        Ok(())
    }

    /// Registers an oracle, returning the mesh for chaining.
    pub fn with(mut self, oracle: Box<dyn Oracle>) -> Result<Self, OracleError> {
        self.register(oracle)?;
        Ok(self)
    }

    pub fn len(&self) -> usize {
        self.oracles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.oracles.is_empty()
    }

    pub fn policy(&self) -> &MeshPolicy {
        &self.policy
    }

    /// Runs every oracle over the evidence and combines the results.
    ///
    /// The only error is [`OracleError::EmptyMesh`]: a verdict from no oracles would carry the
    /// authority of a mesh with none of the evidence.
    pub fn evaluate(&self, evidence: &Evidence) -> Result<CombinedVerdict, OracleError> {
        if self.oracles.is_empty() {
            return Err(OracleError::EmptyMesh);
        }

        let mut judgements: Vec<Judgement> = Vec::with_capacity(self.oracles.len());
        let mut failures: Vec<OracleFailure> = Vec::new();

        for oracle in &self.oracles {
            match oracle.evaluate(evidence) {
                Ok(judgement) => judgements.push(judgement),
                Err(error) => failures.push(OracleFailure {
                    oracle: oracle.manifest().oracle.clone(),
                    error: error.to_string(),
                    retry: RetryClass::Permanent,
                }),
            }
        }

        let mut verdict = self
            .policy
            .combine(evidence.subject.clone(), &evidence.at, judgements);
        verdict.failures = failures;
        Ok(verdict)
    }
}
