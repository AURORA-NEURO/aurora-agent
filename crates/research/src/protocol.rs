//! Pure protocol assembly: a validated request maps to one ordered list of typed steps.
//!
//! [`plan_protocol`] is a pure function — no I/O, no clock, no randomness — so the same request
//! always plans the same protocol, and the dossier can echo the plan next to the executed steps
//! with nothing to reconcile. The runner executes exactly this list in exactly this order; there
//! is no scheduler, no retry, and no step the plan does not name.
//!
//! Step 0 is always [`ProtocolStep::AnchorReferenceFixture`]: the committed `fixtures/fiber-v0.1`
//! pair compiled and checked against the pinned cross-language parity digest, so every dossier is
//! anchored to the same certificate three independent implementations agree on. The remaining
//! steps come from the request: per declared distractor point, generate → compile → compare
//! (blueprint 43.39 families measured under the 43.38 equal-engineering panel), then the optional
//! sweep, mutation (03.08/32), and minimization steps.

use crate::request::ResearchRequest;
use serde::Serialize;

/// One typed step of a research protocol.
///
/// `Serialize`-only: a protocol is derived from a request, never authored directly, so there is
/// deliberately no way to deserialize one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ProtocolStep {
    /// Compile the embedded reference fixtures and require the pinned parity certificate digest.
    AnchorReferenceFixture,
    /// Generate the family preset world and query at one distractor count.
    GenerateWorld { distractors: u32 },
    /// Compile the generated pair and round-trip its certificate through verification.
    CompileFiber { distractors: u32 },
    /// Run the full 43.38 default panel over the generated pair.
    ComparePanel { distractors: u32 },
    /// Run the committed 43.39 structural family sweep at its own declared grid and seed.
    SweepStructuralGrid,
    /// Apply the standard metamorphic suite to the base world (the first declared point).
    MutateBaseWorld { distractors: u32 },
    /// Reduce the base world to a 1-minimal fact set and re-verify the reduction.
    MinimizeBaseWorld { distractors: u32 },
}

impl ProtocolStep {
    /// Human-readable label used in step records and the report's protocol table.
    pub fn label(&self) -> String {
        match self {
            ProtocolStep::AnchorReferenceFixture => "anchor reference fixture".into(),
            ProtocolStep::GenerateWorld { distractors } => {
                format!("generate world (d={distractors})")
            }
            ProtocolStep::CompileFiber { distractors } => {
                format!("compile fiber (d={distractors})")
            }
            ProtocolStep::ComparePanel { distractors } => {
                format!("compare panel (d={distractors})")
            }
            ProtocolStep::SweepStructuralGrid => "sweep structural grid".into(),
            ProtocolStep::MutateBaseWorld { distractors } => {
                format!("mutate base world (d={distractors})")
            }
            ProtocolStep::MinimizeBaseWorld { distractors } => {
                format!("minimize base world (d={distractors})")
            }
        }
    }
}

/// The planned protocol: the request's id and the ordered steps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResearchProtocol {
    pub research_id: String,
    pub steps: Vec<ProtocolStep>,
}

/// Plans the protocol for one validated request. Pure and deterministic.
pub fn plan_protocol(request: &ResearchRequest) -> ResearchProtocol {
    let mut steps = vec![ProtocolStep::AnchorReferenceFixture];
    for point in request.distractor_points() {
        steps.push(ProtocolStep::GenerateWorld {
            distractors: *point,
        });
        steps.push(ProtocolStep::CompileFiber {
            distractors: *point,
        });
        steps.push(ProtocolStep::ComparePanel {
            distractors: *point,
        });
    }
    if request.run_sweep() {
        steps.push(ProtocolStep::SweepStructuralGrid);
    }
    if request.run_mutation() {
        steps.push(ProtocolStep::MutateBaseWorld {
            distractors: request.base_point(),
        });
    }
    if request.run_minimize() {
        steps.push(ProtocolStep::MinimizeBaseWorld {
            distractors: request.base_point(),
        });
    }
    ResearchProtocol {
        research_id: request.research_id().to_string(),
        steps,
    }
}
