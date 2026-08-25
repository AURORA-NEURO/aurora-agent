//! Semantic lifecycle, compaction and garbage collection.
//!
//! Blueprint 23.48, with the compaction rules of 23.44 folded in where they overlap.
//!
//! # Why this is harder than log compaction
//!
//! `bioprism-ledger` established the rule for byte-level compaction: a compaction must declare its
//! retained window, and verification after it is *weaker* than verification before it. That rule is
//! about whether a hash chain still checks. This module is about whether a *conclusion* still
//! stands.
//!
//! Forgetting a claim can invalidate something that rested on it. A conclusion whose premise has
//! been compacted away is not merely harder to verify — it is unsupported, and it is still sitting
//! in the thread being relied on. [`CompactionPlan::check`] refuses such a plan and
//! [`CompactionRefusal::WouldOrphanConclusion`] **names the conclusion**, because "compaction
//! rejected" without saying what would have broken leaves an operator with no move.
//!
//! 23.48 states the requirement negatively — "Compaction cannot remove evidence needed for an
//! active dispute or audit" — and never says how to find such evidence. The answer here is
//! reachability: a premise is protected when a live conclusion depends on it, transitively, and
//! `depends_on` is a relation a caller supplies because this crate cannot read a scientific
//! argument.
//!
//! # Reachability, and the thing content hashes cannot decide
//!
//! 23.48: "Content hashes alone do not determine retention permission." [`LifecycleGraph`] is
//! therefore keyed by object identity with an explicit [`Root`] set, and an object's hash appears
//! nowhere in the liveness computation. A legal hold on an object with a hundred identical copies
//! holds all hundred.
//!
//! # Not implemented
//!
//! No deletion. Nothing here removes anything from anywhere; [`CompactionCertificate`] is a
//! permission, and the caller does the forgetting. No clock, so every expiry check takes an
//! explicit `as_of`. No signatures, so a "signed checkpoint" is a checkpoint with a digest. No
//! cross-organization transport — [`DeletionAttestation`] is a value a caller receives from
//! somewhere this crate knows nothing about.

use crate::reputation::LogicalTime;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The kinds of thing 23.48's collector tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    Thread,
    RoleBinding,
    ContextCapsule,
    Continuation,
    Branch,
    Commitment,
    Grant,
    BudgetLease,
    EvidenceOperation,
    CommonGroundView,
    Artifact,
    Monitor,
    Subscription,
    ExternalHandle,
    Molecule,
}

/// 23.48's eleven object states. Transitions are typed; see [`ObjectState::may_transition_to`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObjectState {
    Active,
    Quiescent,
    Closing,
    Archived,
    Compacted,
    Expired,
    Revoked,
    ScheduledForDeletion,
    Held,
    Purged,
}

impl ObjectState {
    pub const ALL: [ObjectState; 10] = [
        ObjectState::Active,
        ObjectState::Quiescent,
        ObjectState::Closing,
        ObjectState::Archived,
        ObjectState::Compacted,
        ObjectState::Expired,
        ObjectState::Revoked,
        ObjectState::ScheduledForDeletion,
        ObjectState::Held,
        ObjectState::Purged,
    ];

    /// The transition table.
    ///
    /// **23.48 lists the states and says "Transitions are typed and auditable" and gives no table.**
    /// This is the table, and the three constraints that shape it are stated so a disagreement is
    /// about them rather than about forty individual arrows: `Purged` is terminal, `Held` may be
    /// entered from anything not already purged and left only back to the state that can be
    /// re-evaluated, and `ScheduledForDeletion` is the only predecessor of `Purged`.
    pub fn may_transition_to(&self, next: ObjectState) -> bool {
        use ObjectState::*;
        if *self == next {
            return false;
        }
        match self {
            Purged => false,
            Held => matches!(next, Active | Quiescent | Archived | ScheduledForDeletion),
            Active => matches!(next, Quiescent | Closing | Revoked | Expired | Held),
            Quiescent => matches!(next, Active | Closing | Archived | Expired | Revoked | Held),
            Closing => matches!(next, Archived | Expired | Held),
            Archived => matches!(next, Compacted | ScheduledForDeletion | Held),
            Compacted => matches!(next, ScheduledForDeletion | Held),
            Expired | Revoked => matches!(next, ScheduledForDeletion | Archived | Held),
            ScheduledForDeletion => matches!(next, Purged | Held),
        }
    }
}

