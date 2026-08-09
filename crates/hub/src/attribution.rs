//! Licence propagation and attribution, computed from ancestry rather than declared.
//!
//! Blueprint 36.14 (Data Licenses, Redistribution and Derived Artifacts) requires "policy
//! inheritance" and "attribution generation" as controls, and 36.12 lists "preserves data-use and
//! locality labels through derived artifacts" as a thing the platform itself must be evaluated on.
//! This module is that inheritance, made total.
//!
//! # Why the declared licence is not the answer
//!
//! The failure this guards against is quiet: a submitter takes a research-only pack, derives a
//! variant, and publishes it as permissive. Nobody lied about a score. The obligation simply
//! stopped being visible, and by the time anyone notices, ten further derivations exist.
//!
//! So [`LicenceStack::derive`] does not read the declared licence and trust it. It computes the
//! meet of every ancestor's terms — the most restrictive combination — and then checks that the
//! declaration is at least that restrictive. A looser declaration is [`HubError::LicenceEscalation`],
//! naming the terms that must be declared instead. Losing an ancestor's attribution is
//! [`HubError::MissingAttribution`]. Neither is a warning, because a warning on a public hub is a
//! thing that scrolls off the page.
//!
//! # What is not implemented
//!
//! No licence text is parsed and no SPDX identifier is resolved. [`Licence`] carries the four
//! properties the propagation rules actually consume, and a deployment is responsible for mapping
//! real licence documents onto them. Nothing here is legal advice; 36.14 lists "legal review" as a
//! separate control for good reason.

use crate::error::HubError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Redistribution terms, ordered from least to most restrictive.
///
/// The order is the point: `derive` takes the maximum, so adding an ancestor can only tighten the
/// result. `NoDerivatives` sorts above `ControlledAccess` because it is not a tier a derived
/// artifact can be published under at all — it is a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Redistribution {
    /// Public domain or permissive. Redistribution unrestricted.
    Permissive,
    /// Redistributable, but only for research use.
    ResearchOnly,
    /// Redistributable only to credentialed recipients under a data-use agreement.
    ControlledAccess,
    /// Not redistributable in modified form at all.
    NoDerivatives,
}

impl Redistribution {
    pub fn as_str(self) -> &'static str {
        match self {
            Redistribution::Permissive => "permissive",
            Redistribution::ResearchOnly => "research-only",
            Redistribution::ControlledAccess => "controlled-access",
            Redistribution::NoDerivatives => "no-derivatives",
        }
    }
}

impl fmt::Display for Redistribution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where an artifact may be served from, ordered from most open to most restricted.
///
/// The ordering of `Federated` against `Controlled` is a judgement the blueprint does not settle:
/// 34.15 lists `public|controlled|federated` as an unordered enumeration. Federated is treated as
/// the tighter of the two here because a federated artifact's underlying data never leaves its
/// site, so publishing it as merely `controlled` would understate the constraint. A deployment
/// that disagrees should change this one `Ord` derive and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessTier {
    Public,
    Controlled,
    Federated,
}

impl AccessTier {
    pub fn as_str(self) -> &'static str {
        match self {
            AccessTier::Public => "public",
            AccessTier::Controlled => "controlled",
            AccessTier::Federated => "federated",
        }
    }
}

impl fmt::Display for AccessTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The four licence properties the hub's propagation rules consume.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Licence {
    /// Human-facing name. Not parsed, not resolved, not validated against any registry.
    pub name: String,
    pub redistribution: Redistribution,
    pub attribution_required: bool,
    /// `false` means the artifact may not be used commercially.
    pub commercial_use: bool,
    pub access: AccessTier,
}

impl Licence {
    /// A permissive, public, attribution-free licence. Useful as a base to tighten from; a real
    /// submission should name its actual licence.
    pub fn permissive(name: impl Into<String>) -> Licence {
        Licence {
            name: name.into(),
            redistribution: Redistribution::Permissive,
            attribution_required: false,
            commercial_use: true,
            access: AccessTier::Public,
        }
    }

