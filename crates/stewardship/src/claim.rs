//! What a result may be *said* to show, given what was measured.
//!
//! Implements blueprint 14.11 (Result Claims and Leaderboard Governance): its six claim classes,
//! its eligibility rules, its wording requirements, its challenge and correction lifecycle, and its
//! promotion rule.
//!
//! # This is not a leaderboard, and there is not a second one
//!
//! `bioprism-hub` owns boards, entries, ranking and the refusal to order two entries evaluated
//! under different conditions. `bioprism-metrics` owns `Dominance::Incomparable` as a first-class
//! state that no tiebreak collapses. `bioprism-atlas` owns the claim ladder of 43.40: which tier
//! the *evidence* licenses. All three were read before this module was written and none of them is
//! reimplemented here.
//!
//! What is left over is the sentence. A board can be perfectly ordered and every number on it true,
//! and the paragraph published alongside it can still say something the evidence does not support —
//! "best", with no comparison set; "general", from one domain; "causal", from a metric picked after
//! the fact; "independently reproduced", by the team that published it. 14.11 is about that
//! paragraph, and this module makes it a structured object with a gate in front of it:
//!
//! > **A claim that outruns its evidence is not constructible.**
//!
//! [`IssuedClaim`] has private fields, one constructor ([`ClaimSentence::issue`]), and `Serialize`
//! without `Deserialize` — the pattern of `bioprism_lab::CleanMeasurement` and
//! [`crate::review::DimensionScopedApproval`]. Every check below happens on the way in, so a claim
//! that exists has already passed them, and a claim that cannot pass them does not exist as a value
//! to be published by accident.
//!
//! # Three ceilings, and none of them can be raised from here
//!
//! 1. **Evidence tier.** `bioprism_atlas::ClaimTier` is what the atlas says the evidence licenses.
//!    [`ClaimClass::requirements`] states the tier each class needs, and the check is a comparison,
//!    never an override. There is no argument to [`ClaimSentence::issue`] that raises a tier.
//! 2. **Pack trust tier.** `bioprism_registry::TrustTier` is computed from pack bytes and cannot be
//!    asserted, by construction, in the crate that owns it. This module reads one.
//! 3. **Predeclaration.** A [`ClaimClass::ResearchCausal`] claim requires a
//!    [`crate::predeclaration::ConfirmatoryFinding`], which itself cannot be deserialised into
//!    existence. So a causal claim built on a metric chosen after the results is refused two layers
//!    down, in a crate that knows nothing about claims.
//!
//! # Wording is structure, not prose
//!
//! 14.11: *"Claims name population, metric, uncertainty, baseline, resource policy, and
//! limitations. 'Best' requires defined comparison set and date; 'general' requires broad validated
//! coverage."* [`ClaimSentence`] has one field per requirement and a
//! [`ComparisonAssertion`] enum for the two loaded words, so the superlative check is a check on
//! [`ComparisonAssertion::BestIn`]'s comparison set rather than a search for the string "best".
//!
//! `bioprism_hub::lint_claim` does the phrase-level check on free text and is the right tool for
//! that job. A governance layer built on regexes over English would be defeated by a synonym; this
//! one is defeated by lying about the structured fields, which is a different and more visible act.
//!
//! # Corrections leave a tombstone
//!
//! 14.11: *"Never erase the original claim without a tombstone."* [`ClaimRegister`] is append-only.
//! There is no `remove`, no `retract`, and no mutation of an event once appended. A withdrawal
//! appends a [`Tombstone`] carrying the original sentence, and [`ClaimRegister::history`] returns
//! every event including the ones somebody would rather forget.
//!
//! # Not implemented
//!
//! No rendering, no board, no ordering of claims against each other, no notification, no clock. No
//! independent-reproduction *verification*: [`ReproductionRecord`] records that a named party other
//! than the publisher reproduced the result, and nothing here checks that they did. 14.11's
//! "independent rerun or council review" is a human process; what is mechanised is the structural
//! part — that the rerunner is not the publisher, and that a dispute is not closed by the party it
//! was raised against.

use crate::error::ClaimError;
use crate::id::{Actor, ActorId, Epoch};
use crate::predeclaration::ConfirmatoryFinding;
use bioprism_atlas::ClaimTier;
use bioprism_registry::TrustTier;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// How many validated domains a generality claim needs before it may say "general".
///
/// 14.11 requires "broad validated coverage" and gives no number. Three is this crate's position,
/// stated here rather than buried: it is the smallest count for which "broad" is not obviously a
/// lie, and a reader who thinks it should be higher can see exactly what they are disagreeing with.
pub const GENERALITY_DOMAIN_FLOOR: usize = 3;

/// 14.11's six claim classes, in the module's order, with evidence requirements that increase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimClass {
    /// "We ran this and here is what happened." Makes no comparison and needs no baseline.
    DescriptiveRun,
    /// A comparison inside one organisation, not published as a benchmark result.
    InternalComparison,
    /// A result on a public benchmark, published.
    PublicBenchmarkResult,
    /// A result somebody else reproduced.
    IndependentlyReproduced,
    /// A claim that something *caused* something. The class that needs a predeclared analysis.
    ResearchCausal,
    /// A claim that the system is ready for production use.
    ProductionReadiness,
}

