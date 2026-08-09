//! Replaying a bundle, with a verdict that has three states and never two.
//!
//! Blueprint 34.14 lists "independent reproductions" as a primary capability, counts "reproduction
//! success" among its product metrics, and requires the surface to support a `non-reproducible`
//! state explicitly rather than "replace them with zero, empty, or hidden values". Its MVP
//! acceptance test ends with a user verifying a bundle "without creating an account".
//!
//! # Why three states
//!
//! This is the rule the whole workspace runs on. "Nobody checked" and "checked and it failed" are
//! different facts, and a reproduction harness that reports them the same way has thrown away the
//! more useful one:
//!
//! - [`ReproductionVerdict::Reproduced`] — every entry that could be compared matched. It carries
//!   `not_compared` alongside `compared`, so it cannot claim more than it checked.
//! - [`ReproductionVerdict::Diverged`] — an entry differed, and the verdict names **which** entry,
//!   what was expected, what was observed, and which entries matched before it. A diverged verdict
//!   with no named entry would be an aggregate score, which is the thing this platform exists not to
//!   emit.
//! - [`ReproductionVerdict::NotAttempted`] — the replay did not happen, with the reason. A bundle
//!   that does not verify, a toolchain the policy refuses, or nothing comparable at all.
//!
//! There is deliberately no `is_ok()`, no `bool` conversion and no `Ord`. Any of them would let a
//! caller collapse the last two, and the first thing anyone writes on top of a boolean is a
//! percentage.
//!
//! # The bundle is verified before it is compared against
//!
//! [`ReproductionAttempt::replay`] runs [`crate::bundle::ResultBundle::verify`] first. Comparing a
//! fresh result against a manifest whose digests do not match its own contents would attribute the
//! bundle's internal inconsistency to the reproducer. That is [`NotAttemptedReason::BundleDidNotVerify`],
//! not a divergence.
//!
//! # Deliberately not implemented
//!
//! Nothing here executes anything. This module compares digests a caller supplies against digests a
//! bundle records; it does not run a compiler, fetch a world, or invoke `bioprism-fiber` — which it
//! deliberately does not depend on, so that a reproduction check links no engine. There is no
//! tolerance, no fuzzy match and no numerical epsilon: digests are equal or they are not, and a
//! "close enough" reproduction is a divergence. There is no timing, cost or resource comparison, and
//! no statistical reproduction (34.14's "scores and uncertainty" would need a replicate design this
//! crate has no way to represent).

use crate::bundle::ResultBundle;
use crate::environment::{ToolchainDifference, ToolchainFacts};
use bioprism_ids::ContentHash;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// How strictly a replay demands the host toolchain match the bundle's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainPolicy {
    /// Every declared field must be comparable and equal, or the replay is not attempted.
    ///
    /// A field one side declared and the other did not counts as a failure to establish a match, not
    /// as a match. Establishing equality requires two values.
    RequireDeclaredMatch,
    /// Proceed regardless, and carry the differences into the verdict.
    ///
    /// A reproduction on a different toolchain is still informative; it is just informative about
    /// something else, which is why the differences travel with the result.
    RecordDifferences,
}

/// A replay: the outputs a reproducer produced, and the toolchain it produced them on.
#[derive(Debug, Clone)]
pub struct ReproductionAttempt {
    replayed: BTreeMap<String, Value>,
    host_toolchain: ToolchainFacts,
    policy: ToolchainPolicy,
}

impl ReproductionAttempt {
    pub fn on_toolchain(host_toolchain: ToolchainFacts) -> Self {
        ReproductionAttempt {
            replayed: BTreeMap::new(),
            host_toolchain,
            policy: ToolchainPolicy::RecordDifferences,
        }
    }

    /// Records what the reproducer produced under a bundle entry's name.
    pub fn producing(mut self, name: impl Into<String>, content: Value) -> Self {
        self.replayed.insert(name.into(), content);
        self
    }

