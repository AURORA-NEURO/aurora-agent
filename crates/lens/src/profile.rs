//! Context and token profile — blueprint 42.21.
//!
//! 42.21 asks to show "mandatory closure, optional frontier, included facts, omitted subgraphs,
//! token cost, expansions, and marginal utility". Six of those seven are inventory questions this
//! lens can answer from a compiled context. The seventh is discussed under *Not implemented*.
//!
//! The two things worth checking here are both refusals of a silent default:
//!
//! **A protected fact that was omitted is an error, not a saving.** 43.01 requires protected
//! closure to be computed before any relevance step and 42.01 says a view must "never silently
//! truncate protected biological invariants". A token profile that shows a smaller context
//! without showing that the smallness came from dropping mandatory evidence is an advertisement.
//! [`ProfileFinding::ProtectedFactOmitted`] names the fact.
//!
//! **An unpriced expansion is not a free one.** The token cost of a frontier item is a
//! [`Recorded<usize>`], not a `usize`, so an item whose cost nobody measured cannot silently
//! contribute zero to a budget. This is the unmeasured-is-not-zero rule applied to a number that
//! looks harmless — and a budget built from `unwrap_or(0)` is exactly how a context overruns.
//!
//! # Not implemented
//!
//! **Marginal utility.** 42.21 lists it and defines it nowhere: no utility function, no outcome
//! model, no unit. Computing one would require deciding what a fact is worth, which is the
//! decision the whole platform is built to leave to the compiler and the certificate. The lens
//! reports token cost and lets the reader do the division it is willing to justify.
//!
//! **Tokenisation.** There is no tokenizer here. Costs arrive as input, because a count produced
//! by the wrong tokenizer is worse than an absent one — it is an absent one wearing a number.

use crate::grammar::{
    Coverage, EvidenceRequirement, Lens, LensDeclaration, LensId, LensOutcome, PendingRegion,
    RefusalReason, ScopePrecondition,
};
use crate::missingness::Recorded;
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::ScopeKey;
use bioprism_section::{InfluenceClass, Layer, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One fact in the compiled context, with the layer it sits in and what it costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FactEntry {
    pub fact: String,
    pub layer: Layer,
    /// Token cost. `Recorded` rather than `usize`: an unpriced fact must not weigh zero.
    pub tokens: Recorded<usize>,
    /// Whether protected closure requires this fact regardless of relevance.
    pub protected: bool,
}

impl FactEntry {
    pub fn priced(fact: impl Into<String>, layer: Layer, tokens: usize, protected: bool) -> Self {
        FactEntry {
            fact: fact.into(),
            layer,
            tokens: Recorded::known(tokens),
            protected,
        }
    }

    pub fn unpriced(fact: impl Into<String>, layer: Layer, protected: bool) -> Self {
        FactEntry {
            fact: fact.into(),
            layer,
            tokens: Recorded::unrecorded(),
            protected,
        }
    }
}

/// A compiled context, as the profiler reads it.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ContextProfile {
    /// Everything protected closure demanded, whether or not it was included.
    pub closure: Vec<FactEntry>,
    /// Optional expansions the compiler could have taken.
    #[serde(default)]
    pub frontier: Vec<FactEntry>,
    /// The facts that actually reached the section, by name.
    #[serde(default)]
    pub included: Vec<String>,
    /// What was left out, in the vocabulary of 43.26.
    #[serde(default)]
    pub omissions: OmissionManifest,
    /// Layers the caller has not fetched yet — L4 raw artifacts, typically.
    #[serde(default)]
    pub unfetched_layers: Vec<Layer>,
}

impl ContextProfile {
    pub fn new(closure: Vec<FactEntry>, included: Vec<String>) -> Self {
        ContextProfile {
            closure,
            frontier: Vec::new(),
            included,
            omissions: OmissionManifest::default(),
            unfetched_layers: Vec::new(),
        }
    }

    pub fn with_frontier(mut self, frontier: Vec<FactEntry>) -> Self {
        self.frontier = frontier;
        self
    }