/// An object identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ObjectId(pub String);

impl ObjectId {
    pub fn new(id: impl Into<String>) -> Self {
        ObjectId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// 23.48's nine reachability roots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Root {
    ActiveThread,
    OpenMandatoryCommitment,
    UnresolvedDispute,
    ValidContinuation,
    RetainedResultBundle,
    LegalOrAuditHold,
    PublishedMoleculeVersion,
    ActiveGrantLeaseOrSubscription,
    UserPinnedArtifact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleObject {
    pub id: ObjectId,
    pub kind: ObjectKind,
    pub state: ObjectState,
    /// Objects this one keeps alive.
    pub references: BTreeSet<ObjectId>,
    /// Non-empty when this object is itself a retention root.
    pub roots: BTreeSet<Root>,
    pub expires_at: Option<LogicalTime>,
}

impl LifecycleObject {
    pub fn new(id: impl Into<String>, kind: ObjectKind) -> Self {
        LifecycleObject {
            id: ObjectId::new(id),
            kind,
            state: ObjectState::Active,
            references: BTreeSet::new(),
            roots: BTreeSet::new(),
            expires_at: None,
        }
    }

    pub fn referencing(mut self, other: impl Into<String>) -> Self {
        self.references.insert(ObjectId::new(other));
        self
    }

    pub fn rooted_by(mut self, root: Root) -> Self {
        self.roots.insert(root);
        self
    }

    pub fn expiring_at(mut self, at: u64) -> Self {
        self.expires_at = Some(LogicalTime(at));
        self
    }
}

/// The object graph a collector walks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifecycleGraph {
    objects: BTreeMap<ObjectId, LifecycleObject>,
}

impl LifecycleGraph {
    pub fn new() -> Self {
        LifecycleGraph::default()
    }

    pub fn insert(&mut self, object: LifecycleObject) {
        self.objects.insert(object.id.clone(), object);
    }

    pub fn with(mut self, object: LifecycleObject) -> Self {
        self.insert(object);
        self
    }

