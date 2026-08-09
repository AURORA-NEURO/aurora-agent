//! Deduplication, similarity, and effective size.
//!
//! Blueprint 35.10, and the reason this crate exists. The section index lists "effective test size
//! after clustering and similarity" among the smaller quantities a million-instance benchmark must
//! always report, and `AGENTS.md` makes it non-negotiable: **instance count is not benchmark
//! count.**
//!
//! ## What "the same" means, stated three ways
//!
//! An effective-size number is meaningless without the relation that produced it, so
//! [`SimilarityRelation`] is a required argument everywhere and travels inside every result. Each
//! variant documents both what it merges and what it refuses to merge — the second half matters
//! more, because that is where an effective-size number is quietly inflated.
//!
//! The [`SimilarityRelation::EquivalenceClass`] definition is `bioprism_mutation::diversity`'s:
//! distinct `(parent, mutation family, oracle signature)` triples. It is restated rather than
//! imported because this crate does not depend on `bioprism-mutation`, and a test asserts the
//! property that makes it work — a renamed world lands in the same class.
//!
//! ## What is deliberately absent
//!
//! 35.10's required components include "embedding and graph similarity" and "behavioral response
//! similarity". Neither is here. An approximate similarity has a false merge rate, false merge rate
//! is one of 35.10's own metrics, and there is no ground truth in this crate to measure it against.
//! [`RelationQuality`] exists so that a caller who *does* have labelled duplicates can measure any
//! relation's duplicate recall and false merge rate before trusting its effective-size number. An
//! unmeasured similarity relation would be a licence to report whatever effective size flatters the
//! release.

use crate::corpus::Corpus;
use crate::error::ScaleError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a merge means. Three exact relations, in increasing conservatism.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimilarityRelation {
    /// Merges items whose `facts`, `factors` and `events` hash identically.
    ///
    /// Does **not** merge items that differ only in a fact's value, however trivially — this is an
    /// exact relation and near-duplicates survive it. It *does* merge items that differ only in
    /// `world_id` or `description`, which is the whole point: renaming must not manufacture
    /// instances.
    ContentDigest,
    /// Merges items sharing a `(root parent, mutation family, oracle signature)` triple.
    ///
    /// Does **not** merge two different mutation families applied to one parent, even when they
    /// produce similar text, because they probe different failure modes. Does not merge two
    /// parents, however alike their subject matter.
    EquivalenceClass,
    /// Merges everything descended from one parent world.
    ///
    /// The floor. 35's scale constraint — "parent worlds are the unit of scientific breadth" —
    /// read literally. Does not distinguish a family of one from a family of a hundred thousand,
    /// which is why it is reported alongside the other two rather than instead of them.
    ParentWorld,
}

impl SimilarityRelation {
    pub const ALL: [SimilarityRelation; 3] = [
        SimilarityRelation::ContentDigest,
        SimilarityRelation::EquivalenceClass,
        SimilarityRelation::ParentWorld,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            SimilarityRelation::ContentDigest => "content_digest",
            SimilarityRelation::EquivalenceClass => "equivalence_class",
            SimilarityRelation::ParentWorld => "parent_world",
        }
    }

    /// The sentence a methods section must contain if it quotes this relation's effective size.
    pub fn treats_as_same(&self) -> &'static str {
        match self {
            SimilarityRelation::ContentDigest => {
                "items whose facts, factors and events are byte-identical after canonicalization, \
                 regardless of world id or description"
            }
            SimilarityRelation::EquivalenceClass => {
                "items sharing a root parent world, a mutation family and an oracle signature"
            }
            SimilarityRelation::ParentWorld => "all items descended from one parent world",
        }
    }

    /// What it refuses to merge, which is where an inflated number would hide.
    pub fn does_not_merge(&self) -> &'static str {
        match self {
            SimilarityRelation::ContentDigest => {
                "near-duplicates: any difference in a fact value, however trivial, survives as a \
                 separate item"
            }
            SimilarityRelation::EquivalenceClass => {
                "distinct mutation families over one parent, however similar their surface form"
            }
            SimilarityRelation::ParentWorld => {
                "nothing within a family; difficulty and mechanism are invisible to it"
            }
        }
    }

    fn key_for(&self, corpus: &Corpus, id: &str) -> Result<String, ScaleError> {
        let item = corpus
            .get(id)
            .ok_or_else(|| ScaleError::UnknownItem(id.to_string()))?;
        Ok(match self {
            SimilarityRelation::ContentDigest => item.content_digest.clone(),
            SimilarityRelation::EquivalenceClass => {
                let root = corpus.root_of(id)?;
                format!("{}|{}|{}", root.id, item.family, item.signature)
            }
            SimilarityRelation::ParentWorld => corpus.root_of(id)?.id.clone(),
        })
    }
}