    /// Whether `self` is at least as restrictive as `other` in every dimension.
    pub fn is_at_least_as_restrictive_as(&self, other: &Licence) -> bool {
        self.redistribution >= other.redistribution
            && self.access >= other.access
            && (self.attribution_required || !other.attribution_required)
            && (!self.commercial_use || other.commercial_use)
    }
}

/// A rights holder that must be named wherever the artifact appears.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Attribution {
    pub holder: String,
    /// The citation string to reproduce. Not formatted or style-checked by the hub.
    pub citation: String,
    /// The content this attribution belongs to, so a stack can be checked against ancestry.
    pub source: ContentHash,
}

/// One artifact a submission was derived from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ancestor {
    pub content: ContentHash,
    pub licence: Licence,
    /// Present iff `licence.attribution_required`; enforced by [`LicenceStack::derive`].
    pub attribution: Option<Attribution>,
}

impl Ancestor {
    pub fn new(content: ContentHash, licence: Licence) -> Ancestor {
        Ancestor {
            content,
            licence,
            attribution: None,
        }
    }

    pub fn with_attribution(mut self, attribution: Attribution) -> Ancestor {
        self.attribution = Some(attribution);
        self
    }
}

/// The computed licence position of a submission: what it inherited, from whom, and who must be
/// credited.
///
/// Constructed only by [`LicenceStack::derive`]. There is no way to build one that disagrees with
/// its ancestors, which is the whole reason the fields are read-only accessors rather than a
/// public struct literal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenceStack {
    effective: Licence,
    ancestors: Vec<Ancestor>,
    attributions: Vec<Attribution>,
}

impl LicenceStack {
    /// The terms that actually apply: the meet of the declaration and every ancestor.
    pub fn effective(&self) -> &Licence {
        &self.effective
    }

    pub fn ancestors(&self) -> &[Ancestor] {
        &self.ancestors
    }

    /// Every rights holder that must appear wherever this artifact is shown, deduplicated and in
    /// a stable order so two renderings of the same stack are byte-identical.
    pub fn attributions(&self) -> &[Attribution] {
        &self.attributions
    }

    /// True when this submission descends from something else.
    pub fn is_derived(&self) -> bool {
        !self.ancestors.is_empty()
    }

    /// The terms a submitter would have to declare, given this ancestry. Exposed so a refusal can
    /// be acted on rather than merely read.
    pub fn required_terms(declared: &Licence, ancestors: &[Ancestor]) -> Licence {
        let mut required = declared.clone();
        for ancestor in ancestors {
            required.redistribution = required.redistribution.max(ancestor.licence.redistribution);
            required.access = required.access.max(ancestor.licence.access);
            required.attribution_required |= ancestor.licence.attribution_required;
            required.commercial_use &= ancestor.licence.commercial_use;
        }
        required
    }

