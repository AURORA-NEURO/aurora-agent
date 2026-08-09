//! The observable contract of an agent-facing component.
//!
//! Blueprint 23.41's tuple `A = ⟨Iin, Iout, E, G, K, B, Q, F⟩`, with 23.11's behavioural interface
//! folded into the failure and assurance components. This module is data and refinement relations
//! only; the operators over it are [`crate::algebra`].
//!
//! 23.41: "Composition is defined over contracts, not personalities." Nothing here names a model,
//! a vendor or a prompt. An implementation may be a model call, a human, a tool or a nested
//! molecule, and this crate cannot tell which, which is the property that makes substitution
//! decidable at all.
//!
//! # Variance, which is the part everyone gets backwards
//!
//! For `B` to replace `A`, `B` must accept everything `A` accepted and promise everything `A`
//! promised. Inputs are therefore **contravariant** and outputs **covariant**:
//!
//! ```text
//! B.Iin  accepts A.Iin   ⟺  A.Iin <: B.Iin     (B demands no more)
//! B.Iout refines A.Iout  ⟺  B.Iout <: A.Iout   (B delivers no less)
//! ```
//!
//! [`InterfaceType::subtypes`] is the single `<:` used in both directions, so the two clauses
//! cannot drift apart.
//!
//! # Not implemented
//!
//! No values, so no type checking of actual payloads. [`InterfaceType`] is a structural record
//! shape and nothing constructs an inhabitant of one. No refinement predicates: 23.04's
//! `where provenance.complete and source.independent_count >= 2` is out of reach here for the same
//! reason `bioprism-weavelang` gives — the syntax is never specified. No latency or cost
//! *measurement*: [`ResourceEnvelope`] holds declared ceilings, and this crate has no clock with
//! which to observe a violation.

use crate::effect::{EffectSet, Inclusion, Irreversibility};
use bioprism_weave::Capability;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A stable identifier for a component in a composition. Not an agent identity; see
/// [`crate::reputation::IdentityLayers`] for that distinction.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ComponentId(pub String);

impl ComponentId {
    pub fn new(id: impl Into<String>) -> Self {
        ComponentId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ComponentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A field's type in a structural interface.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "of")]
pub enum FieldType {
    Text,
    Number,
    Bool,
    Bytes,
    /// A named nominal type, compared by name. Structural comparison would need a schema resolver,
    /// which this crate does not have and does not pretend to.
    Named(String),
    Optional(Box<FieldType>),
    List(Box<FieldType>),
}

impl FieldType {
    /// Depth subtyping. `T <: Optional<T>` because a value that is always present satisfies a
    /// consumer prepared for absence; the converse fails.
    pub fn subtypes(&self, other: &FieldType) -> bool {
        match (self, other) {
            (a, FieldType::Optional(b)) if a.subtypes(b) => true,
            (FieldType::Optional(a), FieldType::Optional(b)) => a.subtypes(b),
            (FieldType::List(a), FieldType::List(b)) => a.subtypes(b),
            (a, b) => a == b,
        }
    }
}

/// A structural record type. `Iin` and `Iout` of the contract tuple.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub name: String,
    pub fields: BTreeMap<String, FieldType>,
}

impl InterfaceType {
    pub fn new(name: impl Into<String>) -> Self {
        InterfaceType {
            name: name.into(),
            fields: BTreeMap::new(),
        }
    }

    pub fn field(mut self, name: impl Into<String>, ty: FieldType) -> Self {
        self.fields.insert(name.into(), ty);
        self
    }

    /// Width and depth subtyping: `self <: other` when `self` has every field `other` has, at a
    /// subtype. The record *name* is not compared, because 23.41 says the substitution relation is
    /// "contextual refinement, not name or schema equality".
    pub fn subtypes(&self, other: &InterfaceType) -> bool {
        other.fields.iter().all(|(name, required)| {
            self.fields
                .get(name)
                .map(|mine| mine.subtypes(required))
                .unwrap_or(false)
        })
    }