    pub fn get(&self, id: &ObjectId) -> Option<&LifecycleObject> {
        self.objects.get(id)
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Transition an object, refusing anything the table forbids.
    pub fn transition(
        &mut self,
        id: &ObjectId,
        next: ObjectState,
    ) -> Result<ObjectState, LifecycleError> {
        let object = self
            .objects
            .get_mut(id)
            .ok_or_else(|| LifecycleError::NoSuchObject { id: id.clone() })?;
        if !object.state.may_transition_to(next) {
            return Err(LifecycleError::IllegalTransition {
                id: id.clone(),
                from: object.state,
                to: next,
            });
        }
        let previous = object.state;
        object.state = next;
        Ok(previous)
    }

    /// Everything reachable from a retention root.
    ///
    /// Roots are objects that carry one, plus every object they reference transitively. An expired
    /// or purged object stops the walk: it cannot keep anything else alive.
    pub fn live_set(&self, as_of: LogicalTime) -> BTreeSet<ObjectId> {
        let mut live = BTreeSet::new();
        let mut queue: VecDeque<ObjectId> = self
            .objects
            .values()
            .filter(|object| !object.roots.is_empty() && !Self::is_dead(object, as_of))
            .map(|object| object.id.clone())
            .collect();
        while let Some(current) = queue.pop_front() {
            if !live.insert(current.clone()) {
                continue;
            }
            if let Some(object) = self.objects.get(&current) {
                for reference in &object.references {
                    if let Some(target) = self.objects.get(reference) {
                        if !Self::is_dead(target, as_of) {
                            queue.push_back(reference.clone());
                        }
                    }
                }
            }
        }
        live
    }

    fn is_dead(object: &LifecycleObject, as_of: LogicalTime) -> bool {
        if object.state == ObjectState::Purged {
            return true;
        }
        match object.expires_at {
            Some(expiry) => as_of >= expiry && object.state != ObjectState::Held,
            None => false,
        }
    }

    /// Objects reachable from nothing. Candidates for collection, not an instruction to collect.
    pub fn unreachable(&self, as_of: LogicalTime) -> BTreeSet<ObjectId> {
        let live = self.live_set(as_of);
        self.objects
            .keys()
            .filter(|id| !live.contains(*id))
            .cloned()
            .collect()
    }

    /// Objects a thread would leak if it closed now: a grant, lease, subscription or timer still
    /// active. 23.48's "grant and reservation leaks" microbenchmark.
    pub fn leaked_handles(&self, as_of: LogicalTime) -> BTreeSet<ObjectId> {
        self.live_set(as_of)
            .into_iter()
            .filter(|id| {
                self.objects
                    .get(id)
                    .map(|object| {
                        matches!(
                            object.kind,
                            ObjectKind::Grant
                                | ObjectKind::BudgetLease
                                | ObjectKind::Subscription
                                | ObjectKind::Monitor
                                | ObjectKind::ExternalHandle
                        ) && object.state == ObjectState::Active
                    })
                    .unwrap_or(false)
            })
            .collect()
    }
}

/// The eleven checks of 23.48's closure protocol, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClosureCheck {
    MandatoryCommitmentsAccountedFor,
    IrreversibleEffectsRecorded,
    CompensationsCompleteOrWaived,
    GrantsRevokedOrExpired,
    BudgetReservationsReleased,
    BranchesJoinedDetachedOrAborted,
    ContinuationsConsumedTransferredOrExpired,
    UnresolvedDisputesExported,
    RequiredArtifactsRetained,
    RetentionPoliciesScheduled,
    TerminalResultEmitted,
}

impl ClosureCheck {
    pub const ALL: [ClosureCheck; 11] = [
        ClosureCheck::MandatoryCommitmentsAccountedFor,
        ClosureCheck::IrreversibleEffectsRecorded,
        ClosureCheck::CompensationsCompleteOrWaived,
        ClosureCheck::GrantsRevokedOrExpired,
        ClosureCheck::BudgetReservationsReleased,
        ClosureCheck::BranchesJoinedDetachedOrAborted,
        ClosureCheck::ContinuationsConsumedTransferredOrExpired,
        ClosureCheck::UnresolvedDisputesExported,
        ClosureCheck::RequiredArtifactsRetained,
        ClosureCheck::RetentionPoliciesScheduled,
        ClosureCheck::TerminalResultEmitted,
    ];
}

/// An open condition, with the object that keeps it open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenCondition {
    pub check: ClosureCheck,
    pub blocking: BTreeSet<ObjectId>,
}

/// 23.48: "A partial or blocked terminal state may still be valid if its open conditions are
/// explicit." Which is why the blocked variant carries them rather than being an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum ClosureOutcome {
    Closed,
    /// Terminal, valid, and not clean. The conditions are named, which is what makes it valid.
    PartialTerminal { open: Vec<OpenCondition> },
}

impl ClosureOutcome {
    pub fn is_clean(&self) -> bool {
        matches!(self, ClosureOutcome::Closed)
    }
}

/// What a caller asserts it has already done, since this crate cannot observe any of it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureEvidence {
    pub irreversible_effects_recorded: bool,
    pub compensations_settled: bool,
    pub disputes_exported: bool,
    pub retention_scheduled: bool,
    pub terminal_result_emitted: bool,
}