    /// Compute the licence position of a derived submission, or refuse.
    ///
    /// Refusals, in the order checked:
    ///
    /// 1. any ancestor licensed [`Redistribution::NoDerivatives`] — the derivation should not
    ///    exist, so no amount of correct attribution rescues it;
    /// 2. an ancestor that requires attribution but names no holder — the obligation is known to
    ///    exist and is unsatisfiable, which is worse than an absent one;
    /// 3. a required attribution missing from `provided`;
    /// 4. a declared access tier more open than an ancestor's;
    /// 5. a declared licence looser than the computed meet.
    ///
    /// Ordering matters for the error a submitter sees first: the unfixable refusal is reported
    /// before the fixable ones.
    pub fn derive(
        declared: &Licence,
        ancestors: &[Ancestor],
        provided: &[Attribution],
    ) -> Result<LicenceStack, HubError> {
        for ancestor in ancestors {
            if ancestor.licence.redistribution == Redistribution::NoDerivatives {
                return Err(HubError::DerivativeForbidden {
                    ancestor: ancestor.content.to_string(),
                });
            }
        }

        let mut attributions: Vec<Attribution> = Vec::new();
        for ancestor in ancestors {
            if !ancestor.licence.attribution_required {
                continue;
            }
            let Some(required) = ancestor.attribution.as_ref() else {
                return Err(HubError::AttributionUnspecified {
                    ancestor: ancestor.content.to_string(),
                });
            };
            let carried = provided
                .iter()
                .any(|a| a.holder == required.holder && a.source == required.source);
            if !carried {
                return Err(HubError::MissingAttribution {
                    ancestor: ancestor.content.to_string(),
                    holder: required.holder.clone(),
                });
            }
            attributions.push(required.clone());
        }

        for ancestor in ancestors {
            if declared.access < ancestor.licence.access {
                return Err(HubError::AccessTierEscalation {
                    declared: declared.access.as_str(),
                    ancestor: ancestor.content.to_string(),
                    required: ancestor.licence.access.as_str(),
                });
            }
        }

        let required = LicenceStack::required_terms(declared, ancestors);
        if !declared.is_at_least_as_restrictive_as(&required) {
            let offender = ancestors
                .iter()
                .max_by_key(|a| a.licence.redistribution)
                .map(|a| a.content.to_string())
                .unwrap_or_else(|| "<self>".to_string());
            let ancestor_terms = ancestors
                .iter()
                .map(|a| a.licence.redistribution)
                .max()
                .unwrap_or(declared.redistribution);
            return Err(HubError::LicenceEscalation {
                declared: declared.name.clone(),
                declared_terms: declared.redistribution.as_str(),
                ancestor: offender,
                ancestor_terms: ancestor_terms.as_str(),
                required_terms: required.redistribution.as_str(),
            });
        }

        for extra in provided {
            if !attributions.contains(extra) {
                attributions.push(extra.clone());
            }
        }
        attributions.sort();
        attributions.dedup();

        Ok(LicenceStack {
            effective: required,
            ancestors: ancestors.to_vec(),
            attributions,
        })
    }