    pub fn under_policy(mut self, policy: ToolchainPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Compares this attempt against `bundle`, in manifest order.
    ///
    /// Manifest order is entry name order, which [`crate::manifest::BundleManifest::new`] fixes, so
    /// "the first divergence" is a property of the bundle and the attempt rather than of the order a
    /// caller happened to insert things in.
    pub fn replay(&self, bundle: &ResultBundle) -> ReproductionVerdict {
        let verified = match bundle.verify() {
            Ok(verified) => verified,
            Err(error) => {
                return ReproductionVerdict::NotAttempted {
                    reason: NotAttemptedReason::BundleDidNotVerify {
                        detail: error.to_string(),
                    },
                }
            }
        };

        let differences = bundle.manifest.toolchain.compare(&self.host_toolchain);
        if self.policy == ToolchainPolicy::RequireDeclaredMatch && !differences.is_empty() {
            return ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::ToolchainMismatch { differences },
            };
        }

        let mut compared = Vec::new();
        let mut not_compared = Vec::new();
        for entry in &bundle.manifest.entries {
            let Some(content) = self.replayed.get(&entry.name) else {
                not_compared.push(entry.name.clone());
                continue;
            };
            let observed = match ContentHash::of_value(content) {
                Ok(digest) => digest,
                Err(error) => {
                    return ReproductionVerdict::NotAttempted {
                        reason: NotAttemptedReason::ReplayedContentUnhashable {
                            entry: entry.name.clone(),
                            detail: error.to_string(),
                        },
                    }
                }
            };
            if observed != entry.digest {
                return ReproductionVerdict::Diverged {
                    first_divergence: Divergence {
                        entry: entry.name.clone(),
                        expected: entry.digest.clone(),
                        observed,
                        matched_before: compared,
                    },
                };
            }
            compared.push(entry.name.clone());
        }

        if compared.is_empty() {
            return ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::NothingComparable {
                    manifest_entries: bundle.manifest.entries.len(),
                },
            };
        }

        ReproductionVerdict::Reproduced(Reproduced {
            manifest_digest: verified.manifest_digest().clone(),
            compared,
            not_compared,
            toolchain_differences: differences,
        })
    }

    /// A replay that a caller declines to run, with the reason preserved.
    ///
    /// Exists so that "we chose not to try" has a representation, rather than being reported as a
    /// reproduction that produced nothing.
    pub fn refused(rationale: impl Into<String>) -> ReproductionVerdict {
        ReproductionVerdict::NotAttempted {
            reason: NotAttemptedReason::Refused {
                rationale: rationale.into(),
            },
        }
    }
}

/// The outcome of a replay. Three states, never collapsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum ReproductionVerdict {
    Reproduced(Reproduced),
    Diverged { first_divergence: Divergence },
    NotAttempted { reason: NotAttemptedReason },
}

impl ReproductionVerdict {
    /// True only for [`ReproductionVerdict::Reproduced`].
    ///
    /// Deliberately not paired with an `is_failure`, which would merge divergence with a replay that
    /// never ran. A caller that wants to distinguish them must match on the enum.
    pub fn is_reproduced(&self) -> bool {
        matches!(self, ReproductionVerdict::Reproduced(_))
    }

    /// The 34.14 status word for this verdict, in the vocabulary its failure-state list uses.
    pub fn status_word(&self) -> &'static str {
        match self {
            ReproductionVerdict::Reproduced(_) => "reproduced",
            ReproductionVerdict::Diverged { .. } => "non-reproducible",
            ReproductionVerdict::NotAttempted { .. } => "not-attempted",
        }
    }

    pub fn honest_label(&self) -> String {
        match self {
            ReproductionVerdict::Reproduced(reproduced) => reproduced.honest_label(),
            ReproductionVerdict::Diverged { first_divergence } => first_divergence.honest_label(),
            ReproductionVerdict::NotAttempted { reason } => {
                format!("reproduction not attempted: {}", reason.explanation())
            }
        }
    }
}

