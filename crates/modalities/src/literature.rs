//! Literature and knowledge-base claims (28.17).
//!
//! A claim from a paper is evidence **about a paper**. That a result was reported is a fact about
//! the report; that it holds in some population is a different statement, and getting from the
//! first to the second is an act with conditions. This module models the act.
//!
//! # Two types, one direction
//!
//! [`LiteratureClaim`] is freely constructible and freely deserialisable: reading a paper is not a
//! privileged operation. [`BoundClaim`] has private fields, no public constructor and no
//! `Deserialize`, so the only way one comes into existence is [`LiteratureClaim::bind`] returning
//! `Ok`. The pattern is the one AGENTS.md records for `View` and `bioprism-oncoworlds` uses for
//! `PatientRelevantClaim`: where an invariant can be made unrepresentable, make it
//! unrepresentable, rather than testing that nobody violated it.
//!
//! The consequence that matters: **an unbound literature claim is not usable as a measurement**.
//! There is no `as_measurement`, no `into_modal_measurement` and no `From` impl on either type —
//! not on `LiteratureClaim`, and not on `BoundClaim` either, because binding a claim to a scope
//! does not turn a sentence into an observation. What binding produces is a claim that may be
//! *cited*, via [`cites`], and the literature descriptor in [`crate::catalog`] resolves no axes at
//! all, so [`crate::support::supports`] refuses every biological claim against it.
//!
//! # What `bind` checks, and in what order
//!
//! Retraction, then horizon, then evidence tier, then population. Retraction first because a
//! retracted source's other properties are not worth evaluating. The horizon next because
//! temporal leakage invalidates a historical task outright, where a tier or population problem
//! narrows what the claim supports. 28.16 asks the same of registry snapshots, so
//! [`EvaluationHorizon`] is used by both.
//!
//! # Deliberately not implemented
//!
//! - **No entailment checking.** 28.17's "claim drift: the cited result is narrower than the agent
//!   statement" needs a comparison between a restatement and a source span, which is natural
//!   language inference. [`BoundClaim::source_text`] retains the source's own words so a reader
//!   can do it; this crate does not.
//! - **No retrieval, no citation graph, no ranking.** Nothing here fetches a paper, walks
//!   references, or scores a venue. 28.17's "authority bias" is a property of a selection process,
//!   and this crate selects nothing.
//! - **No identifier resolution.** A DOI or PMID is an opaque string here. Resolving it would mean
//!   a network call, and the crate is offline and deterministic.

use crate::descriptor::Modality;
use crate::error::{BindingRefusal, Unsupported};
use crate::support::{supports, ClaimKind};
use bioprism_scope::{ScopeKey, Timestamp};
use serde::{Deserialize, Serialize};

/// Where a claim sits in the evidence hierarchy 28.17 asks agents to reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTier {
    /// The source reports the result it is being cited for.
    Primary,
    /// The source summarises results reported elsewhere.
    Review,
    /// The source states a recommendation synthesised from other sources.
    Guideline,
    /// The source is a curated database record.
    ///
    /// Kept separate from [`EvidenceTier::Primary`] because a database entry is a curator's
    /// reading of a primary source, and the curation step is where 28.19's version drift enters.
    Database,
}

impl EvidenceTier {
    pub fn as_str(self) -> &'static str {
        match self {
            EvidenceTier::Primary => "primary",
            EvidenceTier::Review => "review",
            EvidenceTier::Guideline => "guideline",
            EvidenceTier::Database => "database record",
        }
    }

    /// True only for [`EvidenceTier::Primary`].
    ///
    /// The one-line statement of 28.17's "citation laundering": everything else is a report of a
    /// report, and binding it as though it were the report itself loses the distinction.
    pub fn is_direct_evidence(self) -> bool {
        matches!(self, EvidenceTier::Primary)
    }
}

/// Whether a source has been withdrawn or flagged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetractionStatus {
    #[default]
    None,
    ExpressionOfConcern,
    Retracted,
}

impl RetractionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            RetractionStatus::None => "not flagged",
            RetractionStatus::ExpressionOfConcern => "under an expression of concern",
            RetractionStatus::Retracted => "retracted",
        }
    }

    pub fn is_flagged(self) -> bool {
        !matches!(self, RetractionStatus::None)
    }
}

