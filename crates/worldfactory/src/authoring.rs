//! Parent bioworld authoring (27.01).
//!
//! 27.01's *Critical design decision* is the one §27 sentence with a number in it: "Quality begins
//! with parents, not mutations. The first public release should prefer dozens of deeply audited
//! worlds over thousands of shallow tasks." Everything the mutation modules do is a multiplier on
//! whatever the parent already was — including its defects, which is the arithmetic the sentence is
//! about.
//!
//! This module is the freeze gate. A [`CandidateParent`] carries 27.01's seven required artifacts,
//! its decision map, the availability times of its evidence and the outcome of each review;
//! [`freeze`] either produces a [`FrozenParent`] or names what is missing. [`FrozenParent`] has no
//! public constructor and no `Deserialize`.
//!
//! # The two checks that are not paperwork
//!
//! **Single-path authoring.** 27.01's failure list opens with "author encodes preferred workflow as
//! only acceptable path", and 27.02's workflow step 4 asks for "alternative valid paths". A
//! decision map in which every decision point admits exactly one action does not measure
//! competence; it measures agreement with whoever wrote it. [`freeze`] refuses one, at every tier.
//!
//! **Future information.** An artifact that only became available after a decision was made is
//! evidence the decider could not have had. This is the leak that survives every other review,
//! because the artifact is genuine, the value is correct, and the only thing wrong with it is a
//! timestamp. [`freeze`] compares each decision point's declared evidence against its decision
//! time.
//!
//! # Where 27.01 names a construction it never specifies
//!
//! 27.01's workflow step 7 says "freeze a Gold or Silver parent version" and defines neither tier.
//! Rather than invent a boundary, [`Tier::Silver`] carries the caller's *declared relaxations* and
//! [`FrozenParent::relaxations`] publishes them. Gold means every artifact present and every review
//! passed; Silver means the author said out loud which of those it is missing. A tier whose
//! contents are a fixed list this crate made up would be a governance decision smuggled in as an
//! implementation detail — `crates/registry` owns trust tiers, and 27.18 is where the promotion
//! rules belong.
//!
//! # What is deliberately not here
//!
//! No replay engine, no rebuild, no oracle and no review workflow. 27.01's validation list is
//! mostly human work — biological plausibility review, exploit review, licence review — and the
//! honest thing a type can do is record whether it happened and refuse to treat "not performed" as
//! "passed". [`Reviewed`] therefore has three variants. "Clean rebuild" is likewise recorded, not
//! executed: this crate cannot run anything.

use crate::error::FreezeRefusal;
use crate::observed::SourceRef;
use bioprism_scope::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 27.01's "Required artifacts" list, verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequiredArtifact {
    WorldManifest,
    ArtifactAndLineageGraph,
    AssayLenses,
    DecisionMap,
    OracleMesh,
    BenchmarkAndDataCards,
    ReferenceTraces,
}

impl RequiredArtifact {
    pub const ALL: [RequiredArtifact; 7] = [
        RequiredArtifact::WorldManifest,
        RequiredArtifact::ArtifactAndLineageGraph,
        RequiredArtifact::AssayLenses,
        RequiredArtifact::DecisionMap,
        RequiredArtifact::OracleMesh,
        RequiredArtifact::BenchmarkAndDataCards,
        RequiredArtifact::ReferenceTraces,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            RequiredArtifact::WorldManifest => "world manifest",
            RequiredArtifact::ArtifactAndLineageGraph => "artifact and lineage graph",
            RequiredArtifact::AssayLenses => "AssayLenses",
            RequiredArtifact::DecisionMap => "decision map",
            RequiredArtifact::OracleMesh => "oracle mesh",
            RequiredArtifact::BenchmarkAndDataCards => "benchmark and data cards",
            RequiredArtifact::ReferenceTraces => "reference traces",
        }
    }
}

/// A review's outcome. Three states, and the third is not the first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "review", rename_all = "snake_case")]
pub enum Reviewed {
    Passed { reviewer: String },
    Failed { finding: String },
    /// Nobody ran it. A parent frozen with this recorded is a parent with a known gap; a parent
    /// frozen with it silently treated as a pass is a parent with an unknown one.
    NotPerformed,
}

impl Reviewed {
    pub fn passed(&self) -> bool {
        matches!(self, Reviewed::Passed { .. })
    }
}