    /// Fields `other` requires that `self` lacks or supplies at an incompatible type.
    pub fn missing_against(&self, other: &InterfaceType) -> Vec<String> {
        other
            .fields
            .iter()
            .filter(|(name, required)| {
                !self
                    .fields
                    .get(*name)
                    .map(|mine| mine.subtypes(required))
                    .unwrap_or(false)
            })
            .map(|(name, _)| name.clone())
            .collect()
    }
}

/// How a component reports what it does not know. `K`'s "uncertainty semantics".
///
/// Ordered, and the order is the substitution direction: a replacement may report more finely than
/// the original, never more coarsely. 23.41's worked example is exactly this — "a cheaper model
/// with the same JSON output is not substitutable if it is less calibrated".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintySemantics {
    /// A bare answer. No indication of confidence at all.
    PointEstimate,
    /// A qualitative confidence with no calibration evidence behind it.
    SelfReportedConfidence,
    /// An interval whose coverage nobody has measured.
    UncalibratedInterval,
    /// An interval with measured coverage.
    Calibrated,
}

/// `K` of the contract tuple: evidence consumed, claims emitted, uncertainty semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicContract {
    pub evidence_consumed: BTreeSet<String>,
    pub claims_emitted: BTreeSet<String>,
    pub uncertainty: UncertaintySemantics,
    /// Whether the component will decline rather than guess. A *guarantee*, so a replacement that
    /// does not abstain cannot substitute for one that does.
    pub abstains: bool,
    /// Whether every claim carries its evidence lineage. 23.11's `provenance.complete`.
    pub provenance_complete: bool,
}

impl EpistemicContract {
    pub fn new(uncertainty: UncertaintySemantics) -> Self {
        EpistemicContract {
            evidence_consumed: BTreeSet::new(),
            claims_emitted: BTreeSet::new(),
            uncertainty,
            abstains: false,
            provenance_complete: false,
        }
    }

    pub fn consuming(mut self, evidence: impl Into<String>) -> Self {
        self.evidence_consumed.insert(evidence.into());
        self
    }

    pub fn emitting(mut self, claim: impl Into<String>) -> Self {
        self.claims_emitted.insert(claim.into());
        self
    }

    pub fn abstaining(mut self) -> Self {
        self.abstains = true;
        self
    }

    pub fn with_complete_provenance(mut self) -> Self {
        self.provenance_complete = true;
        self
    }

    /// Whether `self` may stand in for `other` epistemically.
    pub fn refines(&self, other: &EpistemicContract) -> Vec<EpistemicShortfall> {
        let mut out = Vec::new();
        let omitted: BTreeSet<String> = other
            .claims_emitted
            .difference(&self.claims_emitted)
            .cloned()
            .collect();
        if !omitted.is_empty() {
            out.push(EpistemicShortfall::ClaimsOmitted { claims: omitted });
        }
        if self.uncertainty < other.uncertainty {
            out.push(EpistemicShortfall::LessCalibrated {
                required: other.uncertainty,
                offered: self.uncertainty,
            });
        }
        if other.abstains && !self.abstains {
            out.push(EpistemicShortfall::DoesNotAbstain);
        }
        if other.provenance_complete && !self.provenance_complete {
            out.push(EpistemicShortfall::ProvenanceIncomplete);
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "shortfall")]
pub enum EpistemicShortfall {
    ClaimsOmitted {
        claims: BTreeSet<String>,
    },
    LessCalibrated {
        required: UncertaintySemantics,
        offered: UncertaintySemantics,
    },
    DoesNotAbstain,
    ProvenanceIncomplete,
}

/// `B` of the contract tuple: the declared resource envelope.
///
/// Every field is a *declaration*. This crate never observes a run, so an envelope is a promise
/// compared against another promise, and a composition that satisfies every declared envelope may
/// still overrun. Said here because a type named `ResourceEnvelope` invites the opposite reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ResourceEnvelope {
    pub max_tokens: u64,
    pub max_tool_calls: u64,
    /// Declared upper bound in whatever unit the caller uses consistently. Not seconds observed.
    pub declared_latency_units: u64,
    /// Declared upper bound in minor currency units, kept as an integer so envelope arithmetic is
    /// exact and two runs cannot disagree by a rounding step.
    pub declared_cost_minor: u64,
}

impl ResourceEnvelope {
    pub fn new() -> Self {
        ResourceEnvelope {
            max_tokens: 0,
            max_tool_calls: 0,
            declared_latency_units: 0,
            declared_cost_minor: 0,
        }
    }