/// Run the closure protocol over a graph.
///
/// The object-graph checks are computed; the five in [`ClosureEvidence`] are asserted by the
/// caller, and a caller that asserts falsely gets a clean closure it did not earn. Said plainly
/// here because a function named `close_thread` returning `Closed` looks like a verification and is
/// partly a transcription.
pub fn close_thread(
    graph: &LifecycleGraph,
    evidence: &ClosureEvidence,
    as_of: LogicalTime,
) -> ClosureOutcome {
    let mut open = Vec::new();
    let live = graph.live_set(as_of);

    let blocking_for = |kinds: &[ObjectKind], states: &[ObjectState]| -> BTreeSet<ObjectId> {
        live.iter()
            .filter(|id| {
                graph
                    .get(id)
                    .map(|object| {
                        kinds.contains(&object.kind) && states.contains(&object.state)
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect()
    };

    let commitments = blocking_for(
        &[ObjectKind::Commitment],
        &[ObjectState::Active, ObjectState::Closing],
    );
    if !commitments.is_empty() {
        open.push(OpenCondition {
            check: ClosureCheck::MandatoryCommitmentsAccountedFor,
            blocking: commitments,
        });
    }
    let grants = blocking_for(&[ObjectKind::Grant], &[ObjectState::Active]);
    if !grants.is_empty() {
        open.push(OpenCondition {
            check: ClosureCheck::GrantsRevokedOrExpired,
            blocking: grants,
        });
    }
    let leases = blocking_for(&[ObjectKind::BudgetLease], &[ObjectState::Active]);
    if !leases.is_empty() {
        open.push(OpenCondition {
            check: ClosureCheck::BudgetReservationsReleased,
            blocking: leases,
        });
    }
    let branches = blocking_for(&[ObjectKind::Branch], &[ObjectState::Active]);
    if !branches.is_empty() {
        open.push(OpenCondition {
            check: ClosureCheck::BranchesJoinedDetachedOrAborted,
            blocking: branches,
        });
    }
    let continuations = blocking_for(&[ObjectKind::Continuation], &[ObjectState::Active]);
    if !continuations.is_empty() {
        open.push(OpenCondition {
            check: ClosureCheck::ContinuationsConsumedTransferredOrExpired,
            blocking: continuations,
        });
    }

    for (satisfied, check) in [
        (
            evidence.irreversible_effects_recorded,
            ClosureCheck::IrreversibleEffectsRecorded,
        ),
        (
            evidence.compensations_settled,
            ClosureCheck::CompensationsCompleteOrWaived,
        ),
        (
            evidence.disputes_exported,
            ClosureCheck::UnresolvedDisputesExported,
        ),
        (
            evidence.retention_scheduled,
            ClosureCheck::RetentionPoliciesScheduled,
        ),
        (
            evidence.terminal_result_emitted,
            ClosureCheck::TerminalResultEmitted,
        ),
    ] {
        if !satisfied {
            open.push(OpenCondition {
                check,
                blocking: BTreeSet::new(),
            });
        }
    }

    open.sort_by_key(|condition| condition.check);
    if open.is_empty() {
        ClosureOutcome::Closed
    } else {
        ClosureOutcome::PartialTerminal { open }
    }
}

/// A claim in the semantic dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClaimId(pub String);

impl ClaimId {
    pub fn new(id: impl Into<String>) -> Self {
        ClaimId(id.into())
    }
}

impl std::fmt::Display for ClaimId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which claims rest on which. Supplied by a caller: this crate cannot read an argument.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivationGraph {
    premises: BTreeMap<ClaimId, BTreeSet<ClaimId>>,
    live_conclusions: BTreeSet<ClaimId>,
}

impl DerivationGraph {
    pub fn new() -> Self {
        DerivationGraph::default()
    }

    /// `conclusion` rests on `premises`.
    pub fn deriving(
        mut self,
        conclusion: impl Into<String>,
        premises: &[&str],
    ) -> Self {
        self.premises.insert(
            ClaimId::new(conclusion),
            premises.iter().map(|p| ClaimId::new(*p)).collect(),
        );
        self
    }

    /// A conclusion the thread is still relying on. Forgetting anything it rests on is refused.
    pub fn live(mut self, conclusion: impl Into<String>) -> Self {
        self.live_conclusions.insert(ClaimId::new(conclusion));
        self
    }

    pub fn live_conclusions(&self) -> &BTreeSet<ClaimId> {
        &self.live_conclusions
    }

    /// Every claim a live conclusion rests on, transitively.
    pub fn protected(&self) -> BTreeMap<ClaimId, BTreeSet<ClaimId>> {
        let mut out: BTreeMap<ClaimId, BTreeSet<ClaimId>> = BTreeMap::new();
        for conclusion in &self.live_conclusions {
            let mut seen = BTreeSet::new();
            let mut queue = VecDeque::new();
            queue.push_back(conclusion.clone());
            while let Some(current) = queue.pop_front() {
                if !seen.insert(current.clone()) {
                    continue;
                }
                if let Some(premises) = self.premises.get(&current) {
                    for premise in premises {
                        out.entry(premise.clone())
                            .or_default()
                            .insert(conclusion.clone());
                        queue.push_back(premise.clone());
                    }
                }
            }
        }
        out
    }
}

/// What compaction is allowed to forget, and over what window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionPlan {
    /// Claims whose full record would be dropped in favour of a checkpoint.
    pub drop: BTreeSet<ClaimId>,
    /// The prefix being summarised, as a logical half-open interval. `bioprism-ledger`'s rule: a
    /// compaction that does not declare its retained window is not auditable afterwards.
    pub retained_from: LogicalTime,
    pub retained_to: LogicalTime,
}

impl CompactionPlan {
    pub fn new(retained_from: u64, retained_to: u64) -> Self {
        CompactionPlan {
            drop: BTreeSet::new(),
            retained_from: LogicalTime(retained_from),
            retained_to: LogicalTime(retained_to),
        }
    }

    pub fn dropping(mut self, claim: impl Into<String>) -> Self {
        self.drop.insert(ClaimId::new(claim));
        self
    }

    /// Refuse anything that would orphan a live conclusion, and refuse anything 23.44 says must
    /// survive: unresolved conflicts, retraction and supersession lineage, provenance roots.
    ///
    /// The first refusal is the one that matters and it names the conclusion.
    pub fn check(
        &self,
        derivations: &DerivationGraph,
        must_survive: &MustSurvive,
    ) -> Result<CompactionCertificate, CompactionRefusal> {
        if self.retained_to <= self.retained_from {
            return Err(CompactionRefusal::EmptyRetainedWindow {
                from: self.retained_from,
                to: self.retained_to,
            });
        }
        let protected = derivations.protected();
        let mut orphaned: BTreeMap<ClaimId, BTreeSet<ClaimId>> = BTreeMap::new();
        for claim in &self.drop {
            if let Some(conclusions) = protected.get(claim) {
                orphaned.insert(claim.clone(), conclusions.clone());
            }
            if derivations.live_conclusions.contains(claim) {
                orphaned
                    .entry(claim.clone())
                    .or_default()
                    .insert(claim.clone());
            }
        }
        if let Some((premise, conclusions)) = orphaned.into_iter().next() {
            return Err(CompactionRefusal::WouldOrphanConclusion {
                premise,
                conclusions,
            });
        }
        let unresolved: BTreeSet<ClaimId> = self
            .drop
            .intersection(&must_survive.unresolved_conflicts)
            .cloned()
            .collect();
        if !unresolved.is_empty() {
            return Err(CompactionRefusal::UnresolvedConflict {
                claims: unresolved,
            });
        }
        let lineage: BTreeSet<ClaimId> = self
            .drop
            .intersection(&must_survive.retraction_lineage)
            .cloned()
            .collect();
        if !lineage.is_empty() {
            return Err(CompactionRefusal::RetractionLineage { claims: lineage });
        }
        let held: BTreeSet<ClaimId> = self
            .drop
            .intersection(&must_survive.legal_hold)
            .cloned()
            .collect();
        if !held.is_empty() {
            return Err(CompactionRefusal::LegalHold { claims: held });
        }
        let digest = ContentHash::of_value(&json!({
            "drop": self.drop.iter().map(|c| c.0.clone()).collect::<Vec<_>>(),
            "retained_from": self.retained_from.0,
            "retained_to": self.retained_to.0,
        }))
        .map_err(|e| CompactionRefusal::Encoding(e.to_string()))?;
        Ok(CompactionCertificate {
            digest: digest.to_string(),
            dropped: self.drop.clone(),
            retained_from: self.retained_from,
            retained_to: self.retained_to,
            verification_after: VerificationStrength::PrefixCheckpointOnly,
        })
    }
}

/// Categories 23.44 and 23.48 require compaction to preserve.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MustSurvive {
    pub unresolved_conflicts: BTreeSet<ClaimId>,
    pub retraction_lineage: BTreeSet<ClaimId>,
    pub legal_hold: BTreeSet<ClaimId>,
}

impl MustSurvive {
    pub fn new() -> Self {
        MustSurvive::default()
    }

    pub fn conflict(mut self, claim: impl Into<String>) -> Self {
        self.unresolved_conflicts.insert(ClaimId::new(claim));
        self
    }

    pub fn lineage(mut self, claim: impl Into<String>) -> Self {
        self.retraction_lineage.insert(ClaimId::new(claim));
        self
    }

    pub fn held(mut self, claim: impl Into<String>) -> Self {
        self.legal_hold.insert(ClaimId::new(claim));
        self
    }
}

/// How strongly a compacted record can be verified afterwards.
///
/// `bioprism-ledger`'s rule, restated in the type: after compaction the strongest available check
/// is against the checkpoint, not against the original events. There is no variant meaning "as
/// strong as before", because there is no compaction for which that is true.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationStrength {
    /// Every original event is present and rehashable.
    Full,
    /// The prefix is summarised. A verifier can check the checkpoint and the suffix, and cannot
    /// re-derive the prefix.
    PrefixCheckpointOnly,
}

/// Permission to forget, with the window it declared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionCertificate {
    pub digest: String,
    pub dropped: BTreeSet<ClaimId>,
    pub retained_from: LogicalTime,
    pub retained_to: LogicalTime,
    pub verification_after: VerificationStrength,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CompactionRefusal {
    #[error("dropping {premise} would orphan live conclusion(s) {conclusions:?}")]
    WouldOrphanConclusion {
        premise: ClaimId,
        conclusions: BTreeSet<ClaimId>,
    },

    #[error("{claims:?} are alternatives in an unresolved conflict and must survive compaction")]
    UnresolvedConflict { claims: BTreeSet<ClaimId> },

    #[error("{claims:?} carry retraction or supersession lineage needed for audit")]
    RetractionLineage { claims: BTreeSet<ClaimId> },

    #[error("{claims:?} are under a legal or audit hold")]
    LegalHold { claims: BTreeSet<ClaimId> },

    #[error("retained window [{from:?}, {to:?}) is empty; a compaction that retains nothing is not auditable")]
    EmptyRetainedWindow {
        from: LogicalTime,
        to: LogicalTime,
    },

    #[error("canonical encoding failed: {0}")]
    Encoding(String),
}

/// A continuation's declared expiry terms (23.48).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContinuationTerms {
    pub id: ObjectId,
    pub created_at: LogicalTime,
    pub max_age: u64,
    pub owner: String,
    pub transferable: bool,
    pub open_commitments: BTreeSet<String>,
}