/// 27.01's reviews, plus the rebuild its validation list requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    /// "biological plausibility review".
    pub plausibility: Reviewed,
    /// The exploit review of 27.01 workflow step 6 — the pass that looks for shortcuts rather than
    /// for errors.
    pub exploit: Reviewed,
    /// "license review".
    pub licence: Reviewed,
    /// "clean rebuild". Recorded, not executed — this crate runs nothing.
    pub clean_rebuild: bool,
}

impl ReviewRecord {
    /// A record with nothing performed. The starting point, so that a caller has to fill each field
    /// in deliberately rather than receiving passes by default.
    pub fn none_performed() -> Self {
        ReviewRecord {
            plausibility: Reviewed::NotPerformed,
            exploit: Reviewed::NotPerformed,
            licence: Reviewed::NotPerformed,
            clean_rebuild: false,
        }
    }

    fn named(&self) -> [(&'static str, &Reviewed); 3] {
        [
            ("biological plausibility", &self.plausibility),
            ("exploit", &self.exploit),
            ("licence", &self.licence),
        ]
    }
}

/// A point in the world where somebody had to choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionPoint {
    pub id: String,
    pub decided_at: Timestamp,
    /// Every action an expert would accept here. More than one is the normal case; exactly one
    /// everywhere is [`FreezeRefusal::SinglePathAuthoring`].
    pub allowed_actions: BTreeSet<String>,
    /// The evidence the decider is given, by artifact name.
    pub evidence: BTreeSet<String>,
}

impl DecisionPoint {
    pub fn new(id: impl Into<String>, decided_at: Timestamp) -> Self {
        DecisionPoint {
            id: id.into(),
            decided_at,
            allowed_actions: BTreeSet::new(),
            evidence: BTreeSet::new(),
        }
    }

    pub fn allowing(mut self, action: impl Into<String>) -> Self {
        self.allowed_actions.insert(action.into());
        self
    }

    pub fn seeing(mut self, artifact: impl Into<String>) -> Self {
        self.evidence.insert(artifact.into());
        self
    }
}

/// A candidate parent world, before it is frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateParent {
    pub id: String,
    pub artifacts: BTreeSet<RequiredArtifact>,
    pub decision_points: Vec<DecisionPoint>,
    /// When each named evidence artifact became available. A decision point that sees an artifact
    /// with a later availability is a leak.
    pub availability: BTreeMap<String, Timestamp>,
    pub reviews: ReviewRecord,
    pub sources: Vec<SourceRef>,
}

impl CandidateParent {
    pub fn new(id: impl Into<String>) -> Self {
        CandidateParent {
            id: id.into(),
            artifacts: BTreeSet::new(),
            decision_points: Vec::new(),
            availability: BTreeMap::new(),
            reviews: ReviewRecord::none_performed(),
            sources: Vec::new(),
        }
    }

    /// Attach every required artifact. Convenience for a fixture whose point is elsewhere.
    pub fn with_all_artifacts(mut self) -> Self {
        self.artifacts.extend(RequiredArtifact::ALL);
        self
    }

    pub fn with_artifact(mut self, artifact: RequiredArtifact) -> Self {
        self.artifacts.insert(artifact);
        self
    }

    pub fn with_decision(mut self, point: DecisionPoint) -> Self {
        self.decision_points.push(point);
        self
    }

    pub fn available(mut self, artifact: impl Into<String>, at: Timestamp) -> Self {
        self.availability.insert(artifact.into(), at);
        self
    }

    pub fn reviewed(mut self, reviews: ReviewRecord) -> Self {
        self.reviews = reviews;
        self
    }

    pub fn from_source(mut self, source: SourceRef) -> Self {
        self.sources.push(source);
        self
    }
}

/// 27.01 workflow step 7's two tiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tier", rename_all = "snake_case")]
pub enum Tier {
    /// Every artifact present, every review passed.
    Gold,
    /// Some named thing is missing, and the author said which. See the module header for why the
    /// relaxation set is the caller's rather than this crate's.
    Silver {
        relaxing: BTreeSet<RequiredArtifact>,
        /// Reviews the author is knowingly freezing without.
        without_reviews: BTreeSet<String>,
    },
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Gold => "Gold",
            Tier::Silver { .. } => "Silver",
        }
    }

    fn relaxes(&self, artifact: RequiredArtifact) -> bool {
        match self {
            Tier::Gold => false,
            Tier::Silver { relaxing, .. } => relaxing.contains(&artifact),
        }
    }

    fn waives_review(&self, review: &str) -> bool {
        match self {
            Tier::Gold => false,
            Tier::Silver {
                without_reviews, ..
            } => without_reviews.contains(review),
        }
    }
}

