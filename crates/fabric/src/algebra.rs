//! The agent composition algebra and its substitution laws.
//!
//! Blueprint 23.41. Ten operators over [`crate::contract::AgentContract`], a substitution relation,
//! seven conditional laws, and eight separately-reported equivalence dimensions.
//!
//! # Why the laws are functions and not sentences
//!
//! 23.41 states its laws in prose with the conditions attached: associativity "holds only when
//! continuation boundaries, compensation scopes, and budget reservations are observationally
//! equivalent", parallel commutativity "holds only for causally independent or explicitly
//! commutative effects". A law with a condition is a predicate over contracts, and prose cannot be
//! run against a candidate composition. Each `check_*` here takes the contracts the law quantifies
//! over and returns a [`LawReport`] naming either that it holds or which side condition failed and
//! on what. That is the difference between documenting the algebra and having one.
//!
//! Two of the checks deliberately can *fail*, and their failing cases are the interesting tests:
//! reassociating around an uncompensated `E4` effect, and parallelising two components whose write
//! sets overlap.
//!
//! # The substitution rule, in one line
//!
//! Effects a subset, guarantees a superset. [`substitutable`] is that sentence expanded into
//! 23.41's seven clauses, and every clause that fails is named in the verdict. `bioprism-weavelang`
//! already enforces the effect direction at compile time over a *program*
//! (`effects(lower(P)) ⊆ declared_effects(P)`); what is added here is the same direction over a
//! *participant*, plus the six clauses that are not about effects at all — because 23.41's own
//! worked counterexample is a replacement whose effects are identical and which is still not
//! substitutable: "A cheaper model with the same JSON output is not substitutable if it is less
//! calibrated, needs broader authority, or omits evidence."
//!
//! # What is deliberately absent
//!
//! **There is no `fn same_agent(a, b) -> bool`.** 23.41: "A single boolean 'same agent' is
//! inadequate." [`EquivalenceReport`] reports eight dimensions and offers no method that collapses
//! them, not even a private one.
//!
//! No execution. An operator here builds a *contract for the composite*; it does not run anything,
//! there is no scheduler, `par` does not interleave and `race` does not race. The affine law runs a
//! real `bioprism_weave::Budget` rather than reimplementing non-duplication, so there is one
//! implementation of that rule in the workspace and this module is a caller of it.

use crate::contract::{
    AgentContract, ComponentId, DeclaredCommitment, EnvelopeOverrun, EpistemicContract,
    EpistemicShortfall, FailureShortfall, InterfaceType, ResourceEnvelope,
};
use crate::effect::{Effect, EffectSet, Inclusion};
use crate::flow::Labelling;
use bioprism_choreography::WellFormedGlobal;
use bioprism_weave::{Budget, Capability, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Which operator produced a composite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "operator")]
pub enum Operator {
    /// `A ▷ B`
    Sequential,
    /// `A ∥ B`
    Parallel { justification: ParallelJustification },
    /// `A ⊗χ B`
    ChoreographedFusion { choreography_digest: String },
    /// `A ⊕r B`
    PolicyChoice { router: String },
    /// `race_v(A, B, ...)`
    VerifiedRace { verifier: String },
    /// `jury_q({A1 ... An})`
    Jury { policy: String },
    /// `A ⋉g B`
    AttenuatingDelegation { grant: BTreeSet<Capability> },
    /// `shield_p(A)`
    Shield { monitor: String },
    /// `checkpoint_c(A)`
    Checkpoint { label: String },
    /// `fallback_f(A, B)`
    Fallback { predicate: String },
}

/// Why parallel composition is legal for a given pair. 23.41 allows exactly three grounds:
/// disjoint write sets, commutativity, or a declared merge contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "justification")]
pub enum ParallelJustification {
    DisjointWriteSets,
    /// The caller asserts the writes commute. An assertion, recorded as one: 23.41's own warning is
    /// that "two agents writing the same repository branch are not commutative merely because both
    /// return patches", so this variant names who asserted it.
    DeclaredCommutative { asserted_by: String },
    MergeContract { contract: String },
}

/// A composite: the operator, its parts, and the contract the whole exposes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composition {
    pub operator: Operator,
    pub parts: Vec<AgentContract>,
    pub contract: AgentContract,
}

fn merge_epistemic(a: &EpistemicContract, b: &EpistemicContract) -> EpistemicContract {
    EpistemicContract {
        evidence_consumed: a
            .evidence_consumed
            .union(&b.evidence_consumed)
            .cloned()
            .collect(),
        claims_emitted: a.claims_emitted.union(&b.claims_emitted).cloned().collect(),
        uncertainty: a.uncertainty.min(b.uncertainty),
        abstains: a.abstains && b.abstains,
        provenance_complete: a.provenance_complete && b.provenance_complete,
    }
}

fn merge_assurance(
    a: &crate::contract::AssuranceProfile,
    b: &crate::contract::AssuranceProfile,
) -> crate::contract::AssuranceProfile {
    crate::contract::AssuranceProfile {
        verified_at: a.verified_at.min(b.verified_at),
        success_lower_bound_bp: match (a.success_lower_bound_bp, b.success_lower_bound_bp) {
            (Some(x), Some(y)) => Some(x.min(y)),
            _ => None,
        },
        shielded_by: a.shielded_by.clone().or_else(|| b.shielded_by.clone()),
    }
}

fn merge_failure(
    a: &crate::contract::FailureContract,
    b: &crate::contract::FailureContract,
) -> crate::contract::FailureContract {
    crate::contract::FailureContract {
        cancellable: a.cancellable && b.cancellable,
        partial_results: a.partial_results.min(b.partial_results),
        compensatable: a.compensatable.union(&b.compensatable),
        deadline_sensitive: a.deadline_sensitive || b.deadline_sensitive,
    }
}

fn union_commitments(
    a: &BTreeSet<DeclaredCommitment>,
    b: &BTreeSet<DeclaredCommitment>,
) -> BTreeSet<DeclaredCommitment> {
    a.union(b).cloned().collect()
}

