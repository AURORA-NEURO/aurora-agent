//! Conformance evidence, and the trust level it earns.
//!
//! Blueprint 40.16's first non-negotiable invariant is the one this module exists for:
//! **"Registration does not imply trust."** 43.35 puts conformance evidence in the same sentence
//! as discoverable capabilities, and 11.12 asks the platform to "publish plugin trust evidence".
//!
//! # Trust is computed from the evidence, never stored on the plugin
//!
//! There is no `trust_level` field on a manifest, for the reason `bioprism-registry` gives about
//! packs: a stored tier is an assertion, and the party with the strongest incentive to make it is
//! the publisher. [`assess_trust`] is a function of the declaration plus the attached evidence, so
//! two hosts reading the same manifest reach the same level without trusting each other.
//!
//! # The three rules that do the work
//!
//! 1. **Silence is the floor.** No evidence means [`TrustLevel::Unverified`]. Not "probably fine";
//!    the same distinction `bioprism-fiber` insists on between *measured poorly* and *unmeasured*.
//! 2. **Coverage is per capability.** A plugin that provides an adapter and an oracle and brings
//!    an adapter suite is verified as an adapter and unverified as an oracle, so its level is the
//!    lower of the two. Evidence does not spread sideways.
//! 3. **Evidence is bound to content.** Evidence naming a different core digest is evidence about
//!    a different plugin. `bioprism-registry` puts it as "a review of earlier content does not
//!    carry forward"; the same holds when the content is a plugin.

use crate::capability::CapabilityKind;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// One conformance-suite run, attached to a manifest by whoever ran it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceEvidence {
    /// Which suite, e.g. `adapter-conformance/normalize`.
    pub suite: String,
    /// Which capability the run exercised. Evidence for one kind says nothing about another.
    pub covers: CapabilityKind,
    pub cases_passed: usize,
    pub cases_failed: usize,
    /// Who ran it. Compared against the manifest's publisher to decide independence.
    pub attested_by: String,
    /// The core digest of the plugin the run covered, when the runner recorded one.
    ///
    /// `None` is not fatal but is never independent-grade: evidence that does not say what it
    /// tested cannot be shown to have tested *this*.
    pub subject_core_sha256: Option<ContentHash>,
}

impl ConformanceEvidence {
    pub fn new(
        suite: impl Into<String>,
        covers: CapabilityKind,
        attested_by: impl Into<String>,
    ) -> Self {
        ConformanceEvidence {
            suite: suite.into(),
            covers,
            cases_passed: 0,
            cases_failed: 0,
            attested_by: attested_by.into(),
            subject_core_sha256: None,
        }
    }

    pub fn passing(mut self, cases: usize) -> Self {
        self.cases_passed = cases;
        self
    }

    pub fn failing(mut self, cases: usize) -> Self {
        self.cases_failed = cases;
        self
    }

    pub fn covering_core(mut self, digest: ContentHash) -> Self {
        self.subject_core_sha256 = Some(digest);
        self
    }

    /// A run with at least one case and no failures.
    ///
    /// Zero passes and zero failures is *not* passing. An empty suite that reports success is the
    /// cheapest way to manufacture evidence, and it is indistinguishable from a suite that failed
    /// to start.
    pub fn is_passing(&self) -> bool {
        self.cases_failed == 0 && self.cases_passed > 0
    }

    /// Whether this evidence describes exactly `core`.
    ///
    /// Evidence with no recorded subject matches nothing: it is usable for self-attestation, where
    /// the publisher is vouching for their own artefact anyway, and never for independence.
    pub fn covers_core(&self, core: &ContentHash) -> bool {
        self.subject_core_sha256.as_ref() == Some(core)
    }

    pub fn is_independent_of(&self, publisher: &str) -> bool {
        let attester = self.attested_by.trim();
        !attester.is_empty() && attester != publisher.trim()
    }
}

/// How much of the plugin's behaviour has been checked, and by whom.
///
/// Ordered, and the order is the whole point: a host compares against a floor rather than matching
/// a variant, so raising the floor does not mean editing every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Nothing was checked, or what was checked failed, or the evidence is about other content.
    /// Registration still succeeds; selection for load-bearing work does not.
    Unverified,
    /// Every declared capability has a passing suite, run by the publisher.
    SelfAttested,
    /// Every declared capability has a passing suite run by somebody other than the publisher,
    /// naming this exact content.
    IndependentlyVerified,
}

impl TrustLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            TrustLevel::Unverified => "unverified",
            TrustLevel::SelfAttested => "self-attested",
            TrustLevel::IndependentlyVerified => "independently-verified",
        }
    }
}

impl fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The level plus the sentences that explain it.
///
/// `reasons` is populated on the way *down*: it lists what stopped the plugin reaching the next
/// level, so an author is told "the oracle capability has no passing suite" rather than
/// "unverified".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrustAssessment {
    pub level: TrustLevel,
    pub reasons: Vec<String>,
}