/// "Expired continuations cannot be resumed merely because bytes remain available."
///
/// The bytes are not the permission. This function takes no payload at all, which is the point.
pub fn resume_permitted(
    terms: &ContinuationTerms,
    requester: &str,
    as_of: LogicalTime,
) -> Result<(), LifecycleError> {
    if as_of.0.saturating_sub(terms.created_at.0) > terms.max_age {
        return Err(LifecycleError::ContinuationExpired {
            id: terms.id.clone(),
            age: as_of.0.saturating_sub(terms.created_at.0),
            max_age: terms.max_age,
        });
    }
    if requester != terms.owner && !terms.transferable {
        return Err(LifecycleError::ContinuationNotTransferable {
            id: terms.id.clone(),
            owner: terms.owner.clone(),
            requester: requester.to_string(),
        });
    }
    Ok(())
}

/// 23.48's eight-step molecule shutdown, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownStep {
    StopAdmissions,
    DrainOrTransferCommitments,
    RevokeNestedGrants,
    ReleaseResources,
    CloseSubscriptions,
    EmitFinalAttestations,
    ArchiveBoundConfiguration,
    DestroyEphemeralContext,
}

impl ShutdownStep {
    pub const SEQUENCE: [ShutdownStep; 8] = [
        ShutdownStep::StopAdmissions,
        ShutdownStep::DrainOrTransferCommitments,
        ShutdownStep::RevokeNestedGrants,
        ShutdownStep::ReleaseResources,
        ShutdownStep::CloseSubscriptions,
        ShutdownStep::EmitFinalAttestations,
        ShutdownStep::ArchiveBoundConfiguration,
        ShutdownStep::DestroyEphemeralContext,
    ];
}