/// `A ▷ B`: sequential continuation.
///
/// Legal only when `A.Iout` satisfies `B.Iin`. Commitments are carried into the composite rather
/// than dropped, which is the static half of the conservation law; the dynamic half is
/// [`check_commitment_conservation`].
pub fn seq(a: &AgentContract, b: &AgentContract) -> Result<Composition, CompositionError> {
    if !a.output.subtypes(&b.input) {
        return Err(CompositionError::InterfaceMismatch {
            producer: a.id.clone(),
            consumer: b.id.clone(),
            missing: a.output.missing_against(&b.input),
        });
    }
    let contract = AgentContract {
        id: ComponentId::new(format!("({} > {})", a.id.as_str(), b.id.as_str())),
        input: a.input.clone(),
        output: b.output.clone(),
        effects: a.effects.union(&b.effects),
        authority: a.authority.union(&b.authority).cloned().collect(),
        epistemic: merge_epistemic(&a.epistemic, &b.epistemic),
        envelope: a.envelope.sum(&b.envelope),
        assurance: merge_assurance(&a.assurance, &b.assurance),
        failure: merge_failure(&a.failure, &b.failure),
        commitments: union_commitments(&a.commitments, &b.commitments),
        output_labelling: a.output_labelling.join(&b.output_labelling),
    };
    Ok(Composition {
        operator: Operator::Sequential,
        parts: vec![a.clone(), b.clone()],
        contract,
    })
}

/// `A ∥ B`: independent parallelism.
///
/// Refuses overlapping write sets unless the caller supplies a justification. The refusal names the
/// overlapping effects, because "your writes conflict" without saying which is not actionable.
pub fn par(
    a: &AgentContract,
    b: &AgentContract,
    justification: ParallelJustification,
) -> Result<Composition, CompositionError> {
    let overlap = write_set_overlap(a, b);
    if !overlap.is_empty() && justification == ParallelJustification::DisjointWriteSets {
        return Err(CompositionError::WriteSetsOverlap {
            left: a.id.clone(),
            right: b.id.clone(),
            overlap,
        });
    }
    let output = InterfaceType {
        name: format!("({} || {})", a.output.name, b.output.name),
        fields: a
            .output
            .fields
            .iter()
            .map(|(k, v)| (format!("left.{k}"), v.clone()))
            .chain(
                b.output
                    .fields
                    .iter()
                    .map(|(k, v)| (format!("right.{k}"), v.clone())),
            )
            .collect(),
    };
    let contract = AgentContract {
        id: ComponentId::new(format!("({} || {})", a.id.as_str(), b.id.as_str())),
        input: a.input.clone(),
        output,
        effects: a.effects.union(&b.effects),
        authority: a.authority.union(&b.authority).cloned().collect(),
        epistemic: merge_epistemic(&a.epistemic, &b.epistemic),
        envelope: a.envelope.parallel(&b.envelope),
        assurance: merge_assurance(&a.assurance, &b.assurance),
        failure: merge_failure(&a.failure, &b.failure),
        commitments: union_commitments(&a.commitments, &b.commitments),
        output_labelling: a.output_labelling.join(&b.output_labelling),
    };
    Ok(Composition {
        operator: Operator::Parallel { justification },
        parts: vec![a.clone(), b.clone()],
        contract,
    })
}

/// Effects both components write that the other might also write.
///
/// An effect pair counts as overlapping when either side's scope contains the other's, or when the
/// comparison is undecided — an undeclared scope on a write is exactly the case where nobody can
/// show the writes are disjoint.
pub fn write_set_overlap(a: &AgentContract, b: &AgentContract) -> Vec<Effect> {
    let left = a.effects.write_set();
    let right = b.effects.write_set();
    let mut out = BTreeSet::new();
    for l in &left {
        for r in &right {
            if l.kind != r.kind {
                continue;
            }
            match (l.scope.contains(&r.scope), r.scope.contains(&l.scope)) {
                (crate::effect::Containment::DoesNotContain, crate::effect::Containment::DoesNotContain) => {}
                _ => {
                    out.insert(l.clone());
                    out.insert(r.clone());
                }
            }
        }
    }
    out.into_iter().collect()
}

/// `A ⊗χ B`: choreographed fusion into a Capability Molecule.
///
/// The choreography must be well-formed, which is why this takes a
/// `bioprism_choreography::WellFormedGlobal` rather than a `GlobalType`: 23.06's well-formedness is
/// that crate's job and re-deriving it here would give the workspace two answers. Every part must
/// appear as a role, and every role must have a part; a fusion with an unfilled role is a
/// composition that cannot run and is refused now rather than discovered later.
pub fn fuse(
    parts: &[AgentContract],
    choreography: &WellFormedGlobal,
    exported: &InterfaceType,
) -> Result<Composition, CompositionError> {
    let roles: BTreeSet<String> = choreography
        .roles()
        .map(|role| role.as_str().to_string())
        .collect();
    let participants: BTreeSet<String> =
        parts.iter().map(|p| p.id.as_str().to_string()).collect();
    let unfilled: BTreeSet<String> = roles.difference(&participants).cloned().collect();
    if !unfilled.is_empty() {
        return Err(CompositionError::UnfilledRoles { roles: unfilled });
    }
    let unused: BTreeSet<String> = participants.difference(&roles).cloned().collect();
    if !unused.is_empty() {
        return Err(CompositionError::ParticipantsWithoutRole { parts: unused });
    }
    let digest = choreography
        .digest()
        .map_err(|e| CompositionError::ChoreographyDigest(e.to_string()))?;
    Ok(Composition {
        contract: fold_parts(parts, exported, &digest)?,
        operator: Operator::ChoreographedFusion {
            choreography_digest: digest,
        },
        parts: parts.to_vec(),
    })
}