impl TrustAssessment {
    pub fn describe_reasons(&self) -> String {
        if self.reasons.is_empty() {
            "no outstanding gaps".to_string()
        } else {
            self.reasons.join("; ")
        }
    }
}

/// Computes the trust level for a set of declared capabilities and the evidence attached to them.
///
/// `core` is the digest of the plugin's content *excluding* its evidence — see
/// [`crate::PluginManifest::core_digest`] — because evidence that had to hash itself could never
/// be attached after the fact.
pub fn assess_trust(
    publisher: &str,
    declared: &BTreeSet<CapabilityKind>,
    evidence: &[ConformanceEvidence],
    core: &ContentHash,
) -> TrustAssessment {
    let mut reasons = Vec::new();

    if evidence.is_empty() {
        reasons.push("no conformance evidence attached".to_string());
        return TrustAssessment {
            level: TrustLevel::Unverified,
            reasons,
        };
    }

    let mut uncovered = Vec::new();
    let mut not_independent = Vec::new();

    for kind in declared {
        let for_kind: Vec<&ConformanceEvidence> = evidence
            .iter()
            .filter(|entry| entry.covers == *kind)
            .collect();

        if !for_kind.iter().any(|entry| entry.is_passing()) {
            let detail = if for_kind.is_empty() {
                format!("{kind} has no conformance evidence")
            } else {
                format!("{kind} has {} suite(s), none passing", for_kind.len())
            };
            uncovered.push(detail);
            continue;
        }

        if !for_kind.iter().any(|entry| {
            entry.is_passing() && entry.is_independent_of(publisher) && entry.covers_core(core)
        }) {
            not_independent.push(format!(
                "{kind} has no passing suite that is both independent of {publisher:?} and names \
                 core digest {core}"
            ));
        }
    }

    if !uncovered.is_empty() {
        reasons.extend(uncovered);
        return TrustAssessment {
            level: TrustLevel::Unverified,
            reasons,
        };
    }

    if !not_independent.is_empty() {
        reasons.extend(not_independent);
        return TrustAssessment {
            level: TrustLevel::SelfAttested,
            reasons,
        };
    }

    TrustAssessment {
        level: TrustLevel::IndependentlyVerified,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> ContentHash {
        ContentHash::of_bytes(b"core")
    }

    fn declared(kinds: &[CapabilityKind]) -> BTreeSet<CapabilityKind> {
        kinds.iter().copied().collect()
    }

    #[test]
    fn a_plugin_with_no_evidence_assesses_as_unverified() {
        let assessment = assess_trust("acme", &declared(&[CapabilityKind::Oracle]), &[], &core());
        assert_eq!(assessment.level, TrustLevel::Unverified);
        assert!(assessment
            .describe_reasons()
            .contains("no conformance evidence"));
    }

    #[test]
    fn a_suite_that_ran_zero_cases_is_not_a_passing_suite() {
        let evidence = ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "acme");
        assert!(!evidence.is_passing());
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::Unverified);
    }

    #[test]
    fn evidence_from_the_publisher_earns_self_attestation_and_no_more() {
        let evidence = ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "acme")
            .passing(12)
            .covering_core(core());
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::SelfAttested);
        assert!(assessment.describe_reasons().contains("independent"));
    }

    #[test]
    fn evidence_from_a_third_party_naming_this_content_earns_independent_verification() {
        let evidence = ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "reviewer")
            .passing(12)
            .covering_core(core());
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::IndependentlyVerified);
        assert!(assessment.reasons.is_empty());
    }

    #[test]
    fn independent_evidence_about_other_content_does_not_carry_forward() {
        let evidence = ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "reviewer")
            .passing(12)
            .covering_core(ContentHash::of_bytes(b"an earlier build"));
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::SelfAttested);
    }

    #[test]
    fn evidence_for_one_capability_does_not_verify_another() {
        let evidence = ConformanceEvidence::new("adapter/normalize", CapabilityKind::Adapter, "r")
            .passing(3)
            .covering_core(core());
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Adapter, CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::Unverified);
        assert!(assessment.describe_reasons().contains("oracle"));
    }

    #[test]
    fn a_failing_suite_does_not_lift_a_plugin_off_the_floor() {
        let evidence = ConformanceEvidence::new("oracle/basic", CapabilityKind::Oracle, "reviewer")
            .passing(9)
            .failing(1)
            .covering_core(core());
        let assessment = assess_trust(
            "acme",
            &declared(&[CapabilityKind::Oracle]),
            &[evidence],
            &core(),
        );
        assert_eq!(assessment.level, TrustLevel::Unverified);
    }

    #[test]
    fn trust_levels_are_ordered_so_a_host_can_compare_against_a_floor() {
        assert!(TrustLevel::Unverified < TrustLevel::SelfAttested);
        assert!(TrustLevel::SelfAttested < TrustLevel::IndependentlyVerified);
    }
}
