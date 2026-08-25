//! The research request: the one document that starts a run.
//!
//! [`ResearchRequestDocument`] is the serde-facing JSON contract with `deny_unknown_fields` — a
//! request with a silently ignored field is a request the author misread. [`ResearchRequest`] has
//! private fields and is constructed only through validation, so holding a request value *is* the
//! proof it was checked; serde routes through the same validation.
//!
//! The `question` field is recorded verbatim and **never interpreted**. The runner executes the
//! protocol the other fields declare; it does not understand the question, and nothing anywhere
//! in this crate branches on the question's content. The field exists so the dossier and report
//! can state what was asked next to what was measured, and the reader — not this code — judges
//! whether the measurements bear on it.
//!
//! The world families are `bioprism_worldgen`'s committed presets (blueprint 43.39). The request
//! chooses a preset, a seed, and up to [`MAX_DISTRACTOR_POINTS`] distractor counts; it cannot
//! reach the generator's other knobs. That is a deliberate ceiling, stated in `lib.rs` rather
//! than implied away.

use crate::error::ResearchError;
use bioprism_ids::ContentHash;
use bioprism_worldgen::WorldSpec;
use serde::{Deserialize, Serialize};

/// Most distractor counts one request may measure. Small on purpose: a protocol is a handful of
/// declared points, not a parameter scan.
pub const MAX_DISTRACTOR_POINTS: usize = 6;
/// Largest distractor count per point. Matches the order of magnitude the committed sweeps use;
/// beyond it a single comparison stops being an interactive measurement.
pub const MAX_DISTRACTORS_PER_POINT: u32 = 2000;
/// Longest accepted question, in bytes. The question is carried verbatim into every dossier and
/// report, so an unbounded one would bloat both without adding any executable content.
pub const MAX_QUESTION_BYTES: usize = 4096;
/// Longest accepted research id.
pub const MAX_RESEARCH_ID_CHARS: usize = 64;

/// The four committed `bioprism_worldgen` presets a request may choose from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorldFamily {
    ReferenceLike,
    Discriminating,
    ExternalConfirmation,
    PolicyRestricted,
}

impl WorldFamily {
    pub const ALL: [WorldFamily; 4] = [
        WorldFamily::ReferenceLike,
        WorldFamily::Discriminating,
        WorldFamily::ExternalConfirmation,
        WorldFamily::PolicyRestricted,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WorldFamily::ReferenceLike => "reference_like",
            WorldFamily::Discriminating => "discriminating",
            WorldFamily::ExternalConfirmation => "external_confirmation",
            WorldFamily::PolicyRestricted => "policy_restricted",
        }
    }

    /// The preset spec at one distractor count, reseeded and renamed for this run.
    ///
    /// Only `seed` and `world_id` are overridden; every other knob keeps the preset's committed
    /// value, so a dossier world is the preset world at the request's seed — nothing more.
    pub fn spec(self, distractors: u32, seed: u64) -> WorldSpec {
        let mut spec = match self {
            WorldFamily::ReferenceLike => WorldSpec::reference_like(distractors as usize),
            WorldFamily::Discriminating => WorldSpec::discriminating(distractors as usize),
            WorldFamily::ExternalConfirmation => {
                WorldSpec::external_confirmation(distractors as usize)
            }
            WorldFamily::PolicyRestricted => WorldSpec::policy_restricted(distractors as usize),
        };
        spec.seed = seed;
        spec.world_id = format!("research-{}-d{distractors}", self.as_str());
        spec
    }
}

/// The JSON shape of a request. Unknown fields are refused at parse time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResearchRequestDocument {
    /// Names the run in every artifact and filename; restricted to `[A-Za-z0-9._-]`.
    pub research_id: String,
    /// Free text, recorded verbatim in the dossier and report. Never interpreted: the runner
    /// executes the protocol; it does not understand the question.
    pub question: String,
    pub family: WorldFamily,
    /// The distractor counts to measure, in the order given. One to six points, each at most
    /// [`MAX_DISTRACTORS_PER_POINT`], no duplicates — a repeated point would rerun an identical
    /// deterministic measurement and inflate the protocol without adding evidence.
    pub distractor_points: Vec<u32>,
    /// Seed for every generated world in this run. The sweep step is the one exception: it runs
    /// the committed default grid at the grid's own seed, because that grid *is* the benchmark.
    pub seed: u64,
    #[serde(default)]
    pub run_sweep: bool,
    #[serde(default)]
    pub run_mutation: bool,
    #[serde(default)]
    pub run_minimize: bool,
}

/// A validated request. Constructed only via `TryFrom<ResearchRequestDocument>` (or serde, which
/// routes through the same validation), so an invalid request value cannot exist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ResearchRequestDocument", into = "ResearchRequestDocument")]
pub struct ResearchRequest {
    research_id: String,
    question: String,
    family: WorldFamily,
    distractor_points: Vec<u32>,
    seed: u64,
    run_sweep: bool,
    run_mutation: bool,
    run_minimize: bool,
}

