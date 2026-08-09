//! The Decision Cell.
//!
//! Blueprint 03.04 and the executive summary: PRISM changes the unit of evaluation from *task
//! result* to *executable decision state*. A cell freezes everything a candidate needs in order to
//! resume from one meaningful point — here, the world, the query, and the acceptance contract —
//! so that two candidates can be compared as a matched experiment rather than an uncontrolled
//! end-to-end run.
//!
//! The acceptance contract is **set-valued** (03.07). A cell does not assert one correct answer;
//! it declares which verdicts are acceptable and which witnesses must be present. That is what
//! lets a cell accept two different-but-equally-correct continuations without scoring one of them
//! wrong.

use bioprism_ids::ContentHash;
use bioprism_section::OracleStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;

pub const CELL_SCHEMA_VERSION: &str = "bioprism-decision-cell/0.1";

/// A content-addressed reference to an input.
///
/// The digest is what makes a cell replayable: a bundle that names a world by path alone cannot
/// be verified by a third party, because the path may now hold something else.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRef {
    pub locator: String,
    pub sha256: String,
}

impl InputRef {
    pub fn new(locator: impl Into<String>, document: &Value) -> Self {
        InputRef {
            locator: locator.into(),
            sha256: ContentHash::of_value(document)
                .expect("document is finite JSON")
                .as_str()
                .to_string(),
        }
    }

    pub fn matches(&self, document: &Value) -> bool {
        ContentHash::of_value(document)
            .map(|digest| digest.as_str() == self.sha256)
            .unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionCell {
    pub schema_version: String,
    pub cell_id: String,
    /// What decision this cell freezes, in one line, for a human reading a failure report.
    pub decision_point: String,
    pub world: InputRef,
    pub query: InputRef,
    /// Verdicts that count as passing. Set-valued by design (03.07).
    pub acceptable_verdicts: BTreeSet<String>,
    /// Witnesses a candidate must surface. A candidate that reaches the right verdict without
    /// these has not demonstrated it for the right reason.
    pub required_witnesses: BTreeSet<String>,
    /// Whether the mandatory protected closure must be delivered in full (43.13).
    pub require_protected_closure: bool,
}

impl DecisionCell {
    pub fn new(
        cell_id: impl Into<String>,
        decision_point: impl Into<String>,
        world: InputRef,
        query: InputRef,
    ) -> Self {
        DecisionCell {
            schema_version: CELL_SCHEMA_VERSION.to_string(),
            cell_id: cell_id.into(),
            decision_point: decision_point.into(),
            world,
            query,
            acceptable_verdicts: BTreeSet::new(),
            required_witnesses: BTreeSet::new(),
            require_protected_closure: true,
        }
    }

    pub fn accepting(mut self, status: OracleStatus) -> Self {
        self.acceptable_verdicts.insert(status.as_str().to_string());
        self
    }

    pub fn requiring_witness(mut self, kind: impl Into<String>) -> Self {
        self.required_witnesses.insert(kind.into());
        self
    }

    /// Whether an observed outcome satisfies the cell.
    pub fn accepts(
        &self,
        status: OracleStatus,
        witnesses: &BTreeSet<String>,
        closure_complete: bool,
    ) -> Acceptance {
        if !self.acceptable_verdicts.is_empty()
            && !self.acceptable_verdicts.contains(status.as_str())
        {
            return Acceptance::WrongVerdict {
                observed: status.as_str().to_string(),
            };
        }
        let missing: Vec<String> = self
            .required_witnesses
            .difference(witnesses)
            .cloned()
            .collect();
        if !missing.is_empty() {
            return Acceptance::MissingWitnesses(missing);
        }
        if self.require_protected_closure && !closure_complete {
            return Acceptance::ClosureIncomplete;
        }
        Acceptance::Passed
    }

    pub fn digest(&self) -> ContentHash {
        ContentHash::of_value(&serde_json::to_value(self).expect("cell is serialisable"))
            .expect("cell is finite JSON")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Acceptance {
    Passed,
    WrongVerdict { observed: String },
    MissingWitnesses(Vec<String>),
    /// Right answer, incomplete basis. Recorded distinctly because it is the failure mode that
    /// most looks like a pass.
    ClosureIncomplete,
}

impl Acceptance {
    pub fn passed(&self) -> bool {
        matches!(self, Acceptance::Passed)
    }

    pub fn reason(&self) -> String {
        match self {
            Acceptance::Passed => "passed".into(),
            Acceptance::WrongVerdict { observed } => format!("verdict {observed} is not acceptable"),
            Acceptance::MissingWitnesses(kinds) => {
                format!("missing required witnesses: {}", kinds.join(", "))
            }
            Acceptance::ClosureIncomplete => {
                "protected closure was not delivered in full".into()
            }
        }
    }
}