/// A parent world that passed the freeze gate. No public constructor, no `Deserialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrozenParent {
    id: String,
    tier: Tier,
    decision_points: usize,
    /// Decision points admitting more than one acceptable action, over the total. 27.01's own
    /// answer to "does this world measure competence or agreement".
    multi_path_points: usize,
    relaxations: BTreeSet<RequiredArtifact>,
    waived_reviews: BTreeSet<String>,
}

impl FrozenParent {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn tier(&self) -> &Tier {
        &self.tier
    }

    pub fn decision_points(&self) -> usize {
        self.decision_points
    }

    pub fn multi_path_points(&self) -> usize {
        self.multi_path_points
    }

    /// Artifacts this parent was frozen without. Empty for Gold, by construction.
    pub fn relaxations(&self) -> &BTreeSet<RequiredArtifact> {
        &self.relaxations
    }

    /// Reviews this parent was frozen without.
    pub fn waived_reviews(&self) -> &BTreeSet<String> {
        &self.waived_reviews
    }
}

/// Freeze a candidate parent, or say what stops it.
///
/// The order is: rebuild, artifacts, reviews, decision map, future information, embedded controlled
/// assets. Rebuild first because a world that cannot be rebuilt makes every later check a statement
/// about a thing that will never exist again.
pub fn freeze(candidate: &CandidateParent, tier: Tier) -> Result<FrozenParent, FreezeRefusal> {
    if !candidate.reviews.clean_rebuild {
        return Err(FreezeRefusal::NoCleanRebuild);
    }

    for artifact in RequiredArtifact::ALL {
        if !candidate.artifacts.contains(&artifact) && !tier.relaxes(artifact) {
            return Err(FreezeRefusal::MissingArtifact {
                artifact: artifact.as_str().to_string(),
            });
        }
    }

    for (name, outcome) in candidate.reviews.named() {
        match outcome {
            Reviewed::Failed { finding } => {
                return Err(FreezeRefusal::ReviewFailed {
                    review: name.to_string(),
                    finding: finding.clone(),
                })
            }
            Reviewed::NotPerformed if !tier.waives_review(name) => {
                return Err(FreezeRefusal::ReviewNotPerformed {
                    review: name.to_string(),
                    tier: tier.as_str().to_string(),
                })
            }
            _ => {}
        }
    }

    let multi_path_points = candidate
        .decision_points
        .iter()
        .filter(|p| p.allowed_actions.len() > 1)
        .count();
    if !candidate.decision_points.is_empty() && multi_path_points == 0 {
        return Err(FreezeRefusal::SinglePathAuthoring {
            points: candidate.decision_points.len(),
        });
    }

    for point in &candidate.decision_points {
        for artifact in &point.evidence {
            let Some(available_at) = candidate.availability.get(artifact) else {
                continue;
            };
            if *available_at > point.decided_at {
                return Err(FreezeRefusal::FutureInformation {
                    artifact: artifact.clone(),
                    decision: point.id.clone(),
                    available_at: available_at.to_rfc3339(),
                    decided_at: point.decided_at.to_rfc3339(),
                });
            }
        }
    }

    for source in &candidate.sources {
        if source.access.is_controlled() && source.embedded {
            return Err(FreezeRefusal::ControlledAssetEmbedded {
                asset: source.name.clone(),
            });
        }
    }

    let (relaxations, waived_reviews) = match &tier {
        Tier::Gold => (BTreeSet::new(), BTreeSet::new()),
        Tier::Silver {
            relaxing,
            without_reviews,
        } => (relaxing.clone(), without_reviews.clone()),
    };

    Ok(FrozenParent {
        id: candidate.id.clone(),
        tier,
        decision_points: candidate.decision_points.len(),
        multi_path_points,
        relaxations,
        waived_reviews,
    })
}