    pub fn with_omissions(mut self, omissions: OmissionManifest) -> Self {
        self.omissions = omissions;
        self
    }

    /// Token cost per layer, counting only facts whose cost was measured.
    ///
    /// Deliberately paired with [`ContextProfile::unpriced_facts`]: the total is honest only
    /// alongside the count of things it could not include.
    pub fn priced_tokens_by_layer(&self) -> BTreeMap<Layer, usize> {
        let mut by_layer = BTreeMap::new();
        for entry in self.closure.iter().chain(&self.frontier) {
            if let Some(tokens) = entry.tokens.value() {
                *by_layer.entry(entry.layer).or_insert(0) += *tokens;
            }
        }
        by_layer
    }

    /// Facts whose token cost nobody measured. These contribute to no total.
    pub fn unpriced_facts(&self) -> Vec<&FactEntry> {
        self.closure
            .iter()
            .chain(&self.frontier)
            .filter(|entry| !entry.tokens.is_known())
            .collect()
    }
}

/// What the context profiler found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileFinding {
    /// A protected-closure fact that did not reach the section. Forbidden, not economical.
    ProtectedFactOmitted { fact: String, layer: Layer },
    /// An omission group nobody analysed. One of these voids the sufficiency claim.
    UnknownInfluenceOmission { reason: String, count: usize },
    /// An omission the policy fibers blocked. Real, stated, and not the same as unanalysed.
    PolicyBlockedOmission { reason: String, count: usize },
    /// An expansion whose token cost was never measured, so no budget can account for it.
    UnpricedExpansion { fact: String, layer: Layer },
    /// A fact that reached the section without appearing in closure or frontier — the profile
    /// and the section disagree about what was compiled.
    UnaccountedInclusion { fact: String },
}

impl Witness for ProfileFinding {
    fn kind(&self) -> &'static str {
        match self {
            ProfileFinding::ProtectedFactOmitted { .. } => "protected_fact_omitted",
            ProfileFinding::UnknownInfluenceOmission { .. } => "unknown_influence_omission",
            ProfileFinding::PolicyBlockedOmission { .. } => "policy_blocked_omission",
            ProfileFinding::UnpricedExpansion { .. } => "unpriced_expansion",
            ProfileFinding::UnaccountedInclusion { .. } => "unaccounted_inclusion",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            ProfileFinding::ProtectedFactOmitted { .. }
            | ProfileFinding::UnpricedExpansion { .. } => &["fact", "layer"],
            ProfileFinding::UnknownInfluenceOmission { .. }
            | ProfileFinding::PolicyBlockedOmission { .. } => &["reason", "facts"],
            ProfileFinding::UnaccountedInclusion { .. } => &["fact"],
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            ProfileFinding::ProtectedFactOmitted { fact, layer }
            | ProfileFinding::UnpricedExpansion { fact, layer } => {
                vec![Cell::id(fact.clone()), Cell::text(layer.as_str())]
            }
            ProfileFinding::UnknownInfluenceOmission { reason, count }
            | ProfileFinding::PolicyBlockedOmission { reason, count } => {
                vec![Cell::text(reason.clone()), Cell::count(*count)]
            }
            ProfileFinding::UnaccountedInclusion { fact } => vec![Cell::id(fact.clone())],
        }
    }

    fn sentence(&self) -> String {
        match self {
            ProfileFinding::ProtectedFactOmitted { fact, layer } => format!(
                "`{fact}` ({}) is required by protected closure and did not reach the section; \
                 the context is smaller because it is incomplete",
                layer.as_str()
            ),
            ProfileFinding::UnknownInfluenceOmission { reason, count } => format!(
                "{count} fact(s) omitted for `{reason}` were never analysed for influence, so \
                 this context supports no sufficiency claim"
            ),
            ProfileFinding::PolicyBlockedOmission { reason, count } => format!(
                "{count} fact(s) omitted for `{reason}` are blocked by policy; the gap is real \
                 and must be carried into the decision"
            ),
            ProfileFinding::UnpricedExpansion { fact, layer } => format!(
                "`{fact}` ({}) has no measured token cost, so expanding it is not free — it is \
                 unbudgeted",
                layer.as_str()
            ),
            ProfileFinding::UnaccountedInclusion { fact } => {
                format!("`{fact}` reached the section but appears in neither closure nor frontier")
            }
        }
    }
}