/// A replay in which everything comparable matched.
///
/// Private fields, minted only by [`ReproductionAttempt::replay`], `Serialize` but not
/// `Deserialize`: a reproduction result must come from reproducing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reproduced {
    manifest_digest: ContentHash,
    compared: Vec<String>,
    not_compared: Vec<String>,
    toolchain_differences: Vec<ToolchainDifference>,
}

impl Reproduced {
    pub fn manifest_digest(&self) -> &ContentHash {
        &self.manifest_digest
    }

    /// Entries whose digests were recomputed from replayed content and matched.
    pub fn compared(&self) -> &[String] {
        &self.compared
    }

    /// Entries the reproducer produced nothing for. A reproduction is only as broad as this is short.
    pub fn not_compared(&self) -> &[String] {
        &self.not_compared
    }

    /// Toolchain fields that differed or could not be compared, under
    /// [`ToolchainPolicy::RecordDifferences`]. Empty under [`ToolchainPolicy::RequireDeclaredMatch`],
    /// which would not have reached this state otherwise.
    pub fn toolchain_differences(&self) -> &[ToolchainDifference] {
        &self.toolchain_differences
    }

    /// True only when every manifest entry was compared.
    pub fn is_complete(&self) -> bool {
        self.not_compared.is_empty()
    }

    pub fn honest_label(&self) -> String {
        format!(
            "reproduced {} of {} manifest entries; {} not compared ({}); {} toolchain field(s) differed or were not comparable",
            self.compared.len(),
            self.compared.len() + self.not_compared.len(),
            self.not_compared.len(),
            if self.not_compared.is_empty() {
                "none".to_string()
            } else {
                self.not_compared.join(", ")
            },
            self.toolchain_differences.len()
        )
    }
}

/// The first entry at which a replay differed from the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Divergence {
    /// The named entry. A divergence without a name is a score, not a diagnostic.
    pub entry: String,
    pub expected: ContentHash,
    pub observed: ContentHash,
    /// Entries that matched before this one, in manifest order.
    pub matched_before: Vec<String>,
}

impl Divergence {
    pub fn honest_label(&self) -> String {
        format!(
            "diverged at entry `{}`: bundle records {}, replay produced {} ({} earlier entr{} matched)",
            self.entry,
            self.expected,
            self.observed,
            self.matched_before.len(),
            if self.matched_before.len() == 1 { "y" } else { "ies" }
        )
    }
}

/// Why a replay did not happen. Never merged into a divergence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum NotAttemptedReason {
    /// The bundle's manifest disagrees with its own contents. Nothing can be compared against it.
    BundleDidNotVerify { detail: String },
    /// [`ToolchainPolicy::RequireDeclaredMatch`] and the toolchains did not match, or could not be
    /// shown to match.
    ToolchainMismatch {
        differences: Vec<ToolchainDifference>,
    },
    /// The reproducer produced nothing that any manifest entry names.
    NothingComparable { manifest_entries: usize },
    /// Replayed content could not be reduced to canonical bytes, so it has no digest to compare.
    ReplayedContentUnhashable { entry: String, detail: String },
    /// A caller declined to replay, and said why.
    Refused { rationale: String },
}