/// The aggregate contract of a fused molecule.
///
/// Shared by [`fuse`] and by [`substitute`]'s fusion arm so a substituted molecule's effect and
/// authority accounts are recomputed by the same fold that built the original, rather than patched.
fn fold_parts(
    parts: &[AgentContract],
    exported: &InterfaceType,
    digest: &str,
) -> Result<AgentContract, CompositionError> {
    let first = parts.first().ok_or(CompositionError::EmptyFusion)?;
    let mut effects = EffectSet::new();
    let mut authority = BTreeSet::new();
    let mut envelope = ResourceEnvelope::new();
    let mut commitments = BTreeSet::new();
    let mut epistemic = first.epistemic.clone();
    let mut assurance = first.assurance.clone();
    let mut failure = first.failure.clone();
    let mut labelling = first.output_labelling.clone();
    for part in parts {
        effects = effects.union(&part.effects);
        authority.extend(part.authority.iter().cloned());
        envelope = envelope.sum(&part.envelope);
        commitments = union_commitments(&commitments, &part.commitments);
        epistemic = merge_epistemic(&epistemic, &part.epistemic);
        assurance = merge_assurance(&assurance, &part.assurance);
        failure = merge_failure(&failure, &part.failure);
        labelling = labelling.join(&part.output_labelling);
    }
    Ok(AgentContract {
        id: ComponentId::new(format!("molecule:{digest}")),
        input: first.input.clone(),
        output: exported.clone(),
        effects,
        authority,
        epistemic,
        envelope,
        assurance,
        failure,
        commitments,
        output_labelling: labelling,
    })
}

/// `A ⊕r B`: policy choice.
///
/// The composite's effects are the *union*, not the chosen branch's, because the router's decision
/// is not known statically and a contract that promised less than one branch may do would be
/// unsound. 23.41: "This is not equivalent to random model routing" — the router is named in the
/// operator so a trace can attribute the choice.
pub fn choose(
    a: &AgentContract,
    b: &AgentContract,
    router: impl Into<String>,
) -> Result<Composition, CompositionError> {
    if !a.input.subtypes(&b.input) && !b.input.subtypes(&a.input) {
        return Err(CompositionError::BranchesNotInterchangeable {
            left: a.id.clone(),
            right: b.id.clone(),
        });
    }
    let contract = AgentContract {
        id: ComponentId::new(format!("({} (+) {})", a.id.as_str(), b.id.as_str())),
        input: if a.input.subtypes(&b.input) {
            b.input.clone()
        } else {
            a.input.clone()
        },
        output: if a.output.subtypes(&b.output) {
            b.output.clone()
        } else {
            a.output.clone()
        },
        effects: a.effects.union(&b.effects),
        authority: a.authority.union(&b.authority).cloned().collect(),
        epistemic: merge_epistemic(&a.epistemic, &b.epistemic),
        envelope: ResourceEnvelope {
            max_tokens: a.envelope.max_tokens.max(b.envelope.max_tokens),
            max_tool_calls: a.envelope.max_tool_calls.max(b.envelope.max_tool_calls),
            declared_latency_units: a
                .envelope
                .declared_latency_units
                .max(b.envelope.declared_latency_units),
            declared_cost_minor: a
                .envelope
                .declared_cost_minor
                .max(b.envelope.declared_cost_minor),
        },
        assurance: merge_assurance(&a.assurance, &b.assurance),
        failure: merge_failure(&a.failure, &b.failure),
        commitments: union_commitments(&a.commitments, &b.commitments),
        output_labelling: a.output_labelling.join(&b.output_labelling),
    };
    Ok(Composition {
        operator: Operator::PolicyChoice {
            router: router.into(),
        },
        parts: vec![a.clone(), b.clone()],
        contract,
    })
}

/// `race_v(A, B, ...)`: verified race.
///
/// 23.41: "The first result satisfying verifier `v` wins; speed alone cannot determine the winner."
/// A race with no verifier is therefore not this operator and is refused, rather than silently
/// becoming a first-to-finish.
pub fn race_verified(
    branches: &[AgentContract],
    verifier: impl Into<String>,
) -> Result<Composition, CompositionError> {
    let verifier = verifier.into();
    if verifier.is_empty() {
        return Err(CompositionError::UnverifiedRace);
    }
    if branches.len() < 2 {
        return Err(CompositionError::DegenerateRace {
            branches: branches.len(),
        });
    }
    let mut contract = branches[0].clone();
    for branch in &branches[1..] {
        contract.effects = contract.effects.union(&branch.effects);
        contract.authority.extend(branch.authority.iter().cloned());
        contract.epistemic = merge_epistemic(&contract.epistemic, &branch.epistemic);
        contract.envelope = contract.envelope.sum(&branch.envelope);
        contract.assurance = merge_assurance(&contract.assurance, &branch.assurance);
        contract.failure = merge_failure(&contract.failure, &branch.failure);
        contract.commitments = union_commitments(&contract.commitments, &branch.commitments);
        contract.output_labelling = contract.output_labelling.join(&branch.output_labelling);
    }
    contract.id = ComponentId::new(format!("race[{verifier}]"));
    Ok(Composition {
        operator: Operator::VerifiedRace { verifier },
        parts: branches.to_vec(),
        contract,
    })
}

/// `A ⋉g B`: attenuating delegation.
///
/// The grant may not exceed `A`'s delegable authority. Refusal names the capabilities that were
/// asked for and not held, which is the composition-time analogue of
/// `bioprism_weave::AuthorityError::Amplification`.
pub fn delegate(
    delegator: &AgentContract,
    grant: BTreeSet<Capability>,
    delegate_to: &AgentContract,
) -> Result<Composition, CompositionError> {
    let missing: BTreeSet<Capability> = grant.difference(&delegator.authority).cloned().collect();
    if !missing.is_empty() {
        return Err(CompositionError::AuthorityAmplified {
            delegator: delegator.id.clone(),
            missing,
        });
    }
    let mut contract = seq(delegator, delegate_to)?.contract;
    contract.id = ComponentId::new(format!(
        "({} |x {})",
        delegator.id.as_str(),
        delegate_to.id.as_str()
    ));
    contract.authority = delegator.authority.clone();
    Ok(Composition {
        operator: Operator::AttenuatingDelegation { grant },
        parts: vec![delegator.clone(), delegate_to.clone()],
        contract,
    })
}