/// The nominal count and the effective count, in one object that cannot be split.
///
/// There is no constructor that takes a nominal count alone, and [`crate::corpus::NominalCount`]
/// does not serialize. Every field below is derived in [`EffectiveSize::measure`] from the same
/// corpus in the same pass, so `nominal` and `effective` cannot describe different populations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveSize {
    pub relation: SimilarityRelation,
    /// Instances. The number a naive report would headline.
    pub nominal: usize,
    /// Independent equivalence classes under `relation`. The honest denominator.
    pub effective: usize,
    /// `nominal / effective`. 1.0 means every instance is independent; 134.0 means the headline
    /// number is 134 times larger than the information it carries.
    pub inflation_ratio: f64,
    /// Instances removed by merging. `nominal - effective`.
    pub duplicates_collapsed: usize,
    /// Size of the biggest class. A single class holding most of the corpus is the usual shape of
    /// an inflated benchmark.
    pub largest_class: usize,
    /// Share of instances descending from the single most productive parent world. 35.10 lists
    /// parent concentration as an operational metric; above roughly 0.5 the benchmark is one world
    /// wearing a costume.
    pub parent_concentration: f64,
    /// What the relation merges and what it refuses to.
    pub relation_note: String,
}

impl EffectiveSize {
    /// Counts equivalence classes over `corpus` under `relation`.
    pub fn measure(corpus: &Corpus, relation: SimilarityRelation) -> Result<Self, ScaleError> {
        let mut class_sizes: BTreeMap<String, usize> = BTreeMap::new();
        let mut family_sizes: BTreeMap<String, usize> = BTreeMap::new();
        for item in corpus.iter() {
            let key = relation.key_for(corpus, &item.id)?;
            *class_sizes.entry(key).or_default() += 1;
            *family_sizes.entry(corpus.family_of(&item.id)?).or_default() += 1;
        }

        let nominal = corpus.nominal().get();
        let effective = class_sizes.len();
        Ok(EffectiveSize {
            relation,
            nominal,
            effective,
            inflation_ratio: ratio(nominal, effective),
            duplicates_collapsed: nominal.saturating_sub(effective),
            largest_class: class_sizes.values().copied().max().unwrap_or(0),
            parent_concentration: if nominal == 0 {
                0.0
            } else {
                family_sizes.values().copied().max().unwrap_or(0) as f64 / nominal as f64
            },
            relation_note: format!(
                "merges {}; does not merge {}",
                relation.treats_as_same(),
                relation.does_not_merge()
            ),
        })
    }

    /// Builds the figure from counts rather than materialized items.
    ///
    /// 35.16 designs for 1.2 million descendants and nobody should allocate them to report on
    /// them. The counts must come from an enumeration the caller can defend; this constructor does
    /// no merging of its own and cannot detect that the class count was overstated.
    pub fn from_counts(relation: SimilarityRelation, nominal: usize, effective: usize) -> Self {
        EffectiveSize {
            relation,
            nominal,
            effective,
            inflation_ratio: ratio(nominal, effective),
            duplicates_collapsed: nominal.saturating_sub(effective),
            largest_class: 0,
            parent_concentration: 0.0,
            relation_note: format!(
                "analytic: merges {}; class count supplied by the caller, not measured",
                relation.treats_as_same()
            ),
        }
    }

    /// The sentence a report leads with.
    pub fn headline(&self) -> String {
        format!(
            "{} nominal instances collapse to {} independent classes under the {} relation \
             (inflation ×{:.2}, parent concentration {:.2}). Instance count is not benchmark count.",
            self.nominal,
            self.effective,
            self.relation.as_str(),
            self.inflation_ratio,
            self.parent_concentration
        )
    }

    /// Whether the corpus carries enough independent information to be called a benchmark.
    ///
    /// Deliberately conservative and deliberately crude: fewer than three classes is a robustness
    /// check and should be labelled one. It is a floor, not a target.
    pub fn is_publishable_as_a_benchmark(&self) -> bool {
        self.effective >= 3
    }
}

/// All three relations measured over one corpus, so no single flattering number stands alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EffectiveSizeReport {
    pub by_relation: Vec<EffectiveSize>,
    /// Cluster stability: the fraction of item pairs that content-digest and equivalence-class
    /// agree about, together or apart. 35.10 lists cluster stability as a metric without defining
    /// it; this is the Rand index between the two exact relations. `None` when the corpus is too
    /// large to enumerate pairs, which this deliberately refuses to approximate.
    pub cluster_stability: Option<f64>,
}

/// Pair enumeration is quadratic, so stability is computed only below this size and reported as
/// `None` above it rather than sampled. A sampled stability with no stated sampling error is worse
/// than no stability.
const STABILITY_PAIR_LIMIT: usize = 2_000;

impl EffectiveSizeReport {
    pub fn measure(corpus: &Corpus) -> Result<Self, ScaleError> {
        let mut by_relation = Vec::new();
        for relation in SimilarityRelation::ALL {
            by_relation.push(EffectiveSize::measure(corpus, relation)?);
        }
        let cluster_stability = if corpus.len() <= STABILITY_PAIR_LIMIT {
            Some(cluster_stability(
                corpus,
                SimilarityRelation::ContentDigest,
                SimilarityRelation::EquivalenceClass,
            )?)
        } else {
            None
        };
        Ok(EffectiveSizeReport {
            by_relation,
            cluster_stability,
        })
    }

