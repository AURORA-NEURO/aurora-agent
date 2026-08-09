//! Cheap structural features of a task, computed before anything is compiled.
//!
//! Blueprint 09.08 lists the task fingerprint as the router's only view of a new task. The
//! constraint that gives the fingerprint its meaning is stated here rather than in the blueprint,
//! because the blueprint does not state it: **nothing in this module may consult the compiler or
//! the oracle.** Routing that must compile in order to decide whether to compile has saved
//! nothing, and routing that reads the oracle verdict is not routing at all — it is the
//! retrospective selector of Gate 6, which is a *bound* on routing rather than an instance of it.
//!
//! Concretely, every field below is a single linear pass over `world.facts`, `world.factors` and
//! `query.protected_tags`. There is no closure computation, no backward slice, no temporal cut,
//! no BFS from the targets, and no evaluation. The strongest test of that discipline is
//! `the_fingerprint_cannot_see_the_answer`: two worlds that differ only in whether the split
//! actually leaks — that is, whose oracle verdicts differ — produce byte-identical fingerprints.
//!
//! ## What the features are for
//!
//! Each feature exists because some context architecture succeeds or fails on it, and
//! `crates/worldgen` can vary it independently (blueprint 43.39):
//!
//! - **distractor density** — the share of facts outside the protected vocabulary. These are the
//!   facts a relevance step has to remove, because the protected closure will not.
//! - **tag informativeness** — whether the protected vocabulary lexically separates the decisive
//!   facts from the rest. Near 1 a BM25 retriever is reading something close to an answer key;
//!   near 0 the distractors are camouflaged and lexical scoring is actively misleading. Token
//!   overlap is a *proxy* for confusability and has false positives: a fact tagged
//!   `future_evidence` is counted as confusable with the protected tag `negative_evidence`
//!   because both tokenise to `evidence`, even though nothing about it is protected.
//! - **factor arity distribution** — mean, maximum and full histogram of factor input counts, the
//!   width proxy that blueprint 43.18 reports in the certificate's structural block.
//! - **protected-tag count** — the size of the query's protected vocabulary, which bounds how
//!   much work the mandatory closure of 43.13 can do before any relevance step runs.
//! - **unary chain depth** — how far derived evidence sits behind relay factors. This is what
//!   moves the separating depth of a k-hop walk, and unlike a raw ratio it does not wash out as
//!   distractors are added.
//! - **hub share / hub is derived** — how concentrated consumption is on one variable, and
//!   whether that variable is fact-supplied (a leaf, far from the targets) or factor-produced
//!   (near the targets). This is the attachment knob that decides whether *any* traversal depth
//!   admits the decisive set while excluding the distractors.

use bioprism_fiber::Query;
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Iteration bound for the unary-chain relaxation.
///
/// A world may contain a cycle of unary factors. Rather than reject such worlds or recurse into
/// them, the relaxation is bounded: a cyclic chain reports this depth and nothing hangs.
const CHAIN_RELAXATION_ROUNDS: usize = 16;

/// Chain depths at or beyond this are treated as equivalent by the routing distance.
///
/// Beyond four relays the routing-relevant fact — decisive evidence is not adjacent to the
/// targets — is already established, and further depth carries no additional decision content.
const CHAIN_AXIS_CAP: f64 = 4.0;

/// Arity used to normalise the mean-arity axis into `[0, 1]`.
const ARITY_AXIS_CAP: f64 = 4.0;

/// Protected vocabulary size used to normalise the protected-tag axis into `[0, 1]`.
const PROTECTED_TAG_AXIS_CAP: f64 = 16.0;

/// `log10` of the fact count used to normalise the scale axis into `[0, 1]`.
const SCALE_AXIS_DECADES: f64 = 4.0;