/// Check a shutdown ran its steps in order and completely.
///
/// Order matters and 23.48 numbers the steps: revoking grants before draining commitments strands
/// an obligation whose holder can no longer act on it.
pub fn check_shutdown(performed: &[ShutdownStep]) -> Result<(), LifecycleError> {
    let mut expected = ShutdownStep::SEQUENCE.iter();
    for step in performed {
        match expected.next() {
            Some(want) if want == step => {}
            Some(want) => {
                return Err(LifecycleError::ShutdownOutOfOrder {
                    expected: *want,
                    found: *step,
                })
            }
            None => {
                return Err(LifecycleError::ShutdownOutOfOrder {
                    expected: ShutdownStep::DestroyEphemeralContext,
                    found: *step,
                })
            }
        }
    }
    if let Some(missing) = expected.next() {
        return Err(LifecycleError::ShutdownIncomplete { missing: *missing });
    }
    Ok(())
}

/// What a trust domain returns for a deletion request (23.48).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "attestation")]
pub enum DeletionAttestation {
    Deleted { participant: String },
    Unable { participant: String, reason: String },
    LegalHold { participant: String, basis: String },
}

impl DeletionAttestation {
    pub fn participant(&self) -> &str {
        match self {
            DeletionAttestation::Deleted { participant }
            | DeletionAttestation::Unable { participant, .. }
            | DeletionAttestation::LegalHold { participant, .. } => participant,
        }
    }
}