impl ClaimClass {
    pub const ALL: [ClaimClass; 6] = [
        ClaimClass::DescriptiveRun,
        ClaimClass::InternalComparison,
        ClaimClass::PublicBenchmarkResult,
        ClaimClass::IndependentlyReproduced,
        ClaimClass::ResearchCausal,
        ClaimClass::ProductionReadiness,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ClaimClass::DescriptiveRun => "descriptive_run",
            ClaimClass::InternalComparison => "internal_comparison",
            ClaimClass::PublicBenchmarkResult => "public_benchmark_result",
            ClaimClass::IndependentlyReproduced => "independently_reproduced",
            ClaimClass::ResearchCausal => "research_causal",
            ClaimClass::ProductionReadiness => "production_readiness",
        }
    }

    /// Whether this class is published outside the organisation that produced it.
    ///
    /// The line matters because 14.11's wording requirements — uncertainty, resource policy,
    /// limitations — exist to protect a reader who was not in the room. An internal comparison has
    /// no such reader.
    pub const fn public(self) -> bool {
        !matches!(
            self,
            ClaimClass::DescriptiveRun | ClaimClass::InternalComparison
        )
    }

    /// The evidence this class requires.
    ///
    /// The tier mapping is derived from 43.40's ladder as `bioprism-atlas` defines it, not invented:
    /// a public benchmark result needs evidence from public observed worlds, an independently
    /// reproduced result needs the cross-domain rung it takes to reproduce anything, a causal claim
    /// needs the controlled multi-site rung, and a production-readiness claim needs the prospective
    /// workflow rung — which is the top of the ladder, and correctly hard to reach.
    pub const fn requirements(self) -> EvidenceRequirements {
        match self {
            ClaimClass::DescriptiveRun => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::UnitConformance,
                min_pack_tier: TrustTier::Exploratory,
                needs_confirmatory: false,
                needs_independent_reproduction: false,
            },
            ClaimClass::InternalComparison => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::SyntheticStructural,
                min_pack_tier: TrustTier::GeneratedVerified,
                needs_confirmatory: false,
                needs_independent_reproduction: false,
            },
            ClaimClass::PublicBenchmarkResult => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::PublicObservedWorlds,
                min_pack_tier: TrustTier::Reviewed,
                needs_confirmatory: false,
                needs_independent_reproduction: false,
            },
            ClaimClass::IndependentlyReproduced => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::CrossDomainPublic,
                min_pack_tier: TrustTier::Reviewed,
                needs_confirmatory: false,
                needs_independent_reproduction: true,
            },
            ClaimClass::ResearchCausal => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::ControlledHiddenMultiSite,
                min_pack_tier: TrustTier::Gold,
                needs_confirmatory: true,
                needs_independent_reproduction: false,
            },
            ClaimClass::ProductionReadiness => EvidenceRequirements {
                class: self,
                min_claim_tier: ClaimTier::ProspectiveWorkflow,
                min_pack_tier: TrustTier::Gold,
                needs_confirmatory: true,
                needs_independent_reproduction: true,
            },
        }
    }
}

impl fmt::Display for ClaimClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a class demands, as data rather than as branches inside a checker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRequirements {
    pub class: ClaimClass,
    pub min_claim_tier: ClaimTier,
    pub min_pack_tier: TrustTier,
    pub needs_confirmatory: bool,
    pub needs_independent_reproduction: bool,
}

/// The comparison a claim asserts, which is the part that goes wrong.
///
/// Enumerated rather than free text, so that the two loaded words of 14.11 — "best" and "general" —
/// are variants with obligations attached rather than strings a reviewer might miss.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "comparison", rename_all = "snake_case")]
pub enum ComparisonAssertion {
    /// The claim compares nothing. Always permitted; the honest default.
    None,
    /// Better than one named baseline under the stated conditions.
    BetterThan { baseline: String },
    /// The superlative. Requires the comparison set and the epoch it was true at.
    BestIn {
        comparison_set: Vec<String>,
        as_of: Epoch,
    },
    /// The generality claim. Requires validated domains, and enough of them.
    Generally { validated_domains: Vec<String> },
}