/// Blueprint 42.21.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContextTokenProfileLens;

impl ContextTokenProfileLens {
    pub const ID: &'static str = "context_token_profile";
}

impl Lens for ContextTokenProfileLens {
    type Evidence = ContextProfile;
    type Witness = ProfileFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.21",
            "what did this context cost, what did it leave out, and is anything it left out \
             something it was not allowed to leave out?",
            vec![
                EvidenceRequirement::new("profile.closure", "the protected closure, with costs"),
                EvidenceRequirement::new("profile.frontier", "optional expansions, with costs"),
                EvidenceRequirement::new("profile.included", "the facts that reached the section"),
                EvidenceRequirement::new(
                    "profile.omissions",
                    "the omission manifest of 43.26, grouped by structural reason",
                ),
            ],
            vec![ScopePrecondition::new(
                "query",
                "closure is defined against query obligations; without a query there is no \
                 mandatory set",
            )],
            vec![RefusalReason::ScopePreconditionUnmet],
        )
        .expect("42.21 declaration is well formed")
    }

    fn answer(&self, _scope: &ScopeKey, profile: &ContextProfile) -> LensOutcome<ProfileFinding> {
        let mut findings = Vec::new();

        for entry in &profile.closure {
            if entry.protected && !profile.included.contains(&entry.fact) {
                findings.push(ProfileFinding::ProtectedFactOmitted {
                    fact: entry.fact.clone(),
                    layer: entry.layer,
                });
            }
        }

        for group in &profile.omissions.groups {
            match group.influence {
                InfluenceClass::Unknown => {
                    findings.push(ProfileFinding::UnknownInfluenceOmission {
                        reason: group.reason.clone(),
                        count: group.count,
                    })
                }
                InfluenceClass::InaccessibleByPolicy => {
                    findings.push(ProfileFinding::PolicyBlockedOmission {
                        reason: group.reason.clone(),
                        count: group.count,
                    })
                }
                InfluenceClass::Zero
                | InfluenceClass::Bounded
                | InfluenceClass::DeferredAcquisition => {}
            }
        }

        for entry in profile.unpriced_facts() {
            findings.push(ProfileFinding::UnpricedExpansion {
                fact: entry.fact.clone(),
                layer: entry.layer,
            });
        }

        for fact in &profile.included {
            let known = profile
                .closure
                .iter()
                .chain(&profile.frontier)
                .any(|entry| entry.fact == *fact);
            if !known {
                findings.push(ProfileFinding::UnaccountedInclusion { fact: fact.clone() });
            }
        }

        let examined = profile.closure.len() + profile.frontier.len();
        let eligible = examined + profile.unfetched_layers.len();
        let coverage = if profile.unfetched_layers.is_empty() {
            Coverage::complete(Self::ID, examined, eligible)
        } else {
            Coverage::partial(
                Self::ID,
                examined,
                eligible,
                profile
                    .unfetched_layers
                    .iter()
                    .map(|layer| PendingRegion::new(layer.as_str(), "layer not fetched"))
                    .collect(),
            )
        };
        match coverage {
            Ok(coverage) => LensOutcome::Answered {
                witnesses: findings,
                coverage,
            },
            Err(_) => LensOutcome::Answered {
                witnesses: findings,
                coverage: Coverage::complete(Self::ID, examined, examined)
                    .expect("examined equals itself"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::run;
    use bioprism_section::OmissionGroup;

    fn scope() -> ScopeKey {
        ScopeKey::new().exact("query", "Q-1")
    }

    #[test]
    fn a_protected_fact_that_did_not_reach_the_section_is_an_error_not_a_saving() {
        let profile = ContextProfile::new(
            vec![
                FactEntry::priced("germline_status", Layer::L2, 40, true),
                FactEntry::priced("stage", Layer::L2, 10, true),
            ],
            vec!["stage".into()],
        );
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "protected_fact_omitted")
            .expect("omitted protected fact reported");
        assert!(row.sentence.contains("germline_status"));
        assert!(row.sentence.contains("incomplete"));
    }

    #[test]
    fn an_unpriced_expansion_contributes_to_no_token_total() {
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        )
        .with_frontier(vec![FactEntry::unpriced("path_report", Layer::L4, false)]);
        assert_eq!(profile.priced_tokens_by_layer().get(&Layer::L4), None);
        assert_eq!(profile.priced_tokens_by_layer()[&Layer::L2], 10);
        assert_eq!(profile.unpriced_facts().len(), 1);
    }

    #[test]
    fn an_unpriced_expansion_is_reported_rather_than_counted_as_zero() {
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        )
        .with_frontier(vec![FactEntry::unpriced("path_report", Layer::L4, false)]);
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "unpriced_expansion")
            .expect("unpriced expansion reported");
        assert!(row.sentence.contains("unbudgeted"));
    }

    #[test]
    fn an_unanalysed_omission_group_voids_the_sufficiency_claim() {
        let mut omissions = OmissionManifest::default();
        omissions.push(OmissionGroup {
            reason: "distant_subgraph".into(),
            influence: InfluenceClass::Unknown,
            count: 812,
            bound: None,
            examples: Vec::new(),
        });
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        )
        .with_omissions(omissions);
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "unknown_influence_omission")
            .expect("unanalysed omission reported");
        assert!(row.sentence.contains("812"));
        assert!(row.sentence.contains("no sufficiency claim"));
    }

    #[test]
    fn a_zero_influence_omission_is_not_reported_as_a_problem() {
        let mut omissions = OmissionManifest::default();
        omissions.push(OmissionGroup {
            reason: "no_dependency_path".into(),
            influence: InfluenceClass::Zero,
            count: 5000,
            bound: None,
            examples: Vec::new(),
        });
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        )
        .with_omissions(omissions);
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        assert!(report.witnesses().is_empty());
    }

    #[test]
    fn a_policy_blocked_omission_is_not_collapsed_into_an_unanalysed_one() {
        let mut omissions = OmissionManifest::default();
        omissions.push(OmissionGroup {
            reason: "consent_tier".into(),
            influence: InfluenceClass::InaccessibleByPolicy,
            count: 3,
            bound: None,
            examples: Vec::new(),
        });
        omissions.push(OmissionGroup {
            reason: "distant_subgraph".into(),
            influence: InfluenceClass::Unknown,
            count: 7,
            bound: None,
            examples: Vec::new(),
        });
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        )
        .with_omissions(omissions);
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        let kinds: Vec<&str> = report.witnesses().iter().map(|r| r.kind.as_str()).collect();
        assert!(kinds.contains(&"policy_blocked_omission"));
        assert!(kinds.contains(&"unknown_influence_omission"));
        assert_eq!(kinds.len(), 2);
    }

    #[test]
    fn a_fact_in_the_section_that_no_plan_accounts_for_is_reported() {
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into(), "mystery_fact".into()],
        );
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        assert!(report
            .witnesses()
            .iter()
            .any(|r| r.kind == "unaccounted_inclusion" && r.sentence.contains("mystery_fact")));
    }

    #[test]
    fn an_unfetched_layer_makes_the_profile_partial_and_names_the_layer() {
        let mut profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        );
        profile.unfetched_layers = vec![Layer::L4];
        let report = run(&ContextTokenProfileLens, &scope(), &profile).unwrap();
        assert!(!report.completeness().is_complete());
        assert!(report.spoken().iter().any(|l| l.contains("pending l4")));
    }

    #[test]
    fn the_lens_refuses_without_a_query_because_closure_has_no_meaning() {
        let profile = ContextProfile::new(
            vec![FactEntry::priced("stage", Layer::L2, 10, true)],
            vec!["stage".into()],
        );
        let report = run(&ContextTokenProfileLens, &ScopeKey::new(), &profile).unwrap();
        assert_eq!(report.outcome().as_str(), "refused");
    }
}