    pub fn tokens(mut self, value: u64) -> Self {
        self.max_tokens = value;
        self
    }

    pub fn tool_calls(mut self, value: u64) -> Self {
        self.max_tool_calls = value;
        self
    }

    pub fn latency(mut self, value: u64) -> Self {
        self.declared_latency_units = value;
        self
    }

    pub fn cost(mut self, value: u64) -> Self {
        self.declared_cost_minor = value;
        self
    }

    /// Whether `self` fits inside `allowed` on every axis.
    pub fn within(&self, allowed: &ResourceEnvelope) -> Vec<EnvelopeOverrun> {
        let mut out = Vec::new();
        if self.max_tokens > allowed.max_tokens {
            out.push(EnvelopeOverrun::Tokens {
                declared: self.max_tokens,
                allowed: allowed.max_tokens,
            });
        }
        if self.max_tool_calls > allowed.max_tool_calls {
            out.push(EnvelopeOverrun::ToolCalls {
                declared: self.max_tool_calls,
                allowed: allowed.max_tool_calls,
            });
        }
        if self.declared_latency_units > allowed.declared_latency_units {
            out.push(EnvelopeOverrun::Latency {
                declared: self.declared_latency_units,
                allowed: allowed.declared_latency_units,
            });
        }
        if self.declared_cost_minor > allowed.declared_cost_minor {
            out.push(EnvelopeOverrun::Cost {
                declared: self.declared_cost_minor,
                allowed: allowed.declared_cost_minor,
            });
        }
        out
    }

    /// The envelope of two components run one after the other.
    pub fn sum(&self, other: &ResourceEnvelope) -> ResourceEnvelope {
        ResourceEnvelope {
            max_tokens: self.max_tokens.saturating_add(other.max_tokens),
            max_tool_calls: self.max_tool_calls.saturating_add(other.max_tool_calls),
            declared_latency_units: self
                .declared_latency_units
                .saturating_add(other.declared_latency_units),
            declared_cost_minor: self
                .declared_cost_minor
                .saturating_add(other.declared_cost_minor),
        }
    }

    /// The envelope of two components run concurrently: resources add, declared latency is the
    /// larger of the two. Declared, not observed — there is no scheduler here to overlap anything.
    pub fn parallel(&self, other: &ResourceEnvelope) -> ResourceEnvelope {
        ResourceEnvelope {
            declared_latency_units: self.declared_latency_units.max(other.declared_latency_units),
            ..self.sum(other)
        }
    }
}

impl Default for ResourceEnvelope {
    fn default() -> Self {
        ResourceEnvelope::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "overrun")]
pub enum EnvelopeOverrun {
    Tokens { declared: u64, allowed: u64 },
    ToolCalls { declared: u64, allowed: u64 },
    Latency { declared: u64, allowed: u64 },
    Cost { declared: u64, allowed: u64 },
}

/// `Q` of the contract tuple: the verified capability and assurance profile.
///
/// The `verified_at` rung is [`crate::reputation::EvidenceLayer`], reused rather than redefined so
/// a contract's assurance and a registry's capability card cannot disagree about what "verified"
/// means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceProfile {
    pub verified_at: crate::reputation::EvidenceLayer,
    /// A lower confidence bound on success, in basis points. Integer so comparisons are exact.
    ///
    /// `None` is **unmeasured**, categorically distinct from a measured zero. Same rule as
    /// `bioprism-atlas`: a component nobody evaluated must not render as a component that failed.
    pub success_lower_bound_bp: Option<u32>,
    pub shielded_by: Option<String>,
}

impl AssuranceProfile {
    pub fn at(verified_at: crate::reputation::EvidenceLayer) -> Self {
        AssuranceProfile {
            verified_at,
            success_lower_bound_bp: None,
            shielded_by: None,
        }
    }

    pub fn with_lower_bound_bp(mut self, bp: u32) -> Self {
        self.success_lower_bound_bp = Some(bp);
        self
    }

