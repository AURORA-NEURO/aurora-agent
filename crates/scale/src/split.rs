//! Contamination, training exposure and hidden-family splits.
//!
//! Blueprint 35.11. The leakage control that actually matters once a factory generates faster than
//! anyone can read: **items derived from one parent world must never straddle a train/test
//! boundary.**
//!
//! ## Why the check is on lineage, not similarity
//!
//! A mutation of a mutation of a parent can share nothing recognisable with its cousin — different
//! subjects, different distractors, a different leakage mechanism removed — and still leak, because
//! a system that has seen the parent's causal structure has seen the cousin's. Surface similarity
//! misses that by construction. [`crate::corpus::Corpus::root_of`] does not, because it walks the
//! derivation chain. This is the same reason 35.11 asks for a "public/validation/hidden **parent**
//! split" rather than an instance split.
//!
//! ## Two layers, because splits arrive from elsewhere
//!
//! [`FamilySplit`] assigns *families*, never items, so a straddle is unrepresentable: there is no
//! method that takes an item id and a tier. That is the AGENTS.md preference — a type that makes
//! the rule unbreakable beats a test that asserts it.
//!
//! But splits also arrive as flat item→tier tables from other tools, and those can straddle. So
//! [`verify_item_assignment`] exists as the checker for imported splits, and it returns
//! [`SplitError::FamilyStraddlesSplit`] naming the family and **both** sides, because the first
//! question anyone asks is which two items.
//!
//! ## Not implemented
//!
//! 35.11 also asks for time-based data release and model disclosure. Release epochs here are
//! logical integers, not dates — this crate reads no clock — and disclosure is a governance
//! artefact that belongs with `bioprism-governance`, not a data structure.

use crate::corpus::Corpus;
use crate::error::{ScaleError, SplitError};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Where a family may be seen from. 35.11's public/validation/hidden split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Published in full. Assume a system has trained on it.
    Public,
    /// Published for development. Assume repeated tuning against it.
    Validation,
    /// Never published. The only tier whose numbers survive contact with a leaderboard.
    Hidden,
}

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Tier::Public => "public",
            Tier::Validation => "validation",
            Tier::Hidden => "hidden",
        }
    }
}

/// A split defined over families, so no item can cross a boundary its family did not.
///
/// The absence of an `assign_item` method is the invariant. A caller physically cannot place two
/// descendants of one parent world on opposite sides.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FamilySplit {
    families: BTreeMap<String, Tier>,
}

impl FamilySplit {
    pub fn new() -> Self {
        FamilySplit::default()
    }

    /// Assigns a whole lineage family to a tier. Reassignment is refused rather than silently
    /// overwritten: a family that moved tier between two release candidates is the exact accident
    /// that leaks a hidden set.
    pub fn assign_family(
        &mut self,
        family: impl Into<String>,
        tier: Tier,
    ) -> Result<(), SplitError> {
        let family = family.into();
        if let Some(existing) = self.families.get(&family) {
            if *existing != tier {
                return Err(SplitError::FamilyAlreadyAssigned {
                    family,
                    existing: existing.as_str().to_string(),
                    requested: tier.as_str().to_string(),
                });
            }
            return Ok(());
        }
        self.families.insert(family, tier);
        Ok(())
    }

    pub fn tier_of_family(&self, family: &str) -> Option<Tier> {
        self.families.get(family).copied()
    }

    /// The tier an item inherits, resolved through its lineage root.
    pub fn tier_of_item(&self, corpus: &Corpus, id: &str) -> Result<Option<Tier>, ScaleError> {
        Ok(self.tier_of_family(&corpus.family_of(id)?))
    }

    /// Expands to the item→tier table a release actually ships, refusing to leave any item
    /// unassigned. An unassigned item is not "public by default"; it is a release blocker.
    pub fn resolve(&self, corpus: &Corpus) -> Result<BTreeMap<String, Tier>, SplitError> {
        let mut resolved = BTreeMap::new();
        for item in corpus.iter() {
            let family = corpus
                .family_of(&item.id)
                .map_err(|_| SplitError::UnassignedItem {
                    item: item.id.clone(),
                    family: "<unresolvable lineage>".into(),
                })?;
            let tier = self
                .families
                .get(&family)
                .copied()
                .ok_or_else(|| SplitError::UnassignedItem {
                    item: item.id.clone(),
                    family: family.clone(),
                })?;
            resolved.insert(item.id.clone(), tier);
        }
        Ok(resolved)
    }

    pub fn report(&self, corpus: &Corpus) -> Result<SplitReport, SplitError> {
        SplitReport::of(corpus, &self.resolve(corpus)?)
    }
}