/// `shield_p(A)`: a guarded component.
///
/// 23.41: "A shield changes the component's assurance contract and must be visible in evaluation."
/// Visibility is a field, not a convention: the composite's `assurance.shielded_by` is set, and
/// nothing clears it.
pub fn shield(
    inner: &AgentContract,
    monitor: impl Into<String>,
    permitted: EffectSet,
) -> Result<Composition, CompositionError> {
    let monitor = monitor.into();
    let escaping = inner.effects.escalation_over(&permitted);
    if !escaping.is_empty() {
        return Err(CompositionError::ShieldCannotContain {
            component: inner.id.clone(),
            escaping,
        });
    }
    let mut contract = inner.clone();
    contract.id = ComponentId::new(format!("shield[{monitor}]({})", inner.id.as_str()));
    contract.effects = permitted;
    contract.assurance.shielded_by = Some(monitor.clone());
    Ok(Composition {
        operator: Operator::Shield { monitor },
        parts: vec![inner.clone()],
        contract,
    })
}

/// `fallback_f(A, B)`: transfer control to `B` under a declared failure predicate.
///
/// The predicate must be named. An unconditioned fallback is a retry loop with better marketing and
/// erases the distinction 23.41 draws between recovery and ordinary choice.
pub fn fallback(
    primary: &AgentContract,
    secondary: &AgentContract,
    predicate: impl Into<String>,
) -> Result<Composition, CompositionError> {
    let predicate = predicate.into();
    if predicate.is_empty() {
        return Err(CompositionError::UndeclaredFailurePredicate);
    }
    let mut composition = choose(primary, secondary, "fallback")?;
    composition.operator = Operator::Fallback { predicate };
    composition.contract.id = ComponentId::new(format!(
        "fallback({}, {})",
        primary.id.as_str(),
        secondary.id.as_str()
    ));
    Ok(composition)
}

/// The laws of 23.41.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Law {
    Identity,
    SequentialAssociativity,
    ParallelCommutativity,
    AuthorityAttenuation,
    AffineNonDuplication,
    CommitmentConservation,
    EpistemicMonotonicity,
}