    /// The credit line a public page must render. Empty string when nothing is owed, so a caller
    /// cannot accidentally print a header over no content.
    pub fn credit_line(&self) -> String {
        if self.attributions.is_empty() {
            return String::new();
        }
        let holders: Vec<&str> = self
            .attributions
            .iter()
            .map(|a| a.holder.as_str())
            .collect();
        format!(
            "Derived from work by {}. Terms: {} ({} access).",
            holders.join(", "),
            self.effective.redistribution,
            self.effective.access
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn research_only(name: &str) -> Licence {
        Licence {
            name: name.into(),
            redistribution: Redistribution::ResearchOnly,
            attribution_required: true,
            commercial_use: false,
            access: AccessTier::Public,
        }
    }

    #[test]
    fn a_submission_with_no_ancestors_keeps_its_declared_terms() {
        let declared = Licence::permissive("CC0-1.0");
        let stack = LicenceStack::derive(&declared, &[], &[]).expect("no ancestry to inherit");
        assert_eq!(stack.effective(), &declared);
        assert!(!stack.is_derived());
        assert_eq!(stack.credit_line(), "");
    }

    #[test]
    fn derived_licence_is_the_most_restrictive_ancestor_not_the_declared_one() {
        let ancestor_licence = research_only("research-only-v1");
        let attribution = Attribution {
            holder: "Consortium A".into(),
            citation: "Consortium A, 2026".into(),
            source: hash("ancestor"),
        };
        let ancestor = Ancestor::new(hash("ancestor"), ancestor_licence)
            .with_attribution(attribution.clone());

        let declared = Licence {
            name: "derived".into(),
            redistribution: Redistribution::ResearchOnly,
            attribution_required: true,
            commercial_use: false,
            access: AccessTier::Public,
        };
        let stack = LicenceStack::derive(&declared, &[ancestor], &[attribution])
            .expect("declaration already matches the meet");
        assert_eq!(stack.effective().redistribution, Redistribution::ResearchOnly);
        assert!(!stack.effective().commercial_use);
        assert!(stack.is_derived());
    }

    #[test]
    fn a_derived_submission_declaring_a_looser_licence_than_its_ancestor_is_refused() {
        let attribution = Attribution {
            holder: "Consortium A".into(),
            citation: "Consortium A, 2026".into(),
            source: hash("ancestor"),
        };
        let ancestor = Ancestor::new(hash("ancestor"), research_only("research-only-v1"))
            .with_attribution(attribution.clone());

        let err = LicenceStack::derive(
            &Licence::permissive("MIT"),
            &[ancestor],
            &[attribution],
        )
        .expect_err("permissive declaration over a research-only ancestor");
        assert_eq!(
            err,
            HubError::LicenceEscalation {
                declared: "MIT".into(),
                declared_terms: "permissive",
                ancestor: hash("ancestor").to_string(),
                ancestor_terms: "research-only",
                required_terms: "research-only",
            }
        );
    }

    #[test]
    fn dropping_an_ancestor_attribution_is_an_error_not_a_warning() {
        let attribution = Attribution {
            holder: "Consortium A".into(),
            citation: "Consortium A, 2026".into(),
            source: hash("ancestor"),
        };
        let ancestor = Ancestor::new(hash("ancestor"), research_only("research-only-v1"))
            .with_attribution(attribution);

        let declared = Licence {
            name: "derived".into(),
            redistribution: Redistribution::ResearchOnly,
            attribution_required: true,
            commercial_use: false,
            access: AccessTier::Public,
        };
        let err = LicenceStack::derive(&declared, &[ancestor], &[])
            .expect_err("attribution was dropped");
        assert!(matches!(err, HubError::MissingAttribution { ref holder, .. } if holder == "Consortium A"));
    }

    #[test]
    fn a_no_derivatives_ancestor_blocks_the_derivation_outright() {
        let mut licence = research_only("nd");
        licence.redistribution = Redistribution::NoDerivatives;
        licence.attribution_required = false;
        let ancestor = Ancestor::new(hash("nd-ancestor"), licence);
        let declared = Licence {
            name: "derived".into(),
            redistribution: Redistribution::NoDerivatives,
            attribution_required: false,
            commercial_use: false,
            access: AccessTier::Controlled,
        };
        let err = LicenceStack::derive(&declared, &[ancestor], &[])
            .expect_err("no-derivatives ancestor");
        assert_eq!(
            err,
            HubError::DerivativeForbidden {
                ancestor: hash("nd-ancestor").to_string()
            }
        );
    }

    #[test]
    fn a_controlled_ancestor_cannot_be_republished_as_public() {
        let mut licence = Licence::permissive("controlled-src");
        licence.access = AccessTier::Controlled;
        let ancestor = Ancestor::new(hash("controlled"), licence);
        let err = LicenceStack::derive(&Licence::permissive("MIT"), &[ancestor], &[])
            .expect_err("public declaration over a controlled ancestor");
        assert_eq!(
            err,
            HubError::AccessTierEscalation {
                declared: "public",
                ancestor: hash("controlled").to_string(),
                required: "controlled",
            }
        );
    }

    #[test]
    fn an_ancestor_requiring_attribution_but_naming_no_holder_is_unsatisfiable() {
        let ancestor = Ancestor::new(hash("vague"), research_only("vague"));
        let declared = Licence {
            name: "derived".into(),
            redistribution: Redistribution::ResearchOnly,
            attribution_required: true,
            commercial_use: false,
            access: AccessTier::Public,
        };
        let err = LicenceStack::derive(&declared, &[ancestor], &[])
            .expect_err("attribution required but unspecified");
        assert_eq!(
            err,
            HubError::AttributionUnspecified {
                ancestor: hash("vague").to_string()
            }
        );
    }

    #[test]
    fn attribution_order_is_stable_so_two_renderings_agree() {
        let a = Attribution {
            holder: "Z Lab".into(),
            citation: "Z, 2026".into(),
            source: hash("z"),
        };
        let b = Attribution {
            holder: "A Lab".into(),
            citation: "A, 2026".into(),
            source: hash("a"),
        };
        let declared = Licence::permissive("MIT");
        let one = LicenceStack::derive(&declared, &[], &[a.clone(), b.clone()]).unwrap();
        let two = LicenceStack::derive(&declared, &[], &[b, a]).unwrap();
        assert_eq!(one.attributions(), two.attributions());
        assert_eq!(one.credit_line(), two.credit_line());
    }
}