fn valid_research_id(id: &str) -> bool {
    !id.is_empty()
        && id.chars().count() <= MAX_RESEARCH_ID_CHARS
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

impl TryFrom<ResearchRequestDocument> for ResearchRequest {
    type Error = ResearchError;

    fn try_from(document: ResearchRequestDocument) -> Result<Self, Self::Error> {
        if !valid_research_id(&document.research_id) {
            return Err(ResearchError::InvalidRequest {
                reason: format!(
                    "research_id {:?} must be 1..={MAX_RESEARCH_ID_CHARS} characters from \
                     [A-Za-z0-9._-]",
                    document.research_id
                ),
            });
        }
        if document.question.trim().is_empty() {
            return Err(ResearchError::InvalidRequest {
                reason: "question must not be empty: it is recorded verbatim (never interpreted) \
                         and an empty record states nothing"
                    .into(),
            });
        }
        if document.question.len() > MAX_QUESTION_BYTES {
            return Err(ResearchError::InvalidRequest {
                reason: format!(
                    "question is {} bytes; the cap is {MAX_QUESTION_BYTES} bytes because the \
                     question is carried verbatim into every dossier and report",
                    document.question.len()
                ),
            });
        }
        if document.distractor_points.is_empty() {
            return Err(ResearchError::InvalidRequest {
                reason: "distractor_points must name at least one point: a protocol with no \
                         measurement points measures nothing"
                    .into(),
            });
        }
        if document.distractor_points.len() > MAX_DISTRACTOR_POINTS {
            return Err(ResearchError::InvalidRequest {
                reason: format!(
                    "{} distractor points exceed the maximum of {MAX_DISTRACTOR_POINTS}",
                    document.distractor_points.len()
                ),
            });
        }
        let mut seen = std::collections::BTreeSet::new();
        for point in &document.distractor_points {
            if *point > MAX_DISTRACTORS_PER_POINT {
                return Err(ResearchError::InvalidRequest {
                    reason: format!(
                        "distractor point {point} exceeds the per-point ceiling of \
                         {MAX_DISTRACTORS_PER_POINT}"
                    ),
                });
            }
            if !seen.insert(*point) {
                return Err(ResearchError::InvalidRequest {
                    reason: format!(
                        "distractor point {point} is repeated; generation is deterministic, so a \
                         repeated point reruns an identical measurement and inflates the protocol"
                    ),
                });
            }
        }
        Ok(ResearchRequest {
            research_id: document.research_id,
            question: document.question,
            family: document.family,
            distractor_points: document.distractor_points,
            seed: document.seed,
            run_sweep: document.run_sweep,
            run_mutation: document.run_mutation,
            run_minimize: document.run_minimize,
        })
    }
}

impl From<ResearchRequest> for ResearchRequestDocument {
    fn from(request: ResearchRequest) -> Self {
        ResearchRequestDocument {
            research_id: request.research_id,
            question: request.question,
            family: request.family,
            distractor_points: request.distractor_points,
            seed: request.seed,
            run_sweep: request.run_sweep,
            run_mutation: request.run_mutation,
            run_minimize: request.run_minimize,
        }
    }
}

impl ResearchRequest {
    pub fn research_id(&self) -> &str {
        &self.research_id
    }

    /// The question exactly as authored. Recorded, rendered, digested — never interpreted.
    pub fn question(&self) -> &str {
        &self.question
    }

    pub fn family(&self) -> WorldFamily {
        self.family
    }

    pub fn distractor_points(&self) -> &[u32] {
        &self.distractor_points
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn run_sweep(&self) -> bool {
        self.run_sweep
    }

    pub fn run_mutation(&self) -> bool {
        self.run_mutation
    }

    pub fn run_minimize(&self) -> bool {
        self.run_minimize
    }

    /// The first declared point: the base world for the mutation and minimization steps.
    pub fn base_point(&self) -> u32 {
        self.distractor_points[0]
    }

    /// The request document as a canonical JSON value, for embedding in the dossier.
    pub fn to_document_value(&self) -> Result<serde_json::Value, ResearchError> {
        serde_json::to_value(ResearchRequestDocument::from(self.clone())).map_err(|error| {
            ResearchError::Canonicalisation {
                reason: error.to_string(),
            }
        })
    }

    /// Canonical content digest of the request document, stamped into every dossier so a reader
    /// can check which request a run executed.
    pub fn digest(&self) -> Result<String, ResearchError> {
        let value = self.to_document_value()?;
        ContentHash::of_value(&value)
            .map(|digest| digest.to_string())
            .map_err(|error| ResearchError::Canonicalisation {
                reason: error.to_string(),
            })
    }
}