/// A named reason a conditional law does not hold on a particular instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "violation")]
pub enum Violation {
    IdentityChangesLabel {
        from: Labelling,
        to: Labelling,
    },
    IdentityChangesEnvelope {
        overruns: Vec<EnvelopeOverrun>,
    },
    IdentityChangesProvenance {
        shortfalls: Vec<EpistemicShortfall>,
    },
    /// An identity that emits a claim of its own, or performs an effect, is not an identity.
    IdentityIsNotNeutral {
        claims: BTreeSet<String>,
        effects: Vec<Effect>,
    },
    /// Reassociation moves a compensation-scope boundary across an effect nothing can undo.
    IrreversibleEffectCrossesBoundary {
        effect: Effect,
        component: ComponentId,
    },
    DeadlineSensitiveComponent {
        component: ComponentId,
    },
    WriteSetsOverlap {
        overlap: Vec<Effect>,
    },
    AuthorityCreated {
        capabilities: BTreeSet<Capability>,
    },
    AffineResourceDuplicated {
        resource: String,
        holders: BTreeSet<ComponentId>,
    },
    BudgetOversubscribed {
        component: ComponentId,
        detail: String,
    },
    MandatoryCommitmentUnaccounted {
        path: String,
        commitment: String,
    },
    ClaimErased {
        claim: String,
    },
    EvidenceLineageRemoved {
        claim: String,
        evidence: BTreeSet<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum LawOutcome {
    Holds,
    Fails { violations: Vec<Violation> },
    /// The law's premise does not apply to these arguments at all. Not the same as holding.
    Inapplicable { reason: String },
}

impl LawOutcome {
    pub fn holds(&self) -> bool {
        matches!(self, LawOutcome::Holds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LawReport {
    pub law: Law,
    pub outcome: LawOutcome,
}

impl LawReport {
    fn of(law: Law, violations: Vec<Violation>) -> Self {
        LawReport {
            law,
            outcome: if violations.is_empty() {
                LawOutcome::Holds
            } else {
                LawOutcome::Fails { violations }
            },
        }
    }
}

/// `A ▷ 1_T ≈ A` and `1_T ▷ A ≈ A`.
///
/// 23.41: "The equivalence fails if the identity changes provenance, budgets, time, or security
/// labels." Time is the one this crate cannot check — there is no clock — and its absence is
/// reported as part of [`identity_law_unchecked_conditions`] rather than silently passed.
pub fn check_identity(a: &AgentContract, identity: &AgentContract) -> LawReport {
    let mut violations = Vec::new();
    if identity.output_labelling != Labelling::Unlabelled
        && identity.output_labelling != a.output_labelling
    {
        violations.push(Violation::IdentityChangesLabel {
            from: a.output_labelling.clone(),
            to: identity.output_labelling.clone(),
        });
    }
    let overruns = identity.envelope.within(&ResourceEnvelope::new());
    if !overruns.is_empty() {
        violations.push(Violation::IdentityChangesEnvelope { overruns });
    }
    let mut shortfalls = Vec::new();
    if identity.epistemic.uncertainty < a.epistemic.uncertainty {
        shortfalls.push(EpistemicShortfall::LessCalibrated {
            required: a.epistemic.uncertainty,
            offered: identity.epistemic.uncertainty,
        });
    }
    if a.epistemic.provenance_complete && !identity.epistemic.provenance_complete {
        shortfalls.push(EpistemicShortfall::ProvenanceIncomplete);
    }
    if !shortfalls.is_empty() {
        violations.push(Violation::IdentityChangesProvenance { shortfalls });
    }
    if !identity.epistemic.claims_emitted.is_empty() || !identity.effects.is_empty() {
        violations.push(Violation::IdentityIsNotNeutral {
            claims: identity.epistemic.claims_emitted.clone(),
            effects: identity.effects.iter().cloned().collect(),
        });
    }
    LawReport::of(Law::Identity, violations)
}

/// Conditions the identity law depends on that this crate cannot evaluate.
///
/// 23.41 names four; three are checkable from contracts and one is not.
pub fn identity_law_unchecked_conditions() -> &'static [&'static str] {
    &["time: no clock exists in this crate, so an identity that delays cannot be detected"]
}

/// `(A ▷ B) ▷ C ≈ A ▷ (B ▷ C)`.
///
/// 23.41: holds "only when continuation boundaries, compensation scopes, and budget reservations
/// are observationally equivalent. It does not generally hold for irreversible effects or
/// deadline-sensitive behavior." Both exceptions are checkable: an `E4` effect a component cannot
/// compensate makes the compensation scope boundary observable, and a deadline-sensitive component
/// makes the grouping observable through timing.
pub fn check_sequential_associativity(
    a: &AgentContract,
    b: &AgentContract,
    c: &AgentContract,
) -> LawReport {
    let left = seq(a, b).and_then(|ab| seq(&ab.contract, c));
    let right = seq(b, c).and_then(|bc| seq(a, &bc.contract));
    match (left, right) {
        (Ok(_), Ok(_)) => {}
        _ => {
            return LawReport {
                law: Law::SequentialAssociativity,
                outcome: LawOutcome::Inapplicable {
                    reason: "one grouping is not type-correct, so the two sides are not both \
                             constructible and there is nothing to compare"
                        .to_string(),
                },
            }
        }
    }
    let mut violations = Vec::new();
    for part in [a, b, c] {
        for effect in part.uncompensated_effects() {
            if effect.class.is_irreversible() {
                violations.push(Violation::IrreversibleEffectCrossesBoundary {
                    effect,
                    component: part.id.clone(),
                });
            }
        }
        if part.failure.deadline_sensitive {
            violations.push(Violation::DeadlineSensitiveComponent {
                component: part.id.clone(),
            });
        }
    }
    LawReport::of(Law::SequentialAssociativity, violations)
}

/// `A ∥ B ≈ B ∥ A`.
///
/// Holds only for causally independent or explicitly commutative effects.
pub fn check_parallel_commutativity(a: &AgentContract, b: &AgentContract) -> LawReport {
    let overlap = write_set_overlap(a, b);
    LawReport::of(
        Law::ParallelCommutativity,
        if overlap.is_empty() {
            Vec::new()
        } else {
            vec![Violation::WriteSetsOverlap { overlap }]
        },
    )
}

/// `g_child ⊆ g_parent`.
///
/// 23.41: "No composition operator may create authority absent from its inputs or a separate
/// approval transition." Checked over a whole composition, so an operator that quietly widened the
/// composite's authority is caught even if each individual delegation was legal.
pub fn check_authority_attenuation(composition: &Composition) -> LawReport {
    let available: BTreeSet<Capability> = composition
        .parts
        .iter()
        .flat_map(|p| p.authority.iter().cloned())
        .collect();
    let created: BTreeSet<Capability> = composition
        .contract
        .authority
        .difference(&available)
        .cloned()
        .collect();
    LawReport::of(
        Law::AuthorityAttenuation,
        if created.is_empty() {
            Vec::new()
        } else {
            vec![Violation::AuthorityCreated {
                capabilities: created,
            }]
        },
    )
}

/// How an affine resource is spread across a composite's parts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeasePlan {
    pub resource: Resource,
    pub total: u64,
    /// Deterministically ordered, so the component named in an oversubscription violation is the
    /// same on every run.
    pub allocations: BTreeMap<ComponentId, u64>,
    /// One-time secrets, exclusive locks and irreversible-action permits, mapped to their holders.
    pub exclusive_holders: BTreeMap<String, BTreeSet<ComponentId>>,
}

impl LeasePlan {
    pub fn new(resource: Resource, total: u64) -> Self {
        LeasePlan {
            resource,
            total,
            allocations: BTreeMap::new(),
            exclusive_holders: BTreeMap::new(),
        }
    }

    pub fn allocating(mut self, component: &ComponentId, amount: u64) -> Self {
        self.allocations.insert(component.clone(), amount);
        self
    }

    pub fn holding(mut self, resource: impl Into<String>, component: &ComponentId) -> Self {
        self.exclusive_holders
            .entry(resource.into())
            .or_default()
            .insert(component.clone());
        self
    }
}

/// "Budget leases, one-time secrets, exclusive locks, and irreversible-action permits are affine
/// resources. Composition may consume or subdivide them but may not duplicate them."
///
/// The budget half runs a real `bioprism_weave::Budget` and asks it to `split` each allocation in
/// key order. There is one implementation of non-duplication in this workspace and it is the
/// kernel's; this function is a caller, not a copy, so a change to the kernel's affine rule shows
/// up here rather than silently diverging.
pub fn check_affine_non_duplication(plan: &LeasePlan) -> LawReport {
    let mut violations = Vec::new();
    let mut budget = Budget::new().with(plan.resource, plan.total);
    for (component, amount) in &plan.allocations {
        if let Err(error) = budget.split(plan.resource, *amount) {
            violations.push(Violation::BudgetOversubscribed {
                component: component.clone(),
                detail: error.to_string(),
            });
        }
    }
    for (resource, holders) in &plan.exclusive_holders {
        if holders.len() > 1 {
            violations.push(Violation::AffineResourceDuplicated {
                resource: resource.clone(),
                holders: holders.clone(),
            });
        }
    }
    LawReport::of(Law::AffineNonDuplication, violations)
}

/// What a terminal path did with a commitment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "disposition")]
pub enum CommitmentDisposition {
    Closed,
    Transferred { to: ComponentId },
    Split { into: BTreeSet<String> },
    Renegotiated { into: String },
}

/// One way the composite can terminate, and what happened to every commitment on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalPath {
    pub name: String,
    pub dispositions: BTreeMap<String, CommitmentDisposition>,
}

impl TerminalPath {
    pub fn new(name: impl Into<String>) -> Self {
        TerminalPath {
            name: name.into(),
            dispositions: BTreeMap::new(),
        }
    }

    pub fn disposing(
        mut self,
        commitment: impl Into<String>,
        disposition: CommitmentDisposition,
    ) -> Self {
        self.dispositions.insert(commitment.into(), disposition);
        self
    }
}

/// "A composition may close, transfer, split, or renegotiate a commitment, but may not silently
/// discard it. Every terminal path must account for all mandatory obligations."
///
/// Discretionary commitments are exempt, which is the only thing that makes the law satisfiable in
/// practice; the distinction lives on [`DeclaredCommitment::mandatory`].
pub fn check_commitment_conservation(
    composition: &Composition,
    paths: &[TerminalPath],
) -> LawReport {
    let mut violations = Vec::new();
    let mandatory = composition.contract.mandatory_commitments();
    if paths.is_empty() {
        return LawReport {
            law: Law::CommitmentConservation,
            outcome: LawOutcome::Inapplicable {
                reason: "no terminal path was supplied; a composition with no terminal states has \
                         nothing to conserve against"
                    .to_string(),
            },
        };
    }
    for path in paths {
        for commitment in &mandatory {
            if !path.dispositions.contains_key(&commitment.id) {
                violations.push(Violation::MandatoryCommitmentUnaccounted {
                    path: path.name.clone(),
                    commitment: commitment.id.clone(),
                });
            }
        }
    }
    LawReport::of(Law::CommitmentConservation, violations)
}

/// A snapshot of what has been claimed and on what evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceState {
    pub claims_made: BTreeSet<String>,
    pub evidence_lineage: BTreeMap<String, BTreeSet<String>>,
    pub retracted: BTreeSet<String>,
    pub superseded: BTreeMap<String, String>,
}

impl ProvenanceState {
    pub fn asserting(mut self, claim: impl Into<String>, evidence: &[&str]) -> Self {
        let claim = claim.into();
        self.claims_made.insert(claim.clone());
        self.evidence_lineage
            .insert(claim, evidence.iter().map(|e| e.to_string()).collect());
        self
    }