    /// Whether `self` meets a minimum. An unmeasured bound never meets a stated minimum; it is not
    /// treated as zero and it is not treated as passing.
    pub fn meets(&self, minimum: &AssuranceProfile) -> Vec<AssuranceShortfall> {
        let mut out = Vec::new();
        if self.verified_at < minimum.verified_at {
            out.push(AssuranceShortfall::LadderRungTooLow {
                required: minimum.verified_at,
                offered: self.verified_at,
            });
        }
        match (self.success_lower_bound_bp, minimum.success_lower_bound_bp) {
            (_, None) => {}
            (None, Some(required)) => out.push(AssuranceShortfall::Unmeasured { required }),
            (Some(mine), Some(required)) if mine < required => {
                out.push(AssuranceShortfall::BelowLowerBound {
                    required,
                    offered: mine,
                })
            }
            (Some(_), Some(_)) => {}
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "shortfall")]
pub enum AssuranceShortfall {
    LadderRungTooLow {
        required: crate::reputation::EvidenceLayer,
        offered: crate::reputation::EvidenceLayer,
    },
    /// No evidence exists. Not a low score.
    Unmeasured {
        required: u32,
    },
    BelowLowerBound {
        required: u32,
        offered: u32,
    },
}

/// What a component does with work already done when it fails partway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PartialResultPolicy {
    /// Failure yields nothing.
    Discard,
    /// Failure yields what was completed, unmarked.
    ReturnUnmarked,
    /// Failure yields what was completed, marked as partial with the missing parts named.
    ReturnMarked,
}

/// `F` of the contract tuple: failure, cancellation and compensation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FailureContract {
    pub cancellable: bool,
    pub partial_results: PartialResultPolicy,
    /// Effects this component can undo. An effect it performs and cannot compensate is what makes
    /// 23.41's associativity law fail.
    pub compensatable: EffectSet,
    /// Whether the component's behaviour depends on a deadline. The other associativity blocker.
    pub deadline_sensitive: bool,
}

impl FailureContract {
    pub fn new() -> Self {
        FailureContract {
            cancellable: false,
            partial_results: PartialResultPolicy::Discard,
            compensatable: EffectSet::new(),
            deadline_sensitive: false,
        }
    }

    pub fn cancellable(mut self) -> Self {
        self.cancellable = true;
        self
    }

    pub fn returning(mut self, policy: PartialResultPolicy) -> Self {
        self.partial_results = policy;
        self
    }

    pub fn compensating(mut self, effects: EffectSet) -> Self {
        self.compensatable = effects;
        self
    }

    pub fn deadline_sensitive(mut self) -> Self {
        self.deadline_sensitive = true;
        self
    }

    /// Whether `self` preserves the failure semantics `other` promised.
    pub fn preserves(&self, other: &FailureContract) -> Vec<FailureShortfall> {
        let mut out = Vec::new();
        if other.cancellable && !self.cancellable {
            out.push(FailureShortfall::NotCancellable);
        }
        if self.partial_results < other.partial_results {
            out.push(FailureShortfall::WeakerPartialResults {
                required: other.partial_results,
                offered: self.partial_results,
            });
        }
        if let Inclusion::Fails { witnesses } | Inclusion::Undecided { witnesses } =
            self.compensatable.includes(&other.compensatable)
        {
            out.push(FailureShortfall::CompensationNarrowed {
                uncompensated: witnesses,
            });
        }
        out
    }
}

impl Default for FailureContract {
    fn default() -> Self {
        FailureContract::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "shortfall")]
pub enum FailureShortfall {
    NotCancellable,
    WeakerPartialResults {
        required: PartialResultPolicy,
        offered: PartialResultPolicy,
    },
    CompensationNarrowed {
        uncompensated: Vec<crate::effect::Effect>,
    },
}

/// A commitment a component holds open, in 23.41's "commitment conservation" sense.
///
/// Not `bioprism_weave::Commitment`: the kernel's is a ledger record with a lifecycle, this is the
/// *static* obligation a contract declares it will take on, which is what a composition has to
/// account for before anything runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct DeclaredCommitment {
    pub id: String,
    pub mandatory: bool,
}

impl DeclaredCommitment {
    pub fn mandatory(id: impl Into<String>) -> Self {
        DeclaredCommitment {
            id: id.into(),
            mandatory: true,
        }
    }

    pub fn discretionary(id: impl Into<String>) -> Self {
        DeclaredCommitment {
            id: id.into(),
            mandatory: false,
        }
    }
}