impl ComparisonAssertion {
    /// The check 14.11 asks for, run at construction rather than at review time.
    pub fn check(&self) -> Result<(), ClaimError> {
        match self {
            ComparisonAssertion::None => Ok(()),
            ComparisonAssertion::BetterThan { baseline } => {
                if baseline.trim().is_empty() {
                    Err(ClaimError::SentenceIncomplete {
                        class: "any",
                        missing: "baseline",
                    })
                } else {
                    Ok(())
                }
            }
            ComparisonAssertion::BestIn { comparison_set, .. } => {
                if comparison_set.is_empty() {
                    Err(ClaimError::SuperlativeWithoutComparisonSet)
                } else {
                    Ok(())
                }
            }
            ComparisonAssertion::Generally { validated_domains } => {
                if validated_domains.len() < GENERALITY_DOMAIN_FLOOR {
                    Err(ClaimError::GeneralityWithoutBreadth {
                        required: GENERALITY_DOMAIN_FLOOR,
                        stated: validated_domains.len(),
                    })
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Whether this assertion compares the subject against anything at all. A claim that compares
    /// needs a baseline named somewhere, which is why [`ComparisonAssertion::None`] exempts it.
    pub const fn comparative(&self) -> bool {
        !matches!(self, ComparisonAssertion::None)
    }
}

/// What the result is about, held as identifiers rather than as objects.
///
/// This crate does not hold packs, runs or architectures; it holds the claim, and a claim is about
/// things it names. The pack's earned tier travels with the subject because it is the eligibility
/// input, and it is a `TrustTier` read from `bioprism-registry` rather than a string, so it cannot
/// be spelled optimistically.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSubject {
    pub pack: String,
    pub pack_tier: TrustTier,
    pub system: String,
    pub run: String,
}

impl ClaimSubject {
    pub fn new(
        pack: impl Into<String>,
        pack_tier: TrustTier,
        system: impl Into<String>,
        run: impl Into<String>,
    ) -> Self {
        ClaimSubject {
            pack: pack.into(),
            pack_tier,
            system: system.into(),
            run: run.into(),
        }
    }
}

/// Somebody other than the publisher re-ran the result.
///
/// Nothing here verifies that they did. What is checked is the one structural thing that can be:
/// that the reproducing party is not the publishing party.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReproductionRecord {
    pub by: Actor,
    pub at: Epoch,
    pub artifact: String,
}

impl ReproductionRecord {
    pub fn new(by: Actor, at: Epoch, artifact: impl Into<String>) -> Self {
        ReproductionRecord {
            by,
            at,
            artifact: artifact.into(),
        }
    }
}

/// The claim as drafted, one field per thing 14.11 requires a claim to name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimSentence {
    pub class: ClaimClass,
    pub subject: ClaimSubject,
    pub publisher: Actor,
    /// Who or what the result is about: the task population, not the sample.
    pub population: String,
    pub metric: String,
    /// 14.11 requires uncertainty on a public claim. `None` is representable because an internal
    /// descriptive run need not carry one; it is refused for a public class.
    pub uncertainty: Option<String>,
    pub resource_policy: Option<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub comparison: ComparisonAssertion,
}

impl ClaimSentence {
    pub fn new(
        class: ClaimClass,
        subject: ClaimSubject,
        publisher: Actor,
        population: impl Into<String>,
        metric: impl Into<String>,
    ) -> Self {
        ClaimSentence {
            class,
            subject,
            publisher,
            population: population.into(),
            metric: metric.into(),
            uncertainty: None,
            resource_policy: None,
            limitations: Vec::new(),
            comparison: ComparisonAssertion::None,
        }
    }

    #[must_use]
    pub fn with_uncertainty(mut self, text: impl Into<String>) -> Self {
        self.uncertainty = Some(text.into());
        self
    }

    #[must_use]
    pub fn with_resource_policy(mut self, text: impl Into<String>) -> Self {
        self.resource_policy = Some(text.into());
        self
    }

    #[must_use]
    pub fn limited_by(mut self, text: impl Into<String>) -> Self {
        self.limitations.push(text.into());
        self
    }

    #[must_use]
    pub fn asserting(mut self, comparison: ComparisonAssertion) -> Self {
        self.comparison = comparison;
        self
    }