/// Weights for the routing distance, in the order returned by [`Fingerprint::axes`].
///
/// The three categorical axes — tag informativeness, chain depth, hub kind — carry triple weight.
/// This is a **stated prior, not a fitted parameter**: those three are the axes along which
/// `crates/worldgen` changes which architecture is admissible at all, whereas the remaining axes
/// mostly change how expensive an admissible architecture is. The weights were fixed before any
/// routing outcome was measured, and the consequence is deliberate: with the default radius, two
/// tasks that disagree on a single categorical axis are never neighbours.
const AXIS_WEIGHTS: [f64; 9] = [3.0, 3.0, 3.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fingerprint {
    pub facts: usize,
    pub factors: usize,
    /// Size of the query's protected vocabulary.
    pub protected_tag_count: usize,
    /// Share of facts carrying at least one protected tag.
    pub protected_fact_fraction: f64,
    /// Share of facts carrying no protected tag.
    pub distractor_density: f64,
    /// Share of non-protected facts whose tags do *not* tokenise into the protected vocabulary.
    ///
    /// 1.0 means the protected tags separate cleanly; 0.0 means every distractor is camouflaged.
    pub tag_informativeness: f64,
    pub mean_factor_arity: f64,
    pub max_factor_arity: usize,
    /// Full input-count distribution: arity to number of factors.
    pub arity_histogram: BTreeMap<usize, usize>,
    /// Longest chain of one-input, one-output factors.
    pub max_unary_chain: usize,
    /// Consumers of the most-consumed variable, as a share of all factors.
    pub hub_share: f64,
    /// Whether the most-consumed variable is produced by a factor rather than supplied by a fact.
    pub hub_is_derived: bool,
    /// Number of factors producing a query target.
    pub target_producer_count: usize,
}

impl Fingerprint {
    /// One pass over the world and the query. No compile, no oracle, no traversal from targets.
    pub fn of(world: &World, query: &Query) -> Self {
        let facts = world.facts.len();
        let factors = world.factors.len();

        let protected_facts = world
            .facts
            .iter()
            .filter(|fact| fact.has_any_tag(&query.protected_tags))
            .count();
        let non_protected_facts = facts - protected_facts;

        let protected_tokens: BTreeSet<String> = query
            .protected_tags
            .iter()
            .flat_map(|tag| tokenize(tag))
            .collect();
        let camouflaged = world
            .facts
            .iter()
            .filter(|fact| !fact.has_any_tag(&query.protected_tags))
            .filter(|fact| {
                fact.tags
                    .iter()
                    .flat_map(|tag| tokenize(tag))
                    .any(|token| protected_tokens.contains(&token))
            })
            .count();

        let mut arity_histogram: BTreeMap<usize, usize> = BTreeMap::new();
        let mut arity_total = 0usize;
        let mut max_factor_arity = 0usize;
        for factor in &world.factors {
            let arity = factor.arity();
            *arity_histogram.entry(arity).or_default() += 1;
            arity_total += arity;
            max_factor_arity = max_factor_arity.max(arity);
        }

        let (hub_consumers, hub_is_derived) = hub(world);
        let targets: BTreeSet<&str> = query.targets.iter().map(|t| t.as_str()).collect();
        let target_producer_count = world
            .factors
            .iter()
            .filter(|factor| factor.outputs.iter().any(|o| targets.contains(o.as_str())))
            .count();

        Fingerprint {
            facts,
            factors,
            protected_tag_count: query.protected_tags.len(),
            protected_fact_fraction: ratio(protected_facts, facts),
            distractor_density: ratio(non_protected_facts, facts),
            tag_informativeness: if non_protected_facts == 0 {
                1.0
            } else {
                1.0 - ratio(camouflaged, non_protected_facts)
            },
            mean_factor_arity: ratio(arity_total, factors),
            max_factor_arity,
            arity_histogram,
            max_unary_chain: max_unary_chain(world),
            hub_share: ratio(hub_consumers, factors),
            hub_is_derived,
            target_producer_count,
        }
    }

    /// The normalised routing axes, each in `[0, 1]`, in the order [`AXIS_WEIGHTS`] scores them.
    ///
    /// The reported fields are richer than the axes on purpose. A report should show the arity
    /// histogram; a distance function should not, because a histogram has no natural metric and
    /// pretending otherwise would smuggle an unstated modelling assumption into the router.
    pub fn axes(&self) -> [f64; 9] {
        [
            self.tag_informativeness.clamp(0.0, 1.0),
            (self.max_unary_chain as f64 / CHAIN_AXIS_CAP).clamp(0.0, 1.0),
            if self.hub_is_derived { 1.0 } else { 0.0 },
            self.distractor_density.clamp(0.0, 1.0),
            self.protected_fact_fraction.clamp(0.0, 1.0),
            self.hub_share.clamp(0.0, 1.0),
            (self.mean_factor_arity / ARITY_AXIS_CAP).clamp(0.0, 1.0),
            (self.protected_tag_count as f64 / PROTECTED_TAG_AXIS_CAP).clamp(0.0, 1.0),
            ((self.facts as f64 + 1.0).log10() / SCALE_AXIS_DECADES).clamp(0.0, 1.0),
        ]
    }

    /// Weighted mean absolute axis difference, in `[0, 1]`.
    ///
    /// Symmetric, zero on identical fingerprints, and bounded — so a neighbourhood radius is a
    /// number a reader can reason about rather than an opaque threshold.
    pub fn distance(&self, other: &Fingerprint) -> f64 {
        let left = self.axes();
        let right = other.axes();
        let total: f64 = AXIS_WEIGHTS.iter().sum();
        let weighted: f64 = left
            .iter()
            .zip(right.iter())
            .zip(AXIS_WEIGHTS.iter())
            .map(|((a, b), weight)| weight * (a - b).abs())
            .sum();
        weighted / total
    }

    /// A human-readable structural label used in reports only.
    ///
    /// Routing uses [`Fingerprint::distance`] over the continuous axes. The regime is a reading
    /// aid; deciding from the coarse label instead would discard information the router has.
    pub fn regime(&self) -> Regime {
        Regime {
            tags: if self.tag_informativeness >= 0.9 {
                TagRegime::Separable
            } else if self.tag_informativeness <= 0.1 {
                TagRegime::Camouflaged
            } else {
                TagRegime::Mixed
            },
            chain: if self.max_unary_chain >= 2 {
                ChainRegime::Relayed
            } else {
                ChainRegime::Shallow
            },
            attachment: if self.hub_is_derived {
                AttachmentRegime::Derived
            } else {
                AttachmentRegime::Leaf
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagRegime {
    /// Protected tags separate decisive facts lexically; a retriever reads close to an answer key.
    Separable,
    Mixed,
    /// Distractor tags tokenise into the protected vocabulary; lexical scoring is misled.
    Camouflaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChainRegime {
    Shallow,
    Relayed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentRegime {
    /// The busiest variable is fact-supplied, so distractors hang off a leaf far from the targets.
    Leaf,
    /// The busiest variable is factor-produced, so distractors sit as close to the targets as the
    /// decisive facts do.
    Derived,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Regime {
    pub tags: TagRegime,
    pub chain: ChainRegime,
    pub attachment: AttachmentRegime,
}

impl fmt::Display for Regime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tags = match self.tags {
            TagRegime::Separable => "separable-tags",
            TagRegime::Mixed => "mixed-tags",
            TagRegime::Camouflaged => "camouflaged-tags",
        };
        let chain = match self.chain {
            ChainRegime::Shallow => "shallow",
            ChainRegime::Relayed => "relayed",
        };
        let attachment = match self.attachment {
            AttachmentRegime::Leaf => "leaf-hub",
            AttachmentRegime::Derived => "derived-hub",
        };
        write!(f, "{tags}/{chain}/{attachment}")
    }
}

fn ratio(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_ascii_lowercase())
        .collect()
}

/// Consumers of the busiest variable, and whether that variable is factor-produced.
///
/// Ties are broken by variable name so the feature is deterministic across runs and platforms.
fn hub(world: &World) -> (usize, bool) {
    let mut consumers: BTreeMap<&str, usize> = BTreeMap::new();
    for factor in &world.factors {
        for input in &factor.inputs {
            *consumers.entry(input.as_str()).or_default() += 1;
        }
    }

    let Some((variable, count)) = consumers
        .iter()
        .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
        .map(|(variable, count)| (*variable, *count))
    else {
        return (0, false);
    };

    let derived = world
        .factors
        .iter()
        .any(|factor| factor.outputs.iter().any(|o| o.as_str() == variable));
    (count, derived)
}

/// Longest path through one-input, one-output factors.
///
/// Bounded relaxation rather than a traversal: it terminates on cyclic worlds, it is linear in
/// the number of unary factors per round, and it is insensitive to how many unrelated distractor
/// factors the world contains — which a raw "share of factors that are unary" is not.
fn max_unary_chain(world: &World) -> usize {
    let unary: Vec<(&str, &str)> = world
        .factors
        .iter()
        .filter(|factor| factor.inputs.len() == 1 && factor.outputs.len() == 1)
        .map(|factor| (factor.inputs[0].as_str(), factor.outputs[0].as_str()))
        .collect();
    if unary.is_empty() {
        return 0;
    }

    let mut depth: BTreeMap<&str, usize> = BTreeMap::new();
    for _ in 0..CHAIN_RELAXATION_ROUNDS {
        let mut changed = false;
        for (input, output) in &unary {
            let candidate =
                (depth.get(input).copied().unwrap_or(0) + 1).min(CHAIN_RELAXATION_ROUNDS);
            if candidate > depth.get(output).copied().unwrap_or(0) {
                depth.insert(output, candidate);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    depth.values().copied().max().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{spec_task, world_and_query};
    use bioprism_worldgen::{DistractorAttachment, LeakageMechanism, TagStyle, WorldSpec};

    #[test]
    fn the_fingerprint_cannot_see_the_answer() {
        let leaking = WorldSpec::reference_like(40).with_leakage(LeakageMechanism::ALL.to_vec());
        let clean = WorldSpec::reference_like(40).with_leakage(vec![]);

        let (leaking_world, leaking_query) = world_and_query(&leaking);
        let (clean_world, clean_query) = world_and_query(&clean);

        assert_eq!(
            Fingerprint::of(&leaking_world, &leaking_query),
            Fingerprint::of(&clean_world, &clean_query),
            "worlds whose oracle verdicts differ must fingerprint identically, or the router is \
             reading the answer key"
        );
    }

    /// The separable case does not reach exactly 1.0, and that is the metric working.
    ///
    /// `fact.future_label` carries `future_evidence`, which tokenises to `future` + `evidence` and
    /// therefore shares a token with the protected tag `negative_evidence` without being
    /// protected. Token overlap is a proxy for lexical confusability and it has false positives;
    /// this test pins the size of the effect on the reference shape rather than pretending it is
    /// absent.
    #[test]
    fn camouflaged_distractor_tags_collapse_tag_informativeness() {
        let mut distinct = WorldSpec::reference_like(40);
        distinct.tag_style = TagStyle::Distinct;
        let mut camouflaged = WorldSpec::reference_like(40);
        camouflaged.tag_style = TagStyle::Camouflaged;

        let (distinct_world, query) = world_and_query(&distinct);
        let (camouflaged_world, _) = world_and_query(&camouflaged);

        let separable = Fingerprint::of(&distinct_world, &query).tag_informativeness;
        let misled = Fingerprint::of(&camouflaged_world, &query).tag_informativeness;

        assert!(
            (0.95..1.0).contains(&separable),
            "distinct tags separate cleanly apart from one incidental token overlap: {separable}"
        );
        assert!(misled < 0.01, "camouflaged tags do not separate: {misled}");
        assert_eq!(
            Fingerprint::of(&distinct_world, &query).regime().tags,
            TagRegime::Separable
        );
        assert_eq!(
            Fingerprint::of(&camouflaged_world, &query).regime().tags,
            TagRegime::Camouflaged
        );
    }

    #[test]
    fn relay_depth_stays_visible_at_every_distractor_scale() {
        for distractors in [8, 64, 400] {
            let mut shallow = WorldSpec::reference_like(distractors);
            shallow.relay_depth = 0;
            let mut relayed = WorldSpec::reference_like(distractors);
            relayed.relay_depth = 3;

            let (shallow_world, query) = world_and_query(&shallow);
            let (relayed_world, _) = world_and_query(&relayed);

            let shallow_chain = Fingerprint::of(&shallow_world, &query).max_unary_chain;
            let relayed_chain = Fingerprint::of(&relayed_world, &query).max_unary_chain;
            assert!(
                relayed_chain > shallow_chain + 1,
                "at {distractors} distractors the chain feature washed out: {shallow_chain} vs \
                 {relayed_chain}"
            );
        }
    }

    #[test]
    fn attachment_shows_up_as_whether_the_busiest_variable_is_derived() {
        let mut hub_attached = WorldSpec::reference_like(40);
        hub_attached.attachment = DistractorAttachment::Hub;
        let mut near_target = WorldSpec::reference_like(40);
        near_target.attachment = DistractorAttachment::NearTarget;

        let (hub_world, query) = world_and_query(&hub_attached);
        let (near_world, _) = world_and_query(&near_target);

        assert!(!Fingerprint::of(&hub_world, &query).hub_is_derived);
        assert!(Fingerprint::of(&near_world, &query).hub_is_derived);
    }

    #[test]
    fn distractor_density_and_protected_tag_count_track_the_task() {
        let (world, query) = world_and_query(&WorldSpec::reference_like(88));
        let fingerprint = Fingerprint::of(&world, &query);

        assert_eq!(fingerprint.protected_tag_count, query.protected_tags.len());
        assert_eq!(fingerprint.facts, world.facts.len());
        assert!(
            (fingerprint.distractor_density + fingerprint.protected_fact_fraction - 1.0).abs()
                < 1e-12,
            "every fact is either protected or a distractor"
        );
        assert!(fingerprint.distractor_density > 0.8);
    }

    #[test]
    fn the_arity_histogram_accounts_for_every_factor() {
        let (world, query) = world_and_query(&WorldSpec::discriminating(30));
        let fingerprint = Fingerprint::of(&world, &query);
        let counted: usize = fingerprint.arity_histogram.values().sum();
        assert_eq!(counted, world.factors.len());
        assert_eq!(
            fingerprint.max_factor_arity,
            world.factors.iter().map(|f| f.arity()).max().unwrap()
        );
    }

    #[test]
    fn distance_is_zero_to_itself_and_symmetric() {
        let (left_world, query) = world_and_query(&WorldSpec::reference_like(30));
        let (right_world, _) = world_and_query(&WorldSpec::discriminating(30));
        let left = Fingerprint::of(&left_world, &query);
        let right = Fingerprint::of(&right_world, &query);

        assert!(left.distance(&left).abs() < 1e-12);
        assert!((left.distance(&right) - right.distance(&left)).abs() < 1e-12);
        assert!(left.distance(&right) > 0.0);
        assert!(left.distance(&right) <= 1.0);
    }

    /// The stated prior of [`AXIS_WEIGHTS`], checked rather than asserted.
    #[test]
    fn a_single_categorical_flip_falls_outside_the_default_neighbourhood() {
        let mut base = WorldSpec::reference_like(64);
        base.tag_style = TagStyle::Distinct;
        let mut flipped = base.clone();
        flipped.tag_style = TagStyle::Camouflaged;

        let (base_world, query) = world_and_query(&base);
        let (flipped_world, _) = world_and_query(&flipped);
        let distance =
            Fingerprint::of(&base_world, &query).distance(&Fingerprint::of(&flipped_world, &query));

        assert!(
            distance > crate::policy::DEFAULT_NEIGHBOURHOOD_RADIUS,
            "camouflage must move a task out of its own neighbourhood, got {distance}"
        );
    }

    /// Scale alone must not fragment the neighbourhood, or the router abstains for a reason that
    /// has nothing to do with which architecture works.
    #[test]
    fn changing_only_the_distractor_count_keeps_tasks_in_one_neighbourhood() {
        let (small_world, query) = world_and_query(&WorldSpec::reference_like(48));
        let (large_world, _) = world_and_query(&WorldSpec::reference_like(96));
        let distance =
            Fingerprint::of(&small_world, &query).distance(&Fingerprint::of(&large_world, &query));
        assert!(
            distance <= crate::policy::DEFAULT_NEIGHBOURHOOD_RADIUS,
            "a doubling of distractors moved the task out of its neighbourhood: {distance}"
        );
    }

    #[test]
    fn a_fingerprint_round_trips_through_json() {
        let task = spec_task("rt", &WorldSpec::discriminating(12));
        let fingerprint = task.fingerprint();
        let text = serde_json::to_string(&fingerprint).unwrap();
        assert_eq!(
            serde_json::from_str::<Fingerprint>(&text).unwrap(),
            fingerprint
        );
    }
}