    pub fn retracting(mut self, claim: impl Into<String>) -> Self {
        self.retracted.insert(claim.into());
        self
    }
}

/// "An agent may retract or supersede a claim, but it may not erase the fact that the claim was
/// made or remove the evidence lineage used to support it."
///
/// Retraction is therefore an *addition* to `retracted`, never a removal from `claims_made`. This
/// is the same monotonicity `bioprism-weave`'s epistemic ledger enforces on events; here it is
/// stated over the derived provenance state so a composition can be checked without a ledger.
pub fn check_epistemic_monotonicity(before: &ProvenanceState, after: &ProvenanceState) -> LawReport {
    let mut violations = Vec::new();
    for claim in &before.claims_made {
        if !after.claims_made.contains(claim) {
            violations.push(Violation::ClaimErased {
                claim: claim.clone(),
            });
        }
    }
    for (claim, evidence) in &before.evidence_lineage {
        let removed: BTreeSet<String> = match after.evidence_lineage.get(claim) {
            Some(later) => evidence.difference(later).cloned().collect(),
            None => evidence.clone(),
        };
        if !removed.is_empty() {
            violations.push(Violation::EvidenceLineageRemoved {
                claim: claim.clone(),
                evidence: removed,
            });
        }
    }
    LawReport::of(Law::EpistemicMonotonicity, violations)
}

/// 23.41's eight equivalence dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceDimension {
    Functional,
    Evidence,
    Effect,
    Commitment,
    Safety,
    CostDistribution,
    LatencyDistribution,
    TraceExplanation,
}

impl EquivalenceDimension {
    pub const ALL: [EquivalenceDimension; 8] = [
        EquivalenceDimension::Functional,
        EquivalenceDimension::Evidence,
        EquivalenceDimension::Effect,
        EquivalenceDimension::Commitment,
        EquivalenceDimension::Safety,
        EquivalenceDimension::CostDistribution,
        EquivalenceDimension::LatencyDistribution,
        EquivalenceDimension::TraceExplanation,
    ];
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum DimensionVerdict {
    Equivalent,
    Differs { detail: String },
    /// Nothing in a contract determines this dimension, so no answer is available from contracts
    /// alone. Reported rather than defaulted, because defaulting to `Equivalent` would make the
    /// report say two agents match on a dimension nobody examined.
    NotComparable { reason: String },
}

/// Per-dimension equivalence.
///
/// Deliberately has no method that reduces to a boolean.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceReport {
    pub left: ComponentId,
    pub right: ComponentId,
    pub dimensions: BTreeMap<EquivalenceDimension, DimensionVerdict>,
}

impl EquivalenceReport {
    pub fn dimensions_differing(&self) -> Vec<EquivalenceDimension> {
        self.dimensions
            .iter()
            .filter(|(_, v)| matches!(v, DimensionVerdict::Differs { .. }))
            .map(|(d, _)| *d)
            .collect()
    }
}