    /// Issue the claim, or say which rule refused it.
    ///
    /// `licensed` is the tier `bioprism_atlas::permitted_claim` returned for this capability;
    /// passing a higher one is possible only by lying about what the atlas said, which is a lie
    /// about a different crate's output rather than a gap in this one.
    ///
    /// The order of checks is deliberate: structural completeness of the sentence first, because
    /// those are the failures an author can fix; evidence ceilings second, because those are the
    /// failures that mean the claim should not be made at all.
    pub fn issue(
        self,
        licensed: ClaimTier,
        confirmatory: Option<&ConfirmatoryFinding>,
        reproduction: Option<&ReproductionRecord>,
        at: Epoch,
    ) -> Result<IssuedClaim, ClaimError> {
        let requirements = self.class.requirements();
        let class = self.class.as_str();

        if self.population.trim().is_empty() {
            return Err(ClaimError::SentenceIncomplete {
                class,
                missing: "population",
            });
        }
        if self.metric.trim().is_empty() {
            return Err(ClaimError::SentenceIncomplete {
                class,
                missing: "metric",
            });
        }
        self.comparison.check()?;

        if self.class.public() {
            if self.uncertainty.as_deref().unwrap_or("").trim().is_empty() {
                return Err(ClaimError::SentenceIncomplete {
                    class,
                    missing: "uncertainty",
                });
            }
            if self
                .resource_policy
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
            {
                return Err(ClaimError::SentenceIncomplete {
                    class,
                    missing: "resource policy",
                });
            }
            if self.limitations.is_empty() {
                return Err(ClaimError::SentenceIncomplete {
                    class,
                    missing: "limitations",
                });
            }
        }

        if licensed < requirements.min_claim_tier {
            return Err(ClaimError::EvidenceBelowClass {
                class,
                required: requirements.min_claim_tier.to_string(),
                available: licensed.to_string(),
            });
        }
        if self.subject.pack_tier < requirements.min_pack_tier {
            return Err(ClaimError::PackTierBelowClass {
                class,
                required: requirements.min_pack_tier.as_str().to_string(),
                earned: self.subject.pack_tier.as_str().to_string(),
            });
        }

        if requirements.needs_confirmatory && confirmatory.is_none() {
            return Err(ClaimError::CausalClaimWithoutPredeclaration);
        }
        if let Some(finding) = confirmatory {
            if finding.metric() != self.metric {
                return Err(ClaimError::CausalClaimWithoutPredeclaration);
            }
        }

        if requirements.needs_independent_reproduction {
            let record = reproduction.ok_or(ClaimError::SelfReproduction {
                party: self.publisher.id.clone(),
            })?;
            if record.by.id == self.publisher.id {
                return Err(ClaimError::SelfReproduction {
                    party: self.publisher.id.clone(),
                });
            }
        }

        Ok(IssuedClaim {
            id: ClaimId::of(&self, at),
            sentence: self,
            licensed,
            confirmatory: confirmatory.map(|f| f.plan_digest().as_str().to_string()),
            reproduced_by: reproduction.map(|r| r.by.id.clone()),
            issued_at: at,
        })
    }
}

/// Identifier for a registered claim.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimId(String);

impl ClaimId {
    pub fn new(id: impl Into<String>) -> Self {
        ClaimId(id.into())
    }

    fn of(sentence: &ClaimSentence, at: Epoch) -> Self {
        ClaimId(format!(
            "{}/{}/{}/{}",
            sentence.subject.pack, sentence.subject.system, sentence.subject.run, at.0
        ))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ClaimId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A claim that passed every check in [`ClaimSentence::issue`].
///
/// Private fields, one constructor, `Serialize` without `Deserialize`. A claim parsed out of JSON
/// would be a claim nobody checked, which is the exact artifact 14.11 exists to prevent — so the
/// only way to hold one is to have built it from evidence that satisfied the class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "an issued claim is the object a register publishes; dropping it discards the checks"]
pub struct IssuedClaim {
    id: ClaimId,
    sentence: ClaimSentence,
    licensed: ClaimTier,
    confirmatory: Option<String>,
    reproduced_by: Option<ActorId>,
    issued_at: Epoch,
}

impl IssuedClaim {
    pub fn id(&self) -> &ClaimId {
        &self.id
    }

    pub fn class(&self) -> ClaimClass {
        self.sentence.class
    }

    pub fn sentence(&self) -> &ClaimSentence {
        &self.sentence
    }

    pub fn licensed(&self) -> ClaimTier {
        self.licensed
    }

    pub fn publisher(&self) -> &ActorId {
        &self.sentence.publisher.id
    }

    pub fn issued_at(&self) -> Epoch {
        self.issued_at
    }

    /// The digest of the sealed analysis plan, when there was one.
    pub fn predeclaration(&self) -> Option<&str> {
        self.confirmatory.as_deref()
    }

    pub fn reproduced_by(&self) -> Option<&ActorId> {
        self.reproduced_by.as_ref()
    }
}

impl fmt::Display for IssuedClaim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} claim about {} on {}: {} over {}",
            self.sentence.class,
            self.sentence.subject.system,
            self.sentence.subject.pack,
            self.sentence.metric,
            self.sentence.population
        )
    }
}

/// What 14.11 permits a highlight to be chosen on.
///
/// The module: *"Homepage/highlight selection uses reproducibility, diagnostic significance,
/// efficiency, and open artifacts—not only top score or sponsor status."* The two refused bases are
/// variants rather than absent, so that a caller who wants to promote on score alone has to name
/// what they are doing and be refused, rather than finding the option simply missing and picking
/// the nearest permitted label.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionBasis {
    Reproducibility,
    DiagnosticSignificance,
    Efficiency,
    OpenArtifacts,
    /// Refused.
    TopScoreAlone,
    /// Refused.
    SponsorRelationship,
}

impl PromotionBasis {
    pub const fn as_str(self) -> &'static str {
        match self {
            PromotionBasis::Reproducibility => "reproducibility",
            PromotionBasis::DiagnosticSignificance => "diagnostic_significance",
            PromotionBasis::Efficiency => "efficiency",
            PromotionBasis::OpenArtifacts => "open_artifacts",
            PromotionBasis::TopScoreAlone => "top_score_alone",
            PromotionBasis::SponsorRelationship => "sponsor_relationship",
        }
    }

    pub const fn permitted(self) -> bool {
        !matches!(
            self,
            PromotionBasis::TopScoreAlone | PromotionBasis::SponsorRelationship
        )
    }
}