/// Everything about the source that a binding decision depends on.
///
/// The `population` is a [`ScopeKey`] rather than prose, so that "does the target sit inside what
/// this paper studied" is [`ScopeKey::refines`] rather than a judgement call. `None` means the
/// source did not state one, which is a refusal rather than a permissive default: without a stated
/// population, 28.17's "population mismatch" is unfalsifiable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceProvenance {
    /// A DOI, PMID, accession or other locator. Opaque to this crate.
    pub identifier: String,
    pub tier: EvidenceTier,
    pub published: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub population: Option<ScopeKey>,
    #[serde(default)]
    pub retraction: RetractionStatus,
}

impl SourceProvenance {
    pub fn new(identifier: impl Into<String>, tier: EvidenceTier, published: Timestamp) -> Self {
        SourceProvenance {
            identifier: identifier.into(),
            tier,
            published,
            population: None,
            retraction: RetractionStatus::None,
        }
    }

    pub fn studying(mut self, population: ScopeKey) -> Self {
        self.population = Some(population);
        self
    }

    pub fn flagged(mut self, status: RetractionStatus) -> Self {
        self.retraction = status;
        self
    }
}

/// The date after which a source may not be used.
///
/// 28.17 names "temporal leakage: later discoveries are used in historical rediscovery" and 28.16
/// names "stale status: current registry information contaminates historical tasks". They are the
/// same check and share this type. [`EvaluationHorizon::open`] exists for tasks that are not
/// historical, and is a declaration rather than a default — there is no `Default` impl, because
/// defaulting to open would turn "nobody set a horizon" into "no horizon applies".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "horizon", rename_all = "snake_case")]
pub enum EvaluationHorizon {
    /// Sources published after this instant are refused.
    AsOf { instant: Timestamp },
    /// No horizon applies, stated explicitly.
    Open,
}

impl EvaluationHorizon {
    pub fn as_of(instant: Timestamp) -> Self {
        EvaluationHorizon::AsOf { instant }
    }

    pub fn open() -> Self {
        EvaluationHorizon::Open
    }

    pub fn admits(&self, published: Timestamp) -> bool {
        match self {
            EvaluationHorizon::AsOf { instant } => published <= *instant,
            EvaluationHorizon::Open => true,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            EvaluationHorizon::AsOf { instant } => instant.to_rfc3339(),
            EvaluationHorizon::Open => "open".to_string(),
        }
    }
}

/// An assertion made in a source, together with where it was made.
///
/// Freely constructible and round-trippable. Nothing about holding one of these is a claim about
/// the world; the type's whole content is "this document says this".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LiteratureClaim {
    /// The source's own words, retained verbatim.
    pub text: String,
    pub provenance: SourceProvenance,
}

impl LiteratureClaim {
    pub fn new(text: impl Into<String>, provenance: SourceProvenance) -> Self {
        LiteratureClaim {
            text: text.into(),
            provenance,
        }
    }

    /// Binds the claim to a target scope, producing something that may be cited.
    ///
    /// `at_tier` is the tier the caller wants to cite the claim *as*. Asking to cite a review as
    /// primary evidence is the refusal; asking to cite it as a review is fine, and the resulting
    /// [`BoundClaim`] remembers which.
    pub fn bind(
        &self,
        target: &ScopeKey,
        at_tier: EvidenceTier,
        horizon: EvaluationHorizon,
    ) -> Result<BoundClaim, BindingRefusal> {
        if self.provenance.retraction.is_flagged() {
            return Err(BindingRefusal::RetractedSource {
                identifier: self.provenance.identifier.clone(),
                status: self.provenance.retraction.as_str().to_string(),
            });
        }
        self.bind_flagged_source(target, at_tier, horizon)
    }

    /// [`LiteratureClaim::bind`] for a flagged source, with the warrant recorded.
    ///
    /// A retracted paper is still citable — for a claim about the retraction, or in a history of a
    /// field — and refusing outright would make that unrepresentable. What is not allowed is
    /// citing one *silently*, so the warrant is required and travels on the bound claim.
    pub fn bind_despite_flag(
        &self,
        target: &ScopeKey,
        at_tier: EvidenceTier,
        horizon: EvaluationHorizon,
        warrant: impl Into<String>,
    ) -> Result<BoundClaim, BindingRefusal> {
        let warrant = warrant.into();
        let mut bound = self.bind_flagged_source(target, at_tier, horizon)?;
        bound.flag_warrant = Some(warrant);
        Ok(bound)
    }