impl NotAttemptedReason {
    pub fn explanation(&self) -> String {
        match self {
            NotAttemptedReason::BundleDidNotVerify { detail } => {
                format!("the bundle does not verify against its own contents ({detail})")
            }
            NotAttemptedReason::ToolchainMismatch { differences } => format!(
                "the host toolchain does not match the bundle's in {} field(s): {}",
                differences.len(),
                differences
                    .iter()
                    .map(ToolchainDifference::field)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            NotAttemptedReason::NothingComparable { manifest_entries } => format!(
                "the replay produced nothing matching any of the {manifest_entries} manifest entries"
            ),
            NotAttemptedReason::ReplayedContentUnhashable { entry, detail } => {
                format!("replayed content for `{entry}` has no canonical form ({detail})")
            }
            NotAttemptedReason::Refused { rationale } => rationale.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environment::EnvironmentFacts;
    use crate::manifest::EntryRole;
    use serde_json::json;

    fn section() -> Value {
        json!({"layers": ["l0", "l1"]})
    }

    fn query() -> Value {
        json!({"target": "t"})
    }

    fn bundle() -> ResultBundle {
        ResultBundle::builder("run-1")
            .carrying("section", EntryRole::DecisionSection, section())
            .expect("carries")
            .carrying("query", EntryRole::Query, query())
            .expect("carries")
            .in_environment(EnvironmentFacts::undeclared())
            .with_toolchain(ToolchainFacts::declared().with_rustc_version("1.85.0"))
            .build()
            .expect("builds")
    }

    fn host() -> ToolchainFacts {
        ToolchainFacts::declared().with_rustc_version("1.85.0")
    }

    #[test]
    fn an_identical_replay_reproduces_and_names_what_it_compared() {
        let verdict = ReproductionAttempt::on_toolchain(host())
            .producing("section", section())
            .producing("query", query())
            .replay(&bundle());
        let ReproductionVerdict::Reproduced(reproduced) = &verdict else {
            panic!("expected reproduction, got {verdict:?}");
        };
        assert_eq!(reproduced.compared(), ["query", "section"]);
        assert!(reproduced.not_compared().is_empty());
        assert!(reproduced.is_complete());
        assert_eq!(verdict.status_word(), "reproduced");
    }

    #[test]
    fn a_divergence_names_the_first_differing_entry_and_what_matched_before_it() {
        let verdict = ReproductionAttempt::on_toolchain(host())
            .producing("query", query())
            .producing("section", json!({"layers": ["l0"]}))
            .replay(&bundle());
        let ReproductionVerdict::Diverged { first_divergence } = &verdict else {
            panic!("expected divergence, got {verdict:?}");
        };
        assert_eq!(first_divergence.entry, "section");
        assert_eq!(first_divergence.matched_before, vec!["query".to_string()]);
        assert_ne!(first_divergence.expected, first_divergence.observed);
        assert_eq!(verdict.status_word(), "non-reproducible");
    }

    #[test]
    fn the_first_divergence_is_taken_in_manifest_order_not_insertion_order() {
        let verdict = ReproductionAttempt::on_toolchain(host())
            .producing("section", json!({"layers": []}))
            .producing("query", json!({"target": "other"}))
            .replay(&bundle());
        let ReproductionVerdict::Diverged { first_divergence } = &verdict else {
            panic!("expected divergence, got {verdict:?}");
        };
        assert_eq!(
            first_divergence.entry, "query",
            "`query` sorts before `section`, so it is the first divergence regardless of \
             which order the reproducer supplied results in"
        );
        assert!(first_divergence.matched_before.is_empty());
    }

    #[test]
    fn a_partial_replay_reproduces_without_claiming_the_entries_it_never_compared() {
        let verdict = ReproductionAttempt::on_toolchain(host())
            .producing("section", section())
            .replay(&bundle());
        let ReproductionVerdict::Reproduced(reproduced) = &verdict else {
            panic!("expected reproduction, got {verdict:?}");
        };
        assert_eq!(reproduced.compared(), ["section"]);
        assert_eq!(reproduced.not_compared(), ["query"]);
        assert!(!reproduced.is_complete());
        assert!(reproduced.honest_label().contains("1 not compared (query)"));
    }

    #[test]
    fn a_replay_that_compared_nothing_is_not_attempted_rather_than_reproduced() {
        let verdict = ReproductionAttempt::on_toolchain(host()).replay(&bundle());
        assert_eq!(
            verdict,
            ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::NothingComparable {
                    manifest_entries: 2
                }
            }
        );
        assert!(!verdict.is_reproduced());
    }

    #[test]
    fn a_bundle_that_does_not_verify_yields_not_attempted_and_not_diverged() {
        let mut broken = bundle();
        broken.contents.insert("section".into(), json!({"layers": []}));
        let verdict = ReproductionAttempt::on_toolchain(host())
            .producing("section", section())
            .producing("query", query())
            .replay(&broken);
        match &verdict {
            ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::BundleDidNotVerify { detail },
            } => assert!(detail.contains("section"), "{detail}"),
            other => panic!("a broken bundle must not be blamed on the reproducer: {other:?}"),
        }
    }

    #[test]
    fn a_strict_toolchain_policy_declines_rather_than_reporting_a_divergence() {
        let verdict = ReproductionAttempt::on_toolchain(
            ToolchainFacts::declared().with_rustc_version("1.90.0"),
        )
        .under_policy(ToolchainPolicy::RequireDeclaredMatch)
        .producing("section", section())
        .producing("query", query())
        .replay(&bundle());
        match &verdict {
            ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::ToolchainMismatch { differences },
            } => {
                assert_eq!(differences.len(), 1);
                assert_eq!(differences[0].field(), "rustc_version");
            }
            other => panic!("expected a declined replay, got {other:?}"),
        }
    }

