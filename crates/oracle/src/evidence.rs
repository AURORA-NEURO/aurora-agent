//! What an oracle is handed.
//!
//! 40.21 lists the runtime's inputs as the world/cell outcome, the oracle definitions,
//! evaluator-only evidence, and the combination policy. [`Evidence`] is the first and third of
//! those: the artifact under judgement plus the identity of what produced it.
//!
//! Two fields are load-bearing rather than decorative.
//!
//! `at` is the instant the evaluation claims to happen at, and it is an input rather than a clock
//! read. That is what makes 31.16 expiration testable and what keeps a replayed run from
//! silently changing verdict because a validity window closed in the meantime. It is also the
//! only defensible reading of 31.01's "a later event can evaluate but cannot enter an earlier
//! context snapshot" — evaluation time is part of the evidence, not part of the environment.
//!
//! `world` and `run` carry the identity 40.21's failure semantics require on every typed event.
//!
//! Not implemented: the evaluator-holdout isolation of 40.21 ("do not expose evaluator holdouts,
//! hidden labels, or private world state to evaluated agents"). This type is a plain container;
//! nothing here prevents an oracle implementation from being handed a holdout. Enforcing that
//! boundary is a process-isolation concern, and 40.21 places it in dependency 12.

use std::collections::BTreeMap;

use bioprism_ids::{RunId, WorldId};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::OracleError;
use crate::time::UtcTimestamp;

/// The artifact under judgement, plus the identity and instant that situate it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// A stable locator for whatever is being judged — a result bundle, a cell outcome, a claim.
    /// It is the key a scripted judge is keyed by and the subject reported on the verdict.
    pub subject: String,
    /// The instant this evaluation is claimed to occur at. Supplied, never read from a clock.
    pub at: UtcTimestamp,
    /// The artifact itself, as a JSON document keyed by top-level field.
    pub artifact: BTreeMap<String, Value>,
    pub world: Option<WorldId>,
    pub run: Option<RunId>,
}

impl Evidence {
    pub fn new(subject: impl Into<String>, at: UtcTimestamp) -> Self {
        Evidence {
            subject: subject.into(),
            at,
            artifact: BTreeMap::new(),
            world: None,
            run: None,
        }
    }

    pub fn with_field(mut self, pointer: impl Into<String>, value: Value) -> Self {
        self.artifact.insert(pointer.into(), value);
        self
    }

    pub fn with_world(mut self, world: WorldId) -> Self {
        self.world = Some(world);
        self
    }

    pub fn with_run(mut self, run: RunId) -> Self {
        self.run = Some(run);
        self
    }

    pub fn field(&self, pointer: &str) -> Option<&Value> {
        self.artifact.get(pointer)
    }

    /// Reads a field the caller has asserted must exist.
    ///
    /// Oracles should generally prefer [`Evidence::field`] and report an absence as a
    /// [`crate::Finding`], because "the field you require is missing" is usually a judgement about
    /// the artifact. This exists for the narrower case where the harness itself cannot proceed.
    pub fn require_field(&self, pointer: &str) -> Result<&Value, OracleError> {
        self.artifact
            .get(pointer)
            .ok_or_else(|| OracleError::MissingEvidenceField {
                subject: self.subject.clone(),
                pointer: pointer.to_string(),
            })
    }

    /// Reads a field as an `f64`, accepting only JSON numbers.
    ///
    /// Numeric strings are rejected rather than coerced: a survival table storing `"3.5"` where a
    /// number belongs is a schema defect, and quietly parsing it here would hide the defect from
    /// the deterministic oracle whose job is to find it.
    pub fn number(&self, pointer: &str) -> Option<f64> {
        self.artifact.get(pointer).and_then(Value::as_f64)
    }
}
