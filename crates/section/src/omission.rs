//! The omission manifest.
//!
//! This is the load-bearing idea of blueprint 43.26. A compact context is dangerous when nobody
//! can tell what was excluded, so omitted evidence is grouped by *structural reason* and each
//! group is assigned an influence class.
//!
//! The distinction the specification refuses to let slide: [`InfluenceClass::Zero`] means the
//! omission provably cannot change the decision, while [`InfluenceClass::Unknown`] means nobody
//! checked. Only [`InfluenceClass::Zero`] and [`InfluenceClass::Bounded`] support a sufficiency
//! claim; a manifest containing any `Unknown` group must not be labelled sufficient.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceClass {
    /// No dependency path reaches the target; excluding it cannot move the decision.
    Zero,
    /// Influence is non-zero but bounded by a stated quantity.
    Bounded,
    /// Policy or consent forbids access. The decision must account for the gap, not ignore it.
    InaccessibleByPolicy,
    /// Not available at the temporal cut; may become available later.
    DeferredAcquisition,
    /// Not analysed. Never counts toward sufficiency.
    Unknown,
}

impl InfluenceClass {
    /// Whether a group in this class may participate in a sufficiency claim.
    pub fn supports_sufficiency(self) -> bool {
        matches!(self, InfluenceClass::Zero | InfluenceClass::Bounded)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            InfluenceClass::Zero => "zero",
            InfluenceClass::Bounded => "bounded",
            InfluenceClass::InaccessibleByPolicy => "inaccessible_by_policy",
            InfluenceClass::DeferredAcquisition => "deferred_acquisition",
            InfluenceClass::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OmissionGroup {
    /// Structural reason this family was excluded.
    pub reason: String,
    pub influence: InfluenceClass,
    pub count: usize,
    /// A bound on the decision distortion this group can cause, when `influence` is
    /// [`InfluenceClass::Bounded`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bound: Option<f64>,
    /// Representative members, for a human reading the receipt. Never the whole list: large
    /// manifests are content-addressed rather than inlined.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OmissionManifest {
    pub groups: Vec<OmissionGroup>,
}

impl OmissionManifest {
    pub fn push(&mut self, group: OmissionGroup) {
        self.groups.push(group);
    }

    pub fn total_omitted(&self) -> usize {
        self.groups.iter().map(|g| g.count).sum()
    }

    pub fn count_in(&self, class: InfluenceClass) -> usize {
        self.groups
            .iter()
            .filter(|g| g.influence == class)
            .map(|g| g.count)
            .sum()
    }

    /// True only when every group is provably zero-influence or explicitly bounded.
    ///
    /// A manifest with any unknown, policy-blocked or deferred group is *not* sufficient, and
    /// the compiler must abstain or refine rather than present the context as complete.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.groups.iter().all(|g| g.influence.supports_sufficiency())
    }

    pub fn blocking_groups(&self) -> impl Iterator<Item = &OmissionGroup> {
        self.groups
            .iter()
            .filter(|g| !g.influence.supports_sufficiency())
    }
}