/// What survives a withdrawal.
///
/// 14.11: "Never erase the original claim without a tombstone." The tombstone carries the sentence
/// that was withdrawn, not a summary of it, because the point of a tombstone is that the reader can
/// see what was said.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tombstone {
    pub claim: ClaimId,
    pub sentence: ClaimSentence,
    pub reason: String,
    pub at: Epoch,
}

/// One thing that happened to a claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ClaimEvent {
    Published {
        at: Epoch,
        by: ActorId,
    },
    /// 14.11: "Anyone can submit reproducible evidence of error/exploit."
    Disputed {
        at: Epoch,
        by: ActorId,
        evidence: String,
    },
    /// The dispute was examined and the claim stands.
    DisputeResolved {
        at: Epoch,
        by: ActorId,
        finding: String,
    },
    /// Metadata or metric corrected; the claim continues to exist in amended form.
    Corrected {
        at: Epoch,
        by: ActorId,
        what: String,
    },
    Withdrawn {
        at: Epoch,
        by: ActorId,
        reason: String,
    },
    Superseded {
        at: Epoch,
        by: ActorId,
        successor: ClaimId,
    },
    Promoted {
        at: Epoch,
        by: ActorId,
        basis: PromotionBasis,
    },
}

impl ClaimEvent {
    pub fn at(&self) -> Epoch {
        match self {
            ClaimEvent::Published { at, .. }
            | ClaimEvent::Disputed { at, .. }
            | ClaimEvent::DisputeResolved { at, .. }
            | ClaimEvent::Corrected { at, .. }
            | ClaimEvent::Withdrawn { at, .. }
            | ClaimEvent::Superseded { at, .. }
            | ClaimEvent::Promoted { at, .. } => *at,
        }
    }
}

/// Where a claim currently stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ClaimStatus {
    Published,
    /// 14.11: a challenged result "receives disputed status". Disputed is not withdrawn: the claim
    /// stays visible and stays readable, and what it loses is eligibility for promotion.
    Disputed {
        by: ActorId,
    },
    Corrected,
    Withdrawn {
        reason: String,
    },
    Superseded {
        by: ClaimId,
    },
}

impl ClaimStatus {
    /// Whether a claim in this state may be highlighted.
    pub const fn promotable(&self) -> bool {
        matches!(self, ClaimStatus::Published | ClaimStatus::Corrected)
    }
}

/// The append-only record of published claims.
///
/// Not a leaderboard: it does not order claims, score them, or compare two of them.
/// `bioprism-hub` owns that. What this owns is the lifecycle 14.11 specifies — publish, dispute,
/// resolve, correct, withdraw, supersede — under one rule that shapes the whole API: **nothing is
/// removed**. There is no `remove`, no `delete`, and no method that takes `&mut ClaimEvent`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClaimRegister {
    claims: BTreeMap<ClaimId, IssuedClaimRecord>,
    tombstones: Vec<Tombstone>,
}

/// The stored form. `IssuedClaim` is not `Deserialize`, so the register keeps the sentence and the
/// facts derived from it; a register read back from JSON holds records, and issuing remains the
/// only way to obtain an [`IssuedClaim`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct IssuedClaimRecord {
    sentence: ClaimSentence,
    licensed: ClaimTier,
    predeclaration: Option<String>,
    reproduced_by: Option<ActorId>,
    status: ClaimStatus,
    history: Vec<ClaimEvent>,
}

impl ClaimRegister {
    pub fn new() -> Self {
        ClaimRegister::default()
    }

    pub fn publish(&mut self, claim: IssuedClaim) -> Result<ClaimId, ClaimError> {
        if self.claims.contains_key(&claim.id) {
            return Err(ClaimError::ClaimAlreadyRegistered {
                claim: claim.id.to_string(),
            });
        }
        let id = claim.id.clone();
        let published = ClaimEvent::Published {
            at: claim.issued_at,
            by: claim.sentence.publisher.id.clone(),
        };
        self.claims.insert(
            id.clone(),
            IssuedClaimRecord {
                sentence: claim.sentence,
                licensed: claim.licensed,
                predeclaration: claim.confirmatory,
                reproduced_by: claim.reproduced_by,
                status: ClaimStatus::Published,
                history: vec![published],
            },
        );
        Ok(id)
    }

    fn record_mut(&mut self, id: &ClaimId) -> Result<&mut IssuedClaimRecord, ClaimError> {
        self.claims
            .get_mut(id)
            .ok_or_else(|| ClaimError::ClaimUnknown {
                claim: id.to_string(),
            })
    }

    pub fn status(&self, id: &ClaimId) -> Option<&ClaimStatus> {
        self.claims.get(id).map(|r| &r.status)
    }

    pub fn history(&self, id: &ClaimId) -> Option<&[ClaimEvent]> {
        self.claims.get(id).map(|r| r.history.as_slice())
    }

    pub fn sentence(&self, id: &ClaimId) -> Option<&ClaimSentence> {
        self.claims.get(id).map(|r| &r.sentence)
    }

