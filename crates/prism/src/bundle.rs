//! Result bundles and attestation.
//!
//! Blueprint 03.12: package results so a third party can reconstruct what was run, scored, claimed
//! and limited. The MVP release gate is stricter still — "a third party rebuilds and verifies the
//! Result Bundle" — which means a bundle must carry its inputs by digest, its reproduction
//! command, and a self-verifying attestation.
//!
//! The `limitations` field is not decoration. A bundle that reports a result without stating what
//! the result does not establish is the artifact 43.43 exists to prevent.

use crate::cell::DecisionCell;
use crate::fork::ForkResult;
use crate::minimize::Minimization;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const BUNDLE_SCHEMA_VERSION: &str = "bioprism-result-bundle/0.1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultBundle {
    pub schema_version: String,
    pub cell: DecisionCell,
    pub fork: ForkResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimization: Option<Minimization>,
    pub reproduction: Reproduction,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reproduction {
    pub command: String,
    pub compiler_version: String,
    /// Whether every step is deterministic. A bundle that cannot claim this is exploratory.
    pub deterministic: bool,
}

impl ResultBundle {
    pub fn new(cell: DecisionCell, fork: ForkResult) -> Self {
        let command = format!(
            "bioprism context compare --world {} --query {}",
            cell.world.locator, cell.query.locator
        );
        ResultBundle {
            schema_version: BUNDLE_SCHEMA_VERSION.to_string(),
            cell,
            fork,
            minimization: None,
            reproduction: Reproduction {
                command,
                compiler_version: env!("CARGO_PKG_VERSION").to_string(),
                deterministic: true,
            },
            limitations: vec![
                "Judged by a deterministic oracle on one world and one query; establishes nothing \
                 about other worlds or other queries."
                    .into(),
                "Context policy is the only component varied. Model identity, prompt and tooling \
                 are not evaluated here."
                    .into(),
            ],
        }
    }

    pub fn with_minimization(mut self, minimization: Minimization) -> Self {
        self.minimization = Some(minimization);
        self
    }

    pub fn limited_by(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    fn body(&self) -> Value {
        serde_json::to_value(self).expect("bundle is serialisable")
    }

    /// The bundle with its attestation attached.
    pub fn attest(&self) -> Value {
        let body = self.body();
        let digest = ContentHash::of_value(&body).expect("bundle is finite JSON");
        let mut map: Map<String, Value> = body.as_object().expect("object").clone();
        map.insert("bundle_sha256".into(), json!(digest.as_str()));
        Value::Object(map)
    }

    /// Recomputes a bundle's attestation, the way a third party must before trusting it.
    pub fn verify(document: &Value) -> Attestation {
        let Some(map) = document.as_object() else {
            return Attestation::Malformed("not an object".into());
        };
        let Some(claimed) = map.get("bundle_sha256").and_then(Value::as_str) else {
            return Attestation::Malformed("missing bundle_sha256".into());
        };
        let mut body = map.clone();
        body.remove("bundle_sha256");
        match ContentHash::of_value(&Value::Object(body)) {
            Ok(recomputed) if recomputed.as_str() == claimed => Attestation::Valid,
            Ok(recomputed) => Attestation::Mismatch {
                claimed: claimed.to_string(),
                recomputed: recomputed.as_str().to_string(),
            },
            Err(error) => Attestation::Malformed(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attestation {
    Valid,
    Mismatch { claimed: String, recomputed: String },
    Malformed(String),
}

impl Attestation {
    pub fn is_valid(&self) -> bool {
        matches!(self, Attestation::Valid)
    }
}