/// Compare two contracts on all eight dimensions.
pub fn compare(left: &AgentContract, right: &AgentContract) -> EquivalenceReport {
    let mut dimensions = BTreeMap::new();

    dimensions.insert(
        EquivalenceDimension::Functional,
        if left.output.subtypes(&right.output) && right.output.subtypes(&left.output) {
            DimensionVerdict::Equivalent
        } else {
            DimensionVerdict::Differs {
                detail: format!(
                    "output fields differ: {:?}",
                    left.output.missing_against(&right.output)
                ),
            }
        },
    );

    let evidence_shortfalls = left.epistemic.refines(&right.epistemic);
    dimensions.insert(
        EquivalenceDimension::Evidence,
        if evidence_shortfalls.is_empty() && right.epistemic.refines(&left.epistemic).is_empty() {
            DimensionVerdict::Equivalent
        } else {
            DimensionVerdict::Differs {
                detail: format!("{evidence_shortfalls:?}"),
            }
        },
    );

    dimensions.insert(
        EquivalenceDimension::Effect,
        match (
            left.effects.includes(&right.effects),
            right.effects.includes(&left.effects),
        ) {
            (Inclusion::Holds, Inclusion::Holds) => DimensionVerdict::Equivalent,
            (Inclusion::Undecided { witnesses }, _) | (_, Inclusion::Undecided { witnesses }) => {
                DimensionVerdict::NotComparable {
                    reason: format!("undeclared scopes on {} effect(s)", witnesses.len()),
                }
            }
            _ => DimensionVerdict::Differs {
                detail: "effect sets are not mutually inclusive".to_string(),
            },
        },
    );

    dimensions.insert(
        EquivalenceDimension::Commitment,
        if left.commitments == right.commitments {
            DimensionVerdict::Equivalent
        } else {
            DimensionVerdict::Differs {
                detail: "declared commitments differ".to_string(),
            }
        },
    );

    dimensions.insert(
        EquivalenceDimension::Safety,
        if left.peak_class() == right.peak_class()
            && left.assurance.shielded_by == right.assurance.shielded_by
        {
            DimensionVerdict::Equivalent
        } else {
            DimensionVerdict::Differs {
                detail: format!(
                    "peak class {:?} vs {:?}",
                    left.peak_class(),
                    right.peak_class()
                ),
            }
        },
    );

    dimensions.insert(
        EquivalenceDimension::CostDistribution,
        DimensionVerdict::NotComparable {
            reason: "a contract declares a cost ceiling, not a distribution; comparing \
                     distributions needs measurements this crate never takes"
                .to_string(),
        },
    );
    dimensions.insert(
        EquivalenceDimension::LatencyDistribution,
        DimensionVerdict::NotComparable {
            reason: "same as cost: a declared ceiling is not a distribution, and there is no clock"
                .to_string(),
        },
    );
    dimensions.insert(
        EquivalenceDimension::TraceExplanation,
        DimensionVerdict::NotComparable {
            reason: "nothing here executes, so there is no trace to compare".to_string(),
        },
    );

    EquivalenceReport {
        left: left.id.clone(),
        right: right.id.clone(),
        dimensions,
    }
}

/// The context a substitution happens in: what the surrounding composition allows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubstitutionContext {
    pub allowed_envelope: ResourceEnvelope,
    pub minimum_assurance: crate::contract::AssuranceProfile,
}

/// A clause of 23.41's substitution relation that the replacement failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "objection")]
pub enum SubstitutionObjection {
    /// `B.Iin accepts A.Iin` failed: the replacement demands inputs the original did not.
    InputNarrowed { missing: Vec<String> },
    /// `B.Iout refines A.Iout` failed: the replacement delivers less.
    OutputDoesNotRefine { missing: Vec<String> },
    /// `B.E ⊆ A.E` failed.
    EffectsNotSubset { extra: Vec<Effect> },
    /// `B.E ⊆ A.E` could not be decided.
    EffectsUndecided { undecided: Vec<Effect> },
    /// `B.G ⊆ A.G` failed.
    AuthorityBroadened { extra: BTreeSet<Capability> },
    /// `B.B stays within the allowed envelope` failed.
    EnvelopeExceeded { overruns: Vec<EnvelopeOverrun> },
    /// `B.F preserves required failure semantics` failed.
    FailureSemanticsWeakened { shortfalls: Vec<FailureShortfall> },
    /// `B.Q meets the minimum verified profile` failed.
    AssuranceInsufficient {
        shortfalls: Vec<crate::contract::AssuranceShortfall>,
    },
    /// Not one of 23.41's seven clauses but implied by its worked example: the replacement omits
    /// evidence or reports uncertainty more coarsely.
    EpistemicWeakened { shortfalls: Vec<EpistemicShortfall> },
    /// The replacement emits at a label the original did not, so a downstream recipient cleared for
    /// the original may not be cleared for this.
    ///
    /// Boxed because a [`Labelling`] carries compartment, purpose and residency sets and is an
    /// order of magnitude larger than every other variant here.
    OutputLabelWidened { labels: Box<LabelChange> },
}

/// The before and after of an output label, boxed out of [`SubstitutionObjection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelChange {
    pub from: Labelling,
    pub to: Labelling,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum SubstitutionVerdict {
    /// Contextual refinement holds on every clause.
    Refines,
    Refused {
        objections: Vec<SubstitutionObjection>,
    },
    /// Nothing definitively fails but at least one clause could not be decided. A gate treats this
    /// as refusal; a report does not.
    Undecided {
        objections: Vec<SubstitutionObjection>,
    },
}

impl SubstitutionVerdict {
    pub fn admitted(&self) -> bool {
        matches!(self, SubstitutionVerdict::Refines)
    }
}