/// "The caller must not claim global deletion without evidence from every required participant."
///
/// Refuses on a missing attestation *and* on a present-but-negative one, naming which. A silent
/// participant and a participant that said no are different failures and produce different errors.
pub fn claim_global_deletion(
    required: &BTreeSet<String>,
    attestations: &[DeletionAttestation],
) -> Result<GlobalDeletion, LifecycleError> {
    let seen: BTreeMap<&str, &DeletionAttestation> = attestations
        .iter()
        .map(|a| (a.participant(), a))
        .collect();
    let missing: BTreeSet<String> = required
        .iter()
        .filter(|participant| !seen.contains_key(participant.as_str()))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(LifecycleError::DeletionAttestationMissing {
            participants: missing,
        });
    }
    let refused: Vec<DeletionAttestation> = required
        .iter()
        .filter_map(|participant| seen.get(participant.as_str()).copied())
        .filter(|a| !matches!(a, DeletionAttestation::Deleted { .. }))
        .cloned()
        .collect();
    if !refused.is_empty() {
        return Err(LifecycleError::DeletionNotUniversal { refused });
    }
    Ok(GlobalDeletion {
        participants: required.clone(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalDeletion {
    pub participants: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LifecycleError {
    #[error("no object {id} in this graph")]
    NoSuchObject { id: ObjectId },

    #[error("{id} may not go from {from:?} to {to:?}")]
    IllegalTransition {
        id: ObjectId,
        from: ObjectState,
        to: ObjectState,
    },

    #[error("continuation {id} is {age} old and its maximum age is {max_age}")]
    ContinuationExpired {
        id: ObjectId,
        age: u64,
        max_age: u64,
    },

    #[error("continuation {id} belongs to {owner} and is not transferable to {requester}")]
    ContinuationNotTransferable {
        id: ObjectId,
        owner: String,
        requester: String,
    },

    #[error("shutdown step {found:?} ran where {expected:?} was required")]
    ShutdownOutOfOrder {
        expected: ShutdownStep,
        found: ShutdownStep,
    },

    #[error("shutdown stopped before {missing:?}")]
    ShutdownIncomplete { missing: ShutdownStep },

    #[error("no deletion attestation from {participants:?}")]
    DeletionAttestationMissing { participants: BTreeSet<String> },

    #[error("deletion is not global: {refused:?}")]
    DeletionNotUniversal {
        refused: Vec<DeletionAttestation>,
    },
}