    pub fn licensed(&self, id: &ClaimId) -> Option<ClaimTier> {
        self.claims.get(id).map(|r| r.licensed)
    }

    pub fn tombstones(&self) -> &[Tombstone] {
        &self.tombstones
    }

    /// Anyone may dispute. That is 14.11's rule and it is not qualified here.
    pub fn dispute(
        &mut self,
        id: &ClaimId,
        by: &Actor,
        evidence: impl Into<String>,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        let record = self.record_mut(id)?;
        if let ClaimStatus::Withdrawn { .. } = record.status {
            return Err(ClaimError::ClaimWithdrawn {
                claim: id.to_string(),
            });
        }
        record.history.push(ClaimEvent::Disputed {
            at,
            by: by.id.clone(),
            evidence: evidence.into(),
        });
        record.status = ClaimStatus::Disputed { by: by.id.clone() };
        Ok(())
    }

    /// Close a dispute in the claim's favour.
    ///
    /// Refused when the resolver is the publisher or the challenger. 14.11 asks for "independent
    /// rerun or council review"; independence of a council is not observable from here, but "not
    /// one of the two parties" is, and it is the part that can be enforced.
    pub fn resolve_dispute(
        &mut self,
        id: &ClaimId,
        by: &Actor,
        finding: impl Into<String>,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        let record = self.record_mut(id)?;
        if record.sentence.publisher.id == by.id {
            return Err(ClaimError::SelfResolvedDispute {
                party: by.id.clone(),
            });
        }
        if let ClaimStatus::Disputed { by: challenger } = &record.status {
            if *challenger == by.id {
                return Err(ClaimError::SelfResolvedDispute {
                    party: by.id.clone(),
                });
            }
        }
        record.history.push(ClaimEvent::DisputeResolved {
            at,
            by: by.id.clone(),
            finding: finding.into(),
        });
        record.status = ClaimStatus::Published;
        Ok(())
    }

    pub fn correct(
        &mut self,
        id: &ClaimId,
        by: &Actor,
        what: impl Into<String>,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        let record = self.record_mut(id)?;
        if let ClaimStatus::Withdrawn { .. } = record.status {
            return Err(ClaimError::ClaimWithdrawn {
                claim: id.to_string(),
            });
        }
        record.history.push(ClaimEvent::Corrected {
            at,
            by: by.id.clone(),
            what: what.into(),
        });
        record.status = ClaimStatus::Corrected;
        Ok(())
    }

    /// Withdraw, leaving a tombstone that carries the original sentence.
    pub fn withdraw(
        &mut self,
        id: &ClaimId,
        by: &Actor,
        reason: impl Into<String>,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        let reason = reason.into();
        let record = self.record_mut(id)?;
        if let ClaimStatus::Withdrawn { .. } = record.status {
            return Err(ClaimError::ClaimWithdrawn {
                claim: id.to_string(),
            });
        }
        let sentence = record.sentence.clone();
        record.history.push(ClaimEvent::Withdrawn {
            at,
            by: by.id.clone(),
            reason: reason.clone(),
        });
        record.status = ClaimStatus::Withdrawn {
            reason: reason.clone(),
        };
        self.tombstones.push(Tombstone {
            claim: id.clone(),
            sentence,
            reason,
            at,
        });
        Ok(())
    }

    pub fn supersede(
        &mut self,
        id: &ClaimId,
        successor: &ClaimId,
        by: &Actor,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        if !self.claims.contains_key(successor) {
            return Err(ClaimError::ClaimUnknown {
                claim: successor.to_string(),
            });
        }
        let record = self.record_mut(id)?;
        record.history.push(ClaimEvent::Superseded {
            at,
            by: by.id.clone(),
            successor: successor.clone(),
        });
        record.status = ClaimStatus::Superseded {
            by: successor.clone(),
        };
        Ok(())
    }

    /// Promote a claim to a highlight on a stated basis.
    ///
    /// Two refusals: a basis 14.11 excludes, and a claim whose status is not promotable. A disputed
    /// claim is the case that matters — the number is still true, the dispute is unresolved, and
    /// putting it on the front page is the decision 14.11 is written to prevent.
    pub fn promote(
        &mut self,
        id: &ClaimId,
        by: &Actor,
        basis: PromotionBasis,
        at: Epoch,
    ) -> Result<(), ClaimError> {
        if !basis.permitted() {
            return Err(ClaimError::PromotionBasisNotPermitted {
                basis: basis.as_str(),
            });
        }
        let record = self.record_mut(id)?;
        if !record.status.promotable() {
            return match &record.status {
                ClaimStatus::Withdrawn { .. } => Err(ClaimError::ClaimWithdrawn {
                    claim: id.to_string(),
                }),
                _ => Err(ClaimError::ClaimDisputed {
                    claim: id.to_string(),
                }),
            };
        }
        record.history.push(ClaimEvent::Promoted {
            at,
            by: by.id.clone(),
            basis,
        });
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.claims.len()
    }