/// May `replacement` stand in for `original` in a context that allows `context`?
///
/// 23.41's seven clauses, plus two this crate adds because 23.41's own prose demands them and its
/// clause list omits them: the epistemic clause (its worked counterexample is entirely epistemic)
/// and the output-label clause (23.14's flow rule applies to a substituted participant's output as
/// much as to anyone's).
pub fn substitutable(
    replacement: &AgentContract,
    original: &AgentContract,
    context: &SubstitutionContext,
) -> SubstitutionVerdict {
    let mut objections = Vec::new();
    let mut undecided = Vec::new();

    let missing_input = original.input.missing_against(&replacement.input);
    if !missing_input.is_empty() {
        objections.push(SubstitutionObjection::InputNarrowed {
            missing: missing_input,
        });
    }

    let missing_output = replacement.output.missing_against(&original.output);
    if !missing_output.is_empty() {
        objections.push(SubstitutionObjection::OutputDoesNotRefine {
            missing: missing_output,
        });
    }

    match original.effects.includes(&replacement.effects) {
        Inclusion::Holds => {}
        Inclusion::Fails { witnesses } => {
            objections.push(SubstitutionObjection::EffectsNotSubset { extra: witnesses })
        }
        Inclusion::Undecided { witnesses } => {
            undecided.push(SubstitutionObjection::EffectsUndecided {
                undecided: witnesses,
            })
        }
    }

    let extra_authority: BTreeSet<Capability> = replacement
        .authority
        .difference(&original.authority)
        .cloned()
        .collect();
    if !extra_authority.is_empty() {
        objections.push(SubstitutionObjection::AuthorityBroadened {
            extra: extra_authority,
        });
    }

    let overruns = replacement.envelope.within(&context.allowed_envelope);
    if !overruns.is_empty() {
        objections.push(SubstitutionObjection::EnvelopeExceeded { overruns });
    }

    let failure_shortfalls = replacement.failure.preserves(&original.failure);
    if !failure_shortfalls.is_empty() {
        objections.push(SubstitutionObjection::FailureSemanticsWeakened {
            shortfalls: failure_shortfalls,
        });
    }

    let assurance_shortfalls = replacement.assurance.meets(&context.minimum_assurance);
    if !assurance_shortfalls.is_empty() {
        objections.push(SubstitutionObjection::AssuranceInsufficient {
            shortfalls: assurance_shortfalls,
        });
    }

    let epistemic_shortfalls = replacement.epistemic.refines(&original.epistemic);
    if !epistemic_shortfalls.is_empty() {
        objections.push(SubstitutionObjection::EpistemicWeakened {
            shortfalls: epistemic_shortfalls,
        });
    }

    if replacement.output_labelling != original.output_labelling
        && !replacement
            .output_labelling
            .flows_to(&original.output_labelling)
            .permitted()
    {
        objections.push(SubstitutionObjection::OutputLabelWidened {
            labels: Box::new(LabelChange {
                from: original.output_labelling.clone(),
                to: replacement.output_labelling.clone(),
            }),
        });
    }

    if !objections.is_empty() {
        SubstitutionVerdict::Refused { objections }
    } else if !undecided.is_empty() {
        SubstitutionVerdict::Undecided {
            objections: undecided,
        }
    } else {
        SubstitutionVerdict::Refines
    }
}

/// Substitute one part of a composition and rebuild the composite contract.
///
/// Refuses before rebuilding, so a composition is never left holding a part that failed the
/// relation. The rebuilt composite is recomputed from the operator rather than patched, which is
/// what makes the effect and authority accounts of the new whole trustworthy.
pub fn substitute(
    composition: &Composition,
    target: &ComponentId,
    replacement: &AgentContract,
    context: &SubstitutionContext,
) -> Result<Composition, CompositionError> {
    let index = composition
        .parts
        .iter()
        .position(|p| &p.id == target)
        .ok_or_else(|| CompositionError::NoSuchPart {
            part: target.clone(),
        })?;
    let verdict = substitutable(replacement, &composition.parts[index], context);
    if !verdict.admitted() {
        return Err(CompositionError::SubstitutionRefused {
            replacing: target.clone(),
            with: replacement.id.clone(),
            verdict: Box::new(verdict),
        });
    }
    let mut parts = composition.parts.clone();
    parts[index] = replacement.clone();
    match &composition.operator {
        Operator::Sequential => seq(&parts[0], &parts[1]),
        Operator::Parallel { justification } => par(&parts[0], &parts[1], justification.clone()),
        Operator::PolicyChoice { router } => choose(&parts[0], &parts[1], router.clone()),
        Operator::VerifiedRace { verifier } => race_verified(&parts, verifier.clone()),
        Operator::Fallback { predicate } => fallback(&parts[0], &parts[1], predicate.clone()),
        Operator::AttenuatingDelegation { grant } => {
            delegate(&parts[0], grant.clone(), &parts[1])
        }
        Operator::ChoreographedFusion {
            choreography_digest,
        } => Ok(Composition {
            contract: fold_parts(
                &parts,
                &composition.contract.output,
                choreography_digest,
            )?,
            operator: composition.operator.clone(),
            parts,
        }),
        other => Err(CompositionError::SubstitutionUnsupported {
            operator: Box::new(other.clone()),
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompositionError {
    #[error("{producer} does not satisfy {consumer}'s input: missing or ill-typed {missing:?}")]
    InterfaceMismatch {
        producer: ComponentId,
        consumer: ComponentId,
        missing: Vec<String>,
    },

    #[error("{left} and {right} write overlapping resources and no merge contract was declared: {overlap:?}")]
    WriteSetsOverlap {
        left: ComponentId,
        right: ComponentId,
        overlap: Vec<Effect>,
    },

    #[error("choreography roles {roles:?} have no participant")]
    UnfilledRoles { roles: BTreeSet<String> },

    #[error("participants {parts:?} appear in the fusion and in no choreography role")]
    ParticipantsWithoutRole { parts: BTreeSet<String> },

    #[error("fusion of zero participants")]
    EmptyFusion,

    #[error("choreography digest failed: {0}")]
    ChoreographyDigest(String),

    #[error("{left} and {right} accept unrelated inputs, so a router cannot choose between them")]
    BranchesNotInterchangeable {
        left: ComponentId,
        right: ComponentId,
    },

    #[error("a race without a verifier is decided by speed, which 23.41 forbids")]
    UnverifiedRace,

    #[error("a race needs at least two branches, got {branches}")]
    DegenerateRace { branches: usize },

    #[error("{delegator} cannot delegate {missing:?}: it does not hold them")]
    AuthorityAmplified {
        delegator: ComponentId,
        missing: BTreeSet<Capability>,
    },

    #[error("shield cannot contain {component}: {escaping:?} escape the permitted set")]
    ShieldCannotContain {
        component: ComponentId,
        escaping: Vec<Effect>,
    },

    #[error("a fallback needs a declared failure predicate")]
    UndeclaredFailurePredicate,

    #[error("no part named {part} in this composition")]
    NoSuchPart { part: ComponentId },

    #[error("{with} may not replace {replacing}: {verdict:?}")]
    SubstitutionRefused {
        replacing: ComponentId,
        with: ComponentId,
        verdict: Box<SubstitutionVerdict>,
    },

    #[error("substitution into {operator:?} needs data the operator does not carry")]
    SubstitutionUnsupported { operator: Box<Operator> },
}