    #[test]
    fn a_permissive_policy_reproduces_but_carries_the_toolchain_differences_with_the_result() {
        let verdict = ReproductionAttempt::on_toolchain(
            ToolchainFacts::declared().with_rustc_version("1.90.0"),
        )
        .under_policy(ToolchainPolicy::RecordDifferences)
        .producing("section", section())
        .producing("query", query())
        .replay(&bundle());
        let ReproductionVerdict::Reproduced(reproduced) = &verdict else {
            panic!("expected reproduction, got {verdict:?}");
        };
        assert_eq!(reproduced.toolchain_differences().len(), 1);
        assert!(reproduced.toolchain_differences()[0].is_disagreement());
        assert!(reproduced.honest_label().contains("1 toolchain field(s)"));
    }

    #[test]
    fn an_undeclared_host_toolchain_is_not_comparable_and_a_strict_policy_declines() {
        let verdict = ReproductionAttempt::on_toolchain(ToolchainFacts::declared())
            .under_policy(ToolchainPolicy::RequireDeclaredMatch)
            .producing("section", section())
            .replay(&bundle());
        match &verdict {
            ReproductionVerdict::NotAttempted {
                reason: NotAttemptedReason::ToolchainMismatch { differences },
            } => assert!(!differences[0].is_disagreement()),
            other => panic!("an unestablished match is not a match: {other:?}"),
        }
    }

    #[test]
    fn a_refusal_keeps_its_rationale_rather_than_becoming_an_empty_result() {
        let verdict = ReproductionAttempt::refused("controlled-access world unavailable to this reviewer");
        assert_eq!(verdict.status_word(), "not-attempted");
        assert!(verdict
            .honest_label()
            .contains("controlled-access world unavailable"));
    }

    #[test]
    fn the_three_verdicts_serialise_under_distinct_tags() {
        let reproduced = ReproductionAttempt::on_toolchain(host())
            .producing("section", section())
            .producing("query", query())
            .replay(&bundle());
        let diverged = ReproductionAttempt::on_toolchain(host())
            .producing("query", json!({"target": "other"}))
            .replay(&bundle());
        let not_attempted = ReproductionAttempt::refused("no");
        let tags: Vec<String> = [reproduced, diverged, not_attempted]
            .iter()
            .map(|verdict| {
                serde_json::to_value(verdict).expect("serialises")["verdict"]
                    .as_str()
                    .expect("tagged")
                    .to_string()
            })
            .collect();
        assert_eq!(tags, vec!["reproduced", "diverged", "not_attempted"]);
    }
}