    pub fn under(&self, relation: SimilarityRelation) -> Option<&EffectiveSize> {
        self.by_relation.iter().find(|size| size.relation == relation)
    }

    /// The smallest effective count across the relations — the number a claim should rest on.
    pub fn most_conservative(&self) -> Option<&EffectiveSize> {
        self.by_relation.iter().min_by_key(|size| size.effective)
    }

    pub fn headline(&self) -> String {
        match self.most_conservative() {
            Some(size) => size.headline(),
            None => "empty corpus: 0 nominal instances, 0 effective classes".into(),
        }
    }
}

/// Rand index between two relations: the fraction of item pairs they classify identically.
///
/// Quadratic in corpus size, so callers above a few thousand items should partition first.
pub fn cluster_stability(
    corpus: &Corpus,
    left: SimilarityRelation,
    right: SimilarityRelation,
) -> Result<f64, ScaleError> {
    let ids: Vec<&str> = corpus.iter().map(|item| item.id.as_str()).collect();
    let left_keys = ids
        .iter()
        .map(|id| left.key_for(corpus, id))
        .collect::<Result<Vec<_>, _>>()?;
    let right_keys = ids
        .iter()
        .map(|id| right.key_for(corpus, id))
        .collect::<Result<Vec<_>, _>>()?;

    let mut agreements = 0usize;
    let mut pairs = 0usize;
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            pairs += 1;
            if (left_keys[i] == left_keys[j]) == (right_keys[i] == right_keys[j]) {
                agreements += 1;
            }
        }
    }
    Ok(if pairs == 0 {
        1.0
    } else {
        agreements as f64 / pairs as f64
    })
}

/// How well a relation recovers a known duplicate structure.
///
/// 35.10 lists duplicate recall and false merge rate as operational metrics. Both need labelled
/// truth, which is exactly why no approximate relation ships in this crate unmeasured: a relation
/// with a 0.3 false merge rate produces an effective size that is too small, and one with 0.3
/// recall produces one that is too large, and neither announces itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelationQuality {
    pub relation: SimilarityRelation,
    /// True duplicate pairs the relation merged, over all true duplicate pairs.
    pub duplicate_recall: f64,
    /// Merged pairs that were not true duplicates, over all merged pairs.
    pub false_merge_rate: f64,
    pub true_duplicate_pairs: usize,
    pub merged_pairs: usize,
}

impl RelationQuality {
    /// `truth` maps item id to the id of the true class it belongs to.
    pub fn evaluate(
        corpus: &Corpus,
        relation: SimilarityRelation,
        truth: &BTreeMap<String, String>,
    ) -> Result<Self, ScaleError> {
        let ids: Vec<&str> = corpus.iter().map(|item| item.id.as_str()).collect();
        let keys = ids
            .iter()
            .map(|id| relation.key_for(corpus, id))
            .collect::<Result<Vec<_>, _>>()?;

        let mut true_pairs = 0usize;
        let mut merged_pairs = 0usize;
        let mut recalled = 0usize;
        let mut false_merges = 0usize;
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                let same_truth = match (truth.get(ids[i]), truth.get(ids[j])) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                };
                let same_key = keys[i] == keys[j];
                if same_truth {
                    true_pairs += 1;
                }
                if same_key {
                    merged_pairs += 1;
                }
                match (same_truth, same_key) {
                    (true, true) => recalled += 1,
                    (false, true) => false_merges += 1,
                    _ => {}
                }
            }
        }

        Ok(RelationQuality {
            relation,
            duplicate_recall: if true_pairs == 0 {
                1.0
            } else {
                recalled as f64 / true_pairs as f64
            },
            false_merge_rate: if merged_pairs == 0 {
                0.0
            } else {
                false_merges as f64 / merged_pairs as f64
            },
            true_duplicate_pairs: true_pairs,
            merged_pairs,
        })
    }
}

fn ratio(nominal: usize, effective: usize) -> f64 {
    if effective == 0 {
        0.0
    } else {
        nominal as f64 / effective as f64
    }
}

/// Hierarchical effective sample size under intra-parent correlation.
///
/// 35.16 requires "effective test size estimated hierarchically" and does not say how, which is the
/// single largest under-specification in section 35. This uses the standard clustered-design
/// deflation, `n / (1 + (m - 1) * rho)`, with `m` the mean instances per parent family: instances
/// from one parent world are not independent observations of a system's capability, because they
/// share facts, subjects and failure modes.
///
/// `rho` is an **assumption the caller states**, not a constant this crate knows. It is never
/// defaulted. Callers should report a range — [`crate::accounting`] does — because the answer moves
/// by an order of magnitude across plausible values, and quoting a single one hides that.
pub fn hierarchical_effective_size(
    instances: usize,
    families: usize,
    rho: f64,
) -> Result<f64, ScaleError> {
    if !(0.0..1.0).contains(&rho) {
        return Err(ScaleError::CorrelationOutOfRange(rho));
    }
    if instances == 0 || families == 0 {
        return Ok(0.0);
    }
    let mean_cluster = instances as f64 / families as f64;
    let design_effect = 1.0 + (mean_cluster - 1.0) * rho;
    Ok(instances as f64 / design_effect)
}