    pub fn is_empty(&self) -> bool {
        self.claims.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::predeclaration::{AnalysisPlan, ClusteringUnit, ObservedResults, TrialDisposition};

    fn subject(tier: TrustTier) -> ClaimSubject {
        ClaimSubject::new("pack-a", tier, "arch-a", "run-7")
    }

    fn publisher() -> Actor {
        Actor::author("lab-a")
    }

    fn public_sentence(class: ClaimClass, tier: TrustTier) -> ClaimSentence {
        ClaimSentence::new(
            class,
            subject(tier),
            publisher(),
            "structural biology decision cells, 40 parents",
            "first-divergence-rate",
        )
        .with_uncertainty("95% interval clustered at parent task")
        .with_resource_policy("8k tokens, 3 retries, 60s wall")
        .limited_by("no wet-lab validation")
    }

    fn confirmatory() -> ConfirmatoryFinding {
        let sealed = AnalysisPlan::new(
            "first-divergence-rate",
            ClusteringUnit::ParentTask,
            40,
            "after 400",
        )
        .seal(Epoch(1))
        .unwrap();
        let results = ObservedResults::new(Epoch(5))
            .measuring("first-divergence-rate", 0.2)
            .counting(TrialDisposition::Scored, 400);
        sealed.confirm(&results, "first-divergence-rate").unwrap()
    }

    #[test]
    fn a_claim_that_outruns_its_evidence_is_not_constructible() {
        let sentence = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed);
        assert!(matches!(
            sentence
                .clone()
                .issue(ClaimTier::SyntheticStructural, None, None, Epoch(6)),
            Err(ClaimError::EvidenceBelowClass { .. })
        ));
        assert!(sentence
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .is_ok());
    }

