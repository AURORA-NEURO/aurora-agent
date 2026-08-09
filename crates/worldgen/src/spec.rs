//! The structural knobs.
//!
//! Blueprint 43.39: "Generate controlled worlds that vary mathematical structure independently, so
//! the platform can test exactly when each representation and backend succeeds or fails."
//!
//! Each field below is a dimension along which a context strategy can be made to succeed or fail
//! *independently of the others*. The reference world sits at one corner of this space — hub
//! attachment, no relays, distinct tags — which is precisely why it fails to discriminate FIBER
//! from a tuned graph walk or a lexical retriever. See `docs/FINDINGS.md`.

use serde::{Deserialize, Serialize};

/// Where distractor factors attach to the decisive dependency chain.
///
/// This is the knob that decides whether a *separating depth* exists for neighbourhood traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistractorAttachment {
    /// Attach to `cohort_id`, a decisive leaf. Distractors then sit two hops *beyond* the decisive
    /// facts, so some depth reaches the decisive set without them. The reference world's shape.
    Hub,
    /// Attach near the target, above the relay chain. Distractors are then at most as far from the
    /// target as the decisive facts, so no depth admits the decisive set while excluding them.
    NearTarget,
}

/// How distractor facts are tagged and named.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagStyle {
    /// Distractors are tagged `exploratory`. A lexical retriever scoring the query's protected
    /// tags separates them trivially. The reference world's shape.
    Distinct,
    /// Distractors carry tags that *tokenise* to the protected vocabulary — `identity_summary`,
    /// `split_summary` — without being protected tags themselves.
    ///
    /// This defeats lexical scoring without touching protected-closure semantics: closure matches
    /// whole tags, so `identity_summary` is correctly not protected, while BM25 tokenises it to
    /// `identity` + `summary` and scores it against the query.
    Camouflaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageMechanism {
    Identity,
    Site,
    Temporal,
    Preprocessing,
}

impl LeakageMechanism {
    pub const ALL: [LeakageMechanism; 4] = [
        LeakageMechanism::Identity,
        LeakageMechanism::Site,
        LeakageMechanism::Temporal,
        LeakageMechanism::Preprocessing,
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSpec {
    pub world_id: String,
    /// Number of subjects in the cohort.
    pub subjects: usize,
    /// Number of irrelevant exploratory facts.
    pub distractors: usize,
    /// Relay factors inserted between each check factor and the target, pushing decisive facts
    /// further from the target without changing what they mean.
    pub relay_depth: usize,
    pub attachment: DistractorAttachment,
    pub tag_style: TagStyle,
    /// Which leakage defects to inject. An empty list produces a clean, valid split.
    pub leakage: Vec<LeakageMechanism>,
    pub seed: u64,
}

impl WorldSpec {
    /// The reference world's structural corner: hub attachment, no relays, distinct tags.
    pub fn reference_like(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-reference-like-v1".into(),
            subjects: 4,
            distractors,
            relay_depth: 0,
            attachment: DistractorAttachment::Hub,
            tag_style: TagStyle::Distinct,
            leakage: LeakageMechanism::ALL.to_vec(),
            seed: 20_260_808,
        }
    }

    /// A world built to discriminate.
    ///
    /// Distractors attach near the target and decisive facts sit behind a relay chain, so every
    /// depth that admits the decisive set also admits every distractor; and camouflaged tags deny
    /// a lexical retriever the shortcut of scoring the protected vocabulary.
    pub fn discriminating(distractors: usize) -> Self {
        WorldSpec {
            world_id: "generated-discriminating-v1".into(),
            subjects: 4,
            distractors,
            relay_depth: 3,
            attachment: DistractorAttachment::NearTarget,
            tag_style: TagStyle::Camouflaged,
            leakage: LeakageMechanism::ALL.to_vec(),
            seed: 20_260_808,
        }
    }

    pub fn with_world_id(mut self, id: impl Into<String>) -> Self {
        self.world_id = id.into();
        self
    }

    pub fn with_leakage(mut self, leakage: Vec<LeakageMechanism>) -> Self {
        self.leakage = leakage;
        self
    }

    pub fn has(&self, mechanism: LeakageMechanism) -> bool {
        self.leakage.contains(&mechanism)
    }
}