/// Checks an externally produced item→tier table for family straddles.
///
/// This is the import path. [`FamilySplit`] cannot produce a straddling split; a spreadsheet can.
pub fn verify_item_assignment(
    corpus: &Corpus,
    assignment: &BTreeMap<String, Tier>,
) -> Result<SplitReport, SplitError> {
    let mut first_seen: BTreeMap<String, (String, Tier)> = BTreeMap::new();
    for item in corpus.iter() {
        let family = corpus
            .family_of(&item.id)
            .map_err(|_| SplitError::UnassignedItem {
                item: item.id.clone(),
                family: "<unresolvable lineage>".into(),
            })?;
        let tier = assignment
            .get(&item.id)
            .copied()
            .ok_or_else(|| SplitError::UnassignedItem {
                item: item.id.clone(),
                family: family.clone(),
            })?;
        match first_seen.get(&family) {
            Some((other_item, other_tier)) if *other_tier != tier => {
                return Err(SplitError::FamilyStraddlesSplit {
                    family,
                    left_item: other_item.clone(),
                    left: other_tier.as_str().to_string(),
                    right_item: item.id.clone(),
                    right: tier.as_str().to_string(),
                });
            }
            Some(_) => {}
            None => {
                first_seen.insert(family, (item.id.clone(), tier));
            }
        }
    }
    SplitReport::of(corpus, assignment)
}

/// What a split looks like once resolved, with the numbers 35.11 wants reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitReport {
    pub items_by_tier: BTreeMap<String, usize>,
    pub families_by_tier: BTreeMap<String, usize>,
    /// Families whose items all land in one tier. Equals total families in any valid split; it is
    /// reported anyway so a report from a straddling import is visibly wrong.
    pub intact_families: usize,
    pub total_families: usize,
}

impl SplitReport {
    fn of(corpus: &Corpus, assignment: &BTreeMap<String, Tier>) -> Result<Self, SplitError> {
        let mut items_by_tier: BTreeMap<String, usize> = BTreeMap::new();
        let mut tiers_per_family: BTreeMap<String, BTreeSet<Tier>> = BTreeMap::new();
        for item in corpus.iter() {
            let family = corpus
                .family_of(&item.id)
                .map_err(|_| SplitError::UnassignedItem {
                    item: item.id.clone(),
                    family: "<unresolvable lineage>".into(),
                })?;
            let tier = assignment
                .get(&item.id)
                .copied()
                .ok_or_else(|| SplitError::UnassignedItem {
                    item: item.id.clone(),
                    family: family.clone(),
                })?;
            *items_by_tier.entry(tier.as_str().to_string()).or_default() += 1;
            tiers_per_family.entry(family).or_default().insert(tier);
        }

        let mut families_by_tier: BTreeMap<String, usize> = BTreeMap::new();
        let mut intact_families = 0usize;
        for tiers in tiers_per_family.values() {
            if tiers.len() == 1 {
                intact_families += 1;
                if let Some(tier) = tiers.iter().next() {
                    *families_by_tier.entry(tier.as_str().to_string()).or_default() += 1;
                }
            }
        }

        Ok(SplitReport {
            items_by_tier,
            families_by_tier,
            intact_families,
            total_families: tiers_per_family.len(),
        })
    }

    /// The 35.11 headline metric: public score minus hidden score.
    ///
    /// A large positive gap means the public tier was memorized. It is a *difference*, not a
    /// correction — nothing here rescales a public number by the gap, because the gap is itself an
    /// estimate from a smaller sample.
    pub fn hidden_family_gap(public_score: f64, hidden_score: f64) -> f64 {
        public_score - hidden_score
    }
}

/// Fingerprint search and canaries. 35.11's contamination half.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Contamination {
    /// Declared training corpora, name → the set of content digests they contain.
    corpora: BTreeMap<String, BTreeSet<String>>,
    /// Planted items whose reproduction proves exposure of the hidden tier.
    canaries: BTreeMap<String, String>,
}

impl Contamination {
    pub fn new() -> Self {
        Contamination::default()
    }

    /// Declares that `corpus_name` contains these content digests.
    ///
    /// Digests, not text: a contamination check must not require handing the benchmark to the
    /// party being checked.
    pub fn declare_corpus(
        &mut self,
        corpus_name: impl Into<String>,
        digests: impl IntoIterator<Item = String>,
    ) {
        self.corpora
            .entry(corpus_name.into())
            .or_default()
            .extend(digests);
    }

    /// Plants a canary: an item whose digest should never appear in any model output.
    pub fn plant_canary(&mut self, canary_id: impl Into<String>, digest: impl Into<String>) {
        self.canaries.insert(canary_id.into(), digest.into());
    }

    /// Every item in `corpus` whose content digest appears in a declared training corpus.
    ///
    /// Returns findings rather than a boolean because 35's failure containment quarantines
    /// contaminated families and keeps existing result bundles with an invalidation notice. A
    /// caller that wants a release gate can treat a non-empty result as blocking.
    pub fn scan(&self, corpus: &Corpus) -> Vec<SplitError> {
        let mut findings = Vec::new();
        for item in corpus.iter() {
            for (name, digests) in &self.corpora {
                if digests.contains(&item.content_digest) {
                    findings.push(SplitError::TrainingExposure {
                        item: item.id.clone(),
                        digest: item.content_digest.clone(),
                        corpus: name.clone(),
                    });
                }
            }
        }
        findings
    }

    /// Canaries reproduced among `observed_digests`.
    pub fn detect_canaries(&self, observed_digests: &BTreeSet<String>) -> Vec<SplitError> {
        self.canaries
            .iter()
            .filter(|(_, digest)| observed_digests.contains(*digest))
            .map(|(canary, _)| SplitError::CanaryDetected {
                canary: canary.clone(),
            })
            .collect()
    }
}