/// `A = ⟨Iin, Iout, E, G, K, B, Q, F⟩`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentContract {
    pub id: ComponentId,
    pub input: InterfaceType,
    pub output: InterfaceType,
    pub effects: EffectSet,
    pub authority: BTreeSet<Capability>,
    pub epistemic: EpistemicContract,
    pub envelope: ResourceEnvelope,
    pub assurance: AssuranceProfile,
    pub failure: FailureContract,
    pub commitments: BTreeSet<DeclaredCommitment>,
    /// The information-flow label of what this component emits. Unlabelled by default, and
    /// unlabelled is not public.
    pub output_labelling: crate::flow::Labelling,
}

impl AgentContract {
    pub fn new(
        id: impl Into<String>,
        input: InterfaceType,
        output: InterfaceType,
        epistemic: EpistemicContract,
        assurance: AssuranceProfile,
    ) -> Self {
        AgentContract {
            id: ComponentId::new(id),
            input,
            output,
            effects: EffectSet::new(),
            authority: BTreeSet::new(),
            epistemic,
            envelope: ResourceEnvelope::new(),
            assurance,
            failure: FailureContract::new(),
            commitments: BTreeSet::new(),
            output_labelling: crate::flow::Labelling::Unlabelled,
        }
    }

    pub fn with_effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub fn with_authority(mut self, authority: impl IntoIterator<Item = Capability>) -> Self {
        self.authority = authority.into_iter().collect();
        self
    }

    pub fn with_envelope(mut self, envelope: ResourceEnvelope) -> Self {
        self.envelope = envelope;
        self
    }

    pub fn with_failure(mut self, failure: FailureContract) -> Self {
        self.failure = failure;
        self
    }

    pub fn committing(mut self, commitment: DeclaredCommitment) -> Self {
        self.commitments.insert(commitment);
        self
    }

    pub fn emitting_at(mut self, labelling: crate::flow::Labelling) -> Self {
        self.output_labelling = labelling;
        self
    }

    /// The highest irreversibility class this component may reach.
    pub fn peak_class(&self) -> Irreversibility {
        self.effects.peak_class()
    }

    /// Mandatory commitments, which are the ones 23.41's conservation law is about.
    pub fn mandatory_commitments(&self) -> BTreeSet<DeclaredCommitment> {
        self.commitments
            .iter()
            .filter(|c| c.mandatory)
            .cloned()
            .collect()
    }

    /// An effect this component performs and cannot undo.
    pub fn uncompensated_effects(&self) -> Vec<crate::effect::Effect> {
        self.effects.escalation_over(&self.failure.compensatable)
    }
}

/// The identity component `1_T` of 23.41.
///
/// Constructed rather than declared, because an identity that quietly carries an effect, an
/// authority or a commitment is not an identity and the law that names it would be false. The one
/// thing a caller may vary is whether the identity *relabels* its output, which is exactly the
/// condition 23.41 says breaks the law, so [`identity_relabelling`] exists to make the failing
/// case constructible.
pub fn identity(ty: &InterfaceType) -> AgentContract {
    AgentContract {
        id: ComponentId::new(format!("1_{}", ty.name)),
        input: ty.clone(),
        output: ty.clone(),
        effects: EffectSet::new(),
        authority: BTreeSet::new(),
        epistemic: EpistemicContract::new(UncertaintySemantics::Calibrated)
            .with_complete_provenance(),
        envelope: ResourceEnvelope::new(),
        assurance: AssuranceProfile::at(crate::reputation::EvidenceLayer::IndependentlyAttested),
        failure: FailureContract::new().cancellable(),
        commitments: BTreeSet::new(),
        output_labelling: crate::flow::Labelling::Unlabelled,
    }
}

/// An identity-shaped component that changes the security label of what passes through it.
///
/// 23.41: the identity equivalence "fails if the identity changes provenance, budgets, time, or
/// security labels". A pure identity cannot express that failure, so this is how a test gets one.
pub fn identity_relabelling(ty: &InterfaceType, labelling: crate::flow::Labelling) -> AgentContract {
    AgentContract {
        output_labelling: labelling,
        ..identity(ty)
    }
}