    #[test]
    fn a_superlative_without_a_comparison_set_is_refused() {
        let sentence = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .asserting(ComparisonAssertion::BestIn {
                comparison_set: Vec::new(),
                as_of: Epoch(6),
            });
        assert!(matches!(
            sentence.issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6)),
            Err(ClaimError::SuperlativeWithoutComparisonSet)
        ));
    }

    #[test]
    fn a_generality_claim_from_two_domains_is_refused() {
        let sentence = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .asserting(ComparisonAssertion::Generally {
                validated_domains: vec!["structural".into(), "genomic".into()],
            });
        assert!(matches!(
            sentence.issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6)),
            Err(ClaimError::GeneralityWithoutBreadth {
                required: 3,
                stated: 2
            })
        ));
    }

    #[test]
    fn a_causal_claim_requires_a_predeclared_analysis() {
        let sentence = public_sentence(ClaimClass::ResearchCausal, TrustTier::Gold);
        assert!(matches!(
            sentence
                .clone()
                .issue(ClaimTier::ControlledHiddenMultiSite, None, None, Epoch(6)),
            Err(ClaimError::CausalClaimWithoutPredeclaration)
        ));
        assert!(sentence
            .issue(
                ClaimTier::ControlledHiddenMultiSite,
                Some(&confirmatory()),
                None,
                Epoch(6)
            )
            .is_ok());
    }

    #[test]
    fn a_causal_claim_cannot_cite_a_predeclaration_for_a_different_metric() {
        let mut sentence = public_sentence(ClaimClass::ResearchCausal, TrustTier::Gold);
        sentence.metric = "abstention-rate".into();
        assert!(matches!(
            sentence.issue(
                ClaimTier::ControlledHiddenMultiSite,
                Some(&confirmatory()),
                None,
                Epoch(6)
            ),
            Err(ClaimError::CausalClaimWithoutPredeclaration)
        ));
    }

    #[test]
    fn a_result_reproduced_by_its_own_publisher_is_not_independently_reproduced() {
        let sentence = public_sentence(ClaimClass::IndependentlyReproduced, TrustTier::Reviewed);
        let self_repro = ReproductionRecord::new(publisher(), Epoch(6), "bundle-1");
        assert!(matches!(
            sentence.clone().issue(
                ClaimTier::CrossDomainPublic,
                None,
                Some(&self_repro),
                Epoch(7)
            ),
            Err(ClaimError::SelfReproduction { .. })
        ));
        let other = ReproductionRecord::new(Actor::independent_reviewer("lab-b"), Epoch(6), "b-1");
        assert!(sentence
            .issue(ClaimTier::CrossDomainPublic, None, Some(&other), Epoch(7))
            .is_ok());
    }

    #[test]
    fn a_public_claim_must_state_uncertainty_resource_policy_and_limitations() {
        let base = ClaimSentence::new(
            ClaimClass::PublicBenchmarkResult,
            subject(TrustTier::Reviewed),
            publisher(),
            "population",
            "metric",
        );
        assert!(matches!(
            base.clone()
                .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6)),
            Err(ClaimError::SentenceIncomplete {
                missing: "uncertainty",
                ..
            })
        ));
        assert!(matches!(
            base.clone().with_uncertainty("ci").issue(
                ClaimTier::PublicObservedWorlds,
                None,
                None,
                Epoch(6)
            ),
            Err(ClaimError::SentenceIncomplete {
                missing: "resource policy",
                ..
            })
        ));
        assert!(matches!(
            base.with_uncertainty("ci")
                .with_resource_policy("8k")
                .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6)),
            Err(ClaimError::SentenceIncomplete {
                missing: "limitations",
                ..
            })
        ));
    }

    #[test]
    fn an_internal_comparison_is_not_held_to_the_public_wording_requirements() {
        let sentence = ClaimSentence::new(
            ClaimClass::InternalComparison,
            subject(TrustTier::GeneratedVerified),
            publisher(),
            "internal regression set",
            "first-divergence-rate",
        );
        assert!(sentence
            .issue(ClaimTier::SyntheticStructural, None, None, Epoch(3))
            .is_ok());
    }

    #[test]
    fn a_pack_below_the_class_trust_tier_blocks_the_claim() {
        let sentence = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Exploratory);
        assert!(matches!(
            sentence.issue(ClaimTier::ProspectiveWorkflow, None, None, Epoch(6)),
            Err(ClaimError::PackTierBelowClass { .. })
        ));
    }

    #[test]
    fn a_disputed_claim_cannot_be_promoted() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        let challenger = Actor::independent_reviewer("lab-c");
        register
            .dispute(&id, &challenger, "seed leakage in split", Epoch(7))
            .unwrap();
        assert!(matches!(
            register.promote(&id, &challenger, PromotionBasis::Reproducibility, Epoch(8)),
            Err(ClaimError::ClaimDisputed { .. })
        ));
    }

    #[test]
    fn a_dispute_cannot_be_resolved_by_the_publisher_or_the_challenger() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        let challenger = Actor::independent_reviewer("lab-c");
        register
            .dispute(&id, &challenger, "leak", Epoch(7))
            .unwrap();
        assert!(matches!(
            register.resolve_dispute(&id, &publisher(), "no leak", Epoch(8)),
            Err(ClaimError::SelfResolvedDispute { .. })
        ));
        assert!(matches!(
            register.resolve_dispute(&id, &challenger, "still a leak", Epoch(8)),
            Err(ClaimError::SelfResolvedDispute { .. })
        ));
        let council = Actor::independent_reviewer("measurement-council");
        assert!(register
            .resolve_dispute(&id, &council, "split verified clean", Epoch(9))
            .is_ok());
        assert_eq!(register.status(&id), Some(&ClaimStatus::Published));
    }

    #[test]
    fn a_withdrawal_leaves_the_original_sentence_readable() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        register
            .withdraw(&id, &publisher(), "oracle defect", Epoch(9))
            .unwrap();
        assert_eq!(register.tombstones().len(), 1);
        assert_eq!(
            register.tombstones()[0].sentence.metric,
            "first-divergence-rate"
        );
        assert!(register.sentence(&id).is_some());
        assert_eq!(register.history(&id).unwrap().len(), 2);
    }

    #[test]
    fn a_withdrawn_claim_is_never_revived() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        register
            .withdraw(&id, &publisher(), "defect", Epoch(9))
            .unwrap();
        for outcome in [
            register.correct(&id, &publisher(), "typo", Epoch(10)),
            register.dispute(&id, &publisher(), "e", Epoch(10)),
            register.withdraw(&id, &publisher(), "again", Epoch(10)),
        ] {
            assert!(matches!(outcome, Err(ClaimError::ClaimWithdrawn { .. })));
        }
    }

    #[test]
    fn promotion_on_top_score_alone_or_sponsorship_is_refused() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        let hub = Actor::independent_reviewer("hub");
        for basis in [
            PromotionBasis::TopScoreAlone,
            PromotionBasis::SponsorRelationship,
        ] {
            assert!(matches!(
                register.promote(&id, &hub, basis, Epoch(7)),
                Err(ClaimError::PromotionBasisNotPermitted { .. })
            ));
        }
        assert!(register
            .promote(&id, &hub, PromotionBasis::DiagnosticSignificance, Epoch(7))
            .is_ok());
    }

    #[test]
    fn the_register_never_shrinks() {
        let mut register = ClaimRegister::new();
        let claim = public_sentence(ClaimClass::PublicBenchmarkResult, TrustTier::Reviewed)
            .issue(ClaimTier::PublicObservedWorlds, None, None, Epoch(6))
            .unwrap();
        let id = register.publish(claim).unwrap();
        register
            .withdraw(&id, &publisher(), "defect", Epoch(9))
            .unwrap();
        assert_eq!(register.len(), 1);
        let text = serde_json::to_string(&register).unwrap();
        let back: ClaimRegister = serde_json::from_str(&text).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back.tombstones().len(), 1);
    }

    #[test]
    fn every_class_demands_at_least_as_much_as_the_one_below_it() {
        let mut previous = ClaimClass::DescriptiveRun.requirements();
        for class in ClaimClass::ALL.into_iter().skip(1) {
            let current = class.requirements();
            assert!(current.min_claim_tier >= previous.min_claim_tier);
            assert!(current.min_pack_tier >= previous.min_pack_tier);
            previous = current;
        }
    }
}