    fn bind_flagged_source(
        &self,
        target: &ScopeKey,
        at_tier: EvidenceTier,
        horizon: EvaluationHorizon,
    ) -> Result<BoundClaim, BindingRefusal> {
        if !horizon.admits(self.provenance.published) {
            return Err(BindingRefusal::TemporalLeakage {
                identifier: self.provenance.identifier.clone(),
                published: self.provenance.published.to_rfc3339(),
                horizon: horizon.describe(),
            });
        }
        if at_tier.is_direct_evidence() && !self.provenance.tier.is_direct_evidence() {
            return Err(BindingRefusal::CitationLaundering {
                identifier: self.provenance.identifier.clone(),
                tier: self.provenance.tier.as_str().to_string(),
            });
        }
        let Some(population) = &self.provenance.population else {
            return Err(BindingRefusal::UnstatedPopulation {
                identifier: self.provenance.identifier.clone(),
            });
        };
        if !target.refines(population) {
            return Err(BindingRefusal::PopulationMismatch {
                identifier: self.provenance.identifier.clone(),
                population: describe_scope(population),
            });
        }
        Ok(BoundClaim {
            source_text: self.text.clone(),
            provenance: self.provenance.clone(),
            cited_as: at_tier,
            scope: target.clone(),
            horizon,
            flag_warrant: None,
        })
    }
}

/// A literature claim that has been bound to a scope it may be cited in.
///
/// Private fields, `Serialize` only, no public constructor. The only route in is
/// [`LiteratureClaim::bind`], which is why holding one of these carries information: it means the
/// four checks passed. Deserialising one would let a caller mint the conclusion without the
/// premises, so the impl is absent.
///
/// Being bound still does not make it a measurement. It makes it a claim with a stated scope,
/// stated provenance and a stated tier, which is what [`cites`] will accept and what
/// [`crate::support::supports`] will still refuse for every biological claim kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BoundClaim {
    source_text: String,
    provenance: SourceProvenance,
    cited_as: EvidenceTier,
    scope: ScopeKey,
    horizon: EvaluationHorizon,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_warrant: Option<String>,
}

impl BoundClaim {
    /// The source's own words. Retained so a reader can check a restatement against them.
    pub fn source_text(&self) -> &str {
        &self.source_text
    }

    pub fn provenance(&self) -> &SourceProvenance {
        &self.provenance
    }

    /// The tier the claim was bound at, which may be weaker than the source's own tier.
    pub fn cited_as(&self) -> EvidenceTier {
        self.cited_as
    }

    pub fn scope(&self) -> &ScopeKey {
        &self.scope
    }

    pub fn horizon(&self) -> EvaluationHorizon {
        self.horizon
    }

    /// The warrant for citing a flagged source, when one was given.
    pub fn flag_warrant(&self) -> Option<&str> {
        self.flag_warrant.as_deref()
    }

    /// True when the claim was bound at the primary tier from a primary source.
    pub fn is_direct_evidence(&self) -> bool {
        self.cited_as.is_direct_evidence() && self.provenance.tier.is_direct_evidence()
    }
}

/// Whether a bound claim may be cited in support of a claim of this kind, and at what strength.
///
/// Delegates to [`crate::support::supports`] against the 28.17 descriptor, which resolves no axes,
/// so everything except [`ClaimKind::PublishedClaimSupport`] is refused. The delegation is the
/// point: a bound literature claim does not get a private permission system, it goes through the
/// same support relation as every other modality and loses.
///
/// The success value is the tier the claim was bound at rather than `()`, because "this may be
/// cited" and "this may be cited as primary evidence" are different permissions and a caller that
/// only learns the first will spend the second.
pub fn cites(claim: &BoundClaim, kind: ClaimKind) -> Result<EvidenceTier, Unsupported> {
    supports(Modality::Literature, kind)?;
    Ok(claim.cited_as())
}

fn describe_scope(scope: &ScopeKey) -> String {
    let bindings: Vec<String> = scope
        .iter()
        .map(|(dimension, value)| format!("{dimension}={}", value.describe()))
        .collect();
    if bindings.is_empty() {
        "an unconstrained scope".to_string()
    } else {
        bindings.join(", ")
    }
}
