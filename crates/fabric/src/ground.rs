//! Distributed epistemic CRDT and scoped common ground.
//!
//! Blueprint 23.44.
//!
//! # Common ground is not the intersection of two belief sets
//!
//! This is the module's reason to exist, and it is the thing 23.44 assumes without stating. 23.44
//! gives the signature `common-ground(thread, role-set, purpose, clearance, time)` and five worked
//! examples and never says what the function computes. The naive reading — the propositions every
//! member believes — is wrong in a way that breaks coordination: `A` knows `X`, `B` knows `X`, and
//! neither knows the other knows. Neither can act on `X` in a way that depends on the other
//! acting, and no amount of intersecting belief sets reveals that.
//!
//! [`GroundLevel`] models three levels and [`common_ground`] computes all of them:
//!
//! - [`GroundLevel::Distributed`] — everyone believes it. This is the intersection, and it is the
//!   weakest level, not the answer.
//! - [`GroundLevel::Mutual`] — everyone believes it *and* has evidence that the others do, to some
//!   finite depth. Depth is derived from causal-parent chains: an operation by `B` whose ancestry
//!   contains `A`'s assertion is evidence that `B` saw it.
//! - [`GroundLevel::PubliclyGrounded`] — a single operation, visible to every member of the scope,
//!   that every member has built on. This is what a shared channel buys.
//!
//! **The honest caveat, named rather than buried:** the third level is not common knowledge in the
//! logician's sense. Unbounded `k`-order knowledge is not derivable from any finite set of
//! messages — that is the coordinated-attack result, and no CRDT repairs it. What
//! [`GroundLevel::PubliclyGrounded`] means is precisely "there is a public event and every member
//! has an operation causally after it", which is the strongest thing an event log can witness. The
//! variant is named for what it is rather than for what a reader might hope it is.
//!
//! # The merge
//!
//! [`Replica::merge`] is a validated set union, so it is commutative, associative and idempotent,
//! and [`crate::ground`]'s tests check all three on a case with a retraction, a supersession and a
//! genuine conflict rather than on three disjoint singletons. Causal validation is the one thing
//! that makes it more than a `BTreeSet::extend`: an operation whose parents are absent is held in
//! [`MergeReport::pending`] rather than admitted or dropped, and it is admitted later when its
//! parents arrive. Holding is what preserves commutativity under reordered delivery — dropping
//! would not.
//!
//! # Not implemented
//!
//! No signatures, so "resistance to forged events" is out of reach and the crate says so;
//! validation here is causal, not cryptographic. No network and no partition — [`ConsistencyMode`]
//! is a declaration a caller makes, not a state this crate detects. No wall clock: `time` in
//! 23.44's signature is a [`crate::reputation::LogicalTime`] the caller supplies.

use crate::flow::{FlowDecision, Labelling, Principal};
use crate::reputation::LogicalTime;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// A proposition identifier. Opaque: this crate never interprets what a proposition says.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Proposition(pub String);

impl Proposition {
    pub fn new(text: impl Into<String>) -> Self {
        Proposition(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A content-derived operation identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpId(pub String);

/// 23.44's nine epistemic operations.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "op")]
pub enum OpKind {
    Observe {
        proposition: Proposition,
        value: String,
        locator: String,
    },
    Assert {
        proposition: Proposition,
        value: String,
        /// Confidence in basis points, integer so replicas cannot disagree by a rounding step.
        confidence_bp: u32,
    },
    Assume {
        proposition: Proposition,
        expiry: LogicalTime,
    },
    Challenge {
        target: OpId,
        counterevidence: String,
    },
    Attest {
        target: OpId,
        method: String,
        passed: bool,
    },
    Retract {
        target: OpId,
        reason: String,
    },
    Supersede {
        old: OpId,
        relation: String,
        proposition: Proposition,
        value: String,
    },
    Expire {
        target: OpId,
        policy: String,
    },
    Resolve {
        dispute: Proposition,
        decision: String,
        basis: String,
    },
}

impl OpKind {
    /// The proposition this operation is about, when it is about one directly. `Challenge`,
    /// `Attest`, `Retract` and `Expire` target an *operation*, so their proposition is resolved
    /// through the replica rather than read off the operation.
    pub fn direct_proposition(&self) -> Option<&Proposition> {
        match self {
            OpKind::Observe { proposition, .. }
            | OpKind::Assert { proposition, .. }
            | OpKind::Assume { proposition, .. }
            | OpKind::Supersede { proposition, .. } => Some(proposition),
            OpKind::Resolve { dispute, .. } => Some(dispute),
            _ => None,
        }
    }

    fn target(&self) -> Option<&OpId> {
        match self {
            OpKind::Challenge { target, .. }
            | OpKind::Attest { target, .. }
            | OpKind::Retract { target, .. }
            | OpKind::Expire { target, .. } => Some(target),
            OpKind::Supersede { old, .. } => Some(old),
            _ => None,
        }
    }
}

/// One epistemic operation, with everything 23.44 requires of it except a signature.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EpistemicOp {
    pub id: OpId,
    pub author: String,
    pub parents: BTreeSet<OpId>,
    pub clock: LogicalTime,
    pub schema: String,
    pub label: Labelling,
    pub kind: OpKind,
}

impl EpistemicOp {
    /// Build an operation whose identifier is derived from its content.
    ///
    /// 23.44 asks for "a stable identifier"; a content-derived one is stable *and* makes forging a
    /// distinct operation with the same id impossible without also making the contents equal. It
    /// is not a signature and does not authenticate the author.
    pub fn new(
        author: impl Into<String>,
        parents: impl IntoIterator<Item = OpId>,
        clock: LogicalTime,
        label: Labelling,
        kind: OpKind,
    ) -> Result<Self, GroundError> {
        let author = author.into();
        let parents: BTreeSet<OpId> = parents.into_iter().collect();
        let payload = json!({
            "author": author,
            "parents": parents.iter().map(|p| p.0.clone()).collect::<Vec<_>>(),
            "clock": clock.0,
            "kind": serde_json::to_value(&kind).map_err(|e| GroundError::Encoding(e.to_string()))?,
        });
        let hash =
            ContentHash::of_value(&payload).map_err(|e| GroundError::Encoding(e.to_string()))?;
        Ok(EpistemicOp {
            id: OpId(hash.to_string()),
            author,
            parents,
            clock,
            schema: "bioprism-fabric-epistemic-op/0.1".to_string(),
            label,
            kind,
        })
    }
}

/// An append-only replica of the epistemic event set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Replica {
    admitted: BTreeMap<OpId, EpistemicOp>,
    /// Operations whose causal parents have not arrived. Held, not dropped: dropping would make
    /// merge order-dependent, which is the one property this whole design exists to have.
    pending: BTreeMap<OpId, EpistemicOp>,
}

/// What one merge did.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergeReport {
    pub admitted: BTreeSet<OpId>,
    pub already_present: BTreeSet<OpId>,
    pub pending: BTreeSet<OpId>,
    /// An operation naming a parent that is neither admitted nor pending anywhere and that names
    /// itself, directly or transitively. A cycle in causal parents is not a partition; it is a
    /// malformed event.
    pub rejected_cyclic: BTreeSet<OpId>,
}

impl Replica {
    pub fn new() -> Self {
        Replica::default()
    }

    /// Append one operation. Idempotent by construction: identifiers are content-derived, so a
    /// duplicate delivery is the same key.
    pub fn append(&mut self, op: EpistemicOp) -> MergeReport {
        let mut report = MergeReport::default();
        if self.admitted.contains_key(&op.id) {
            report.already_present.insert(op.id);
            return report;
        }
        if op.parents.contains(&op.id) {
            report.rejected_cyclic.insert(op.id);
            return report;
        }
        self.pending.insert(op.id.clone(), op);
        self.drain_pending(&mut report);
        report
    }

    fn drain_pending(&mut self, report: &mut MergeReport) {
        loop {
            let ready: Vec<OpId> = self
                .pending
                .iter()
                .filter(|(_, op)| op.parents.iter().all(|p| self.admitted.contains_key(p)))
                .map(|(id, _)| id.clone())
                .collect();
            if ready.is_empty() {
                break;
            }
            for id in ready {
                if let Some(op) = self.pending.remove(&id) {
                    report.admitted.insert(id.clone());
                    self.admitted.insert(id, op);
                }
            }
        }
        report.pending = self.pending.keys().cloned().collect();
    }

    /// Validated union with another replica.
    ///
    /// Commutative, associative and idempotent. The proof obligation is discharged by tests rather
    /// than by assertion; see `merge_is_commutative_on_a_case_with_a_conflict_and_a_retraction`.
    pub fn merge(&mut self, other: &Replica) -> MergeReport {
        let mut report = MergeReport::default();
        for op in other.admitted.values().chain(other.pending.values()) {
            if self.admitted.contains_key(&op.id) {
                report.already_present.insert(op.id.clone());
            } else if !self.pending.contains_key(&op.id) {
                if op.parents.contains(&op.id) {
                    report.rejected_cyclic.insert(op.id.clone());
                } else {
                    self.pending.insert(op.id.clone(), op.clone());
                }
            }
        }
        self.drain_pending(&mut report);
        report
    }

    pub fn get(&self, id: &OpId) -> Option<&EpistemicOp> {
        self.admitted.get(id)
    }

    pub fn admitted_ops(&self) -> impl Iterator<Item = &EpistemicOp> {
        self.admitted.values()
    }

    pub fn admitted_count(&self) -> usize {
        self.admitted.len()
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// A digest over the admitted set. Two replicas that have converged produce the same digest,
    /// whatever order they received operations in.
    pub fn digest(&self) -> Result<String, GroundError> {
        let ids: Vec<String> = self.admitted.keys().map(|id| id.0.clone()).collect();
        let hash = ContentHash::of_value(&json!({ "admitted": ids }))
            .map_err(|e| GroundError::Encoding(e.to_string()))?;
        Ok(hash.to_string())
    }

    /// Every operation causally at or before `id`, including `id` itself.
    pub fn ancestry(&self, id: &OpId) -> BTreeSet<OpId> {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(id.clone());
        while let Some(current) = queue.pop_front() {
            if !seen.insert(current.clone()) {
                continue;
            }
            if let Some(op) = self.admitted.get(&current) {
                for parent in &op.parents {
                    queue.push_back(parent.clone());
                }
            }
        }
        seen
    }

    /// The proposition an operation concerns, following `target` links to the operation it is
    /// about. Returns `None` for an operation whose target is not admitted.
    pub fn proposition_of(&self, id: &OpId) -> Option<&Proposition> {
        let mut current = id.clone();
        for _ in 0..self.admitted.len().saturating_add(1) {
            let op = self.admitted.get(&current)?;
            if let Some(proposition) = op.kind.direct_proposition() {
                return Some(proposition);
            }
            current = op.kind.target()?.clone();
        }
        None
    }

    /// 23.44's label-aware replication. A recipient receives only what its clearance admits, and
    /// learns how many operations it did not receive.
    pub fn replicate_for(&self, recipient: &Principal) -> RedactedReplica {
        let mut out = Replica::new();
        let mut omitted = 0usize;
        let mut ordered: Vec<&EpistemicOp> = self.admitted.values().collect();
        ordered.sort_by_key(|op| (op.clock, op.id.clone()));
        for op in ordered {
            match op.label.flows_to(&recipient.clearance) {
                FlowDecision::Permitted => {
                    out.append(op.clone());
                }
                FlowDecision::Refused { .. } => omitted += 1,
            }
        }
        RedactedReplica {
            replica: out,
            omitted_count: omitted,
            recipient: recipient.id.clone(),
        }
    }
}

/// A replica projected for one recipient, with an omission count so the recipient knows its view is
/// incomplete. 23.44: "Redacted replicas include omission proofs or counts where useful so a
/// recipient knows its view is incomplete." A count, not a proof — this crate has no cryptography.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactedReplica {
    pub recipient: String,
    pub replica: Replica,
    pub omitted_count: usize,
}

impl RedactedReplica {
    pub fn view_is_complete(&self) -> bool {
        self.omitted_count == 0
    }
}

/// 23.44's eleven derived epistemic states.
///
/// "These labels are derived from event structure and policy. They are not globally writable
/// fields" — which is why there is no setter and no variant a caller can assign.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DerivedState {
    Observed,
    Asserted,
    Supported,
    Contradicted,
    Disputed,
    ProvisionallyAccepted,
    VerifiedUnderMethod,
    Superseded,
    Retracted,
    Stale,
    Unresolved,
}

/// The interpretation policy 23.44 leaves to the implementation.
///
/// Deterministic and total: every proposition with at least one operation gets exactly one state,
/// and the order of the checks below *is* the policy. Written as an ordered list rather than a
/// scoring function so that a disagreement about the outcome is a disagreement about one ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationPolicy {
    /// Assumptions older than this many logical units are [`DerivedState::Stale`].
    pub staleness_horizon: u64,
}

impl InterpretationPolicy {
    pub fn new(staleness_horizon: u64) -> Self {
        InterpretationPolicy { staleness_horizon }
    }
}

/// Derive the state of every proposition in a replica, as of a logical time.
pub fn interpret(
    replica: &Replica,
    policy: InterpretationPolicy,
    as_of: LogicalTime,
) -> BTreeMap<Proposition, DerivedState> {
    let mut retracted_targets = BTreeSet::new();
    let mut superseded_targets = BTreeSet::new();
    let mut challenged = BTreeSet::new();
    let mut attested_pass = BTreeSet::new();
    let mut resolved = BTreeSet::new();
    let mut values: BTreeMap<Proposition, BTreeSet<String>> = BTreeMap::new();
    let mut states: BTreeMap<Proposition, DerivedState> = BTreeMap::new();

    for op in replica.admitted_ops() {
        match &op.kind {
            OpKind::Retract { target, .. } => {
                retracted_targets.insert(target.clone());
            }
            OpKind::Supersede { old, .. } => {
                superseded_targets.insert(old.clone());
            }
            OpKind::Challenge { target, .. } => {
                challenged.insert(target.clone());
            }
            OpKind::Attest { target, passed, .. } if *passed => {
                attested_pass.insert(target.clone());
            }
            OpKind::Resolve { dispute, .. } => {
                resolved.insert(dispute.clone());
            }
            _ => {}
        }
    }

    for op in replica.admitted_ops() {
        let proposition = match op.kind.direct_proposition() {
            Some(p) => p.clone(),
            None => continue,
        };
        match &op.kind {
            OpKind::Observe { value, .. }
            | OpKind::Assert { value, .. }
            | OpKind::Supersede { value, .. } => {
                values.entry(proposition.clone()).or_default().insert(value.clone());
            }
            _ => {}
        }
        let state = if retracted_targets.contains(&op.id) {
            DerivedState::Retracted
        } else if superseded_targets.contains(&op.id) {
            DerivedState::Superseded
        } else if attested_pass.contains(&op.id) {
            DerivedState::VerifiedUnderMethod
        } else if challenged.contains(&op.id) {
            DerivedState::Disputed
        } else {
            match &op.kind {
                OpKind::Observe { .. } => DerivedState::Observed,
                OpKind::Assert { .. } => DerivedState::Asserted,
                OpKind::Assume { expiry, .. } => {
                    if as_of.0.saturating_sub(op.clock.0) > policy.staleness_horizon
                        || as_of >= *expiry
                    {
                        DerivedState::Stale
                    } else {
                        DerivedState::ProvisionallyAccepted
                    }
                }
                OpKind::Supersede { .. } => DerivedState::Asserted,
                OpKind::Resolve { .. } => DerivedState::Supported,
                _ => DerivedState::Unresolved,
            }
        };
        let entry = states.entry(proposition).or_insert(state);
        *entry = (*entry).max(state);
    }

    for (proposition, distinct) in &values {
        if distinct.len() > 1 && !resolved.contains(proposition) {
            states.insert(proposition.clone(), DerivedState::Contradicted);
        }
    }

    states
}

/// A preserved disagreement. 23.44: "A summarizer may render the conflict but cannot collapse it
/// without a resolution event."
///
/// `status` is not a settable field: [`conflicts`] recomputes it from the replica every time, so
/// the only way to move a conflict out of `unresolved` is to append an
/// [`OpKind::Resolve`] operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Conflict {
    pub proposition: Proposition,
    pub alternatives: BTreeSet<OpId>,
    pub decisive_evidence_needed: BTreeSet<String>,
    pub resolved_by: Option<OpId>,
}

impl Conflict {
    pub fn is_unresolved(&self) -> bool {
        self.resolved_by.is_none()
    }
}

/// Every proposition on which two admitted operations assert incompatible values.
pub fn conflicts(replica: &Replica) -> Vec<Conflict> {
    let mut by_proposition: BTreeMap<Proposition, BTreeMap<String, BTreeSet<OpId>>> =
        BTreeMap::new();
    let mut resolutions: BTreeMap<Proposition, OpId> = BTreeMap::new();
    for op in replica.admitted_ops() {
        match &op.kind {
            OpKind::Observe {
                proposition, value, ..
            }
            | OpKind::Assert {
                proposition, value, ..
            }
            | OpKind::Supersede {
                proposition, value, ..
            } => {
                by_proposition
                    .entry(proposition.clone())
                    .or_default()
                    .entry(value.clone())
                    .or_default()
                    .insert(op.id.clone());
            }
            OpKind::Resolve { dispute, .. } => {
                resolutions.insert(dispute.clone(), op.id.clone());
            }
            _ => {}
        }
    }
    by_proposition
        .into_iter()
        .filter(|(_, values)| values.len() > 1)
        .map(|(proposition, values)| Conflict {
            decisive_evidence_needed: values.keys().map(|v| format!("evidence-for:{v}")).collect(),
            alternatives: values.into_values().flatten().collect(),
            resolved_by: resolutions.get(&proposition).cloned(),
            proposition,
        })
        .collect()
}

/// The scope 23.44's `common-ground(thread, role-set, purpose, clearance, time)` is computed over.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub thread: String,
    pub members: Vec<Principal>,
    pub purpose: String,
    pub as_of: LogicalTime,
}

/// How strongly a proposition is held in common.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "level")]
pub enum GroundLevel {
    /// Every member believes it. Nobody necessarily knows that anybody else does. The intersection
    /// of belief sets, and the level at which coordination is not yet possible.
    Distributed,
    /// Every member believes it and has causal evidence that every other member does, to `depth`
    /// orders of knowledge.
    Mutual { depth: u8 },
    /// A single operation admitted by every member's clearance, on which every member has built.
    /// The strongest thing an event log witnesses; see the module docs for why it is not called
    /// common knowledge.
    PubliclyGrounded,
}

/// The result of a common-ground computation, with each level kept separate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommonGround {
    pub levels: BTreeMap<Proposition, GroundLevel>,
    /// Propositions some but not all members believe. Not common ground at any level, and named
    /// because a coordinator needs to know what to broadcast.
    pub asymmetric: BTreeMap<Proposition, BTreeSet<String>>,
}

impl CommonGround {
    pub fn at_least(&self, level: GroundLevel) -> BTreeSet<Proposition> {
        self.levels
            .iter()
            .filter(|(_, actual)| **actual >= level)
            .map(|(p, _)| p.clone())
            .collect()
    }

    /// The plain intersection of belief sets. Exposed so a caller can see for itself that this is
    /// a *superset* of what is mutually known, which is the module's central claim.
    pub fn distributed(&self) -> BTreeSet<Proposition> {
        self.levels.keys().cloned().collect()
    }
}

/// Compute scoped common ground.
///
/// Three predicates, each strictly stronger than the last:
///
/// - **Believes.** The replica projected for the member's clearance contains an unretracted
///   `Observe`, `Assert` or `Supersede` on the proposition at or before `as_of`. Every member
///   believing it is [`GroundLevel::Distributed`] — the intersection of belief sets, and no more.
/// - **Mutually known.** For every ordered pair, the observer *authored* an operation on the
///   proposition whose causal ancestry contains one the other authored. Authorship is what makes
///   this stronger than belief: seeing an operation in a shared log says nothing about whether the
///   other party saw yours, and only an operation you wrote after theirs is evidence that you did.
/// - **Publicly grounded.** There is a single operation that every member's clearance admits and
///   that lies in the ancestry of *every* operation any member authored on the proposition. Not
///   merely one everyone built on eventually: one that every member's belief *originates* from. If
///   two members formed the belief independently and only cross-referenced afterwards, they are
///   mutually informed and there was no public event, which is exactly the distinction this level
///   exists to draw.
pub fn common_ground(replica: &Replica, scope: &Scope) -> CommonGround {
    let mut retracted = BTreeSet::new();
    for op in replica.admitted_ops() {
        if let OpKind::Retract { target, .. } = &op.kind {
            retracted.insert(target.clone());
        }
    }

    let views: BTreeMap<String, RedactedReplica> = scope
        .members
        .iter()
        .map(|member| (member.id.clone(), replica.replicate_for(member)))
        .collect();

    let mut visible: BTreeMap<Proposition, BTreeSet<String>> = BTreeMap::new();
    let mut authored: BTreeMap<Proposition, BTreeMap<String, BTreeSet<OpId>>> = BTreeMap::new();

    for (member_id, view) in &views {
        for op in view.replica.admitted_ops() {
            if retracted.contains(&op.id) || op.clock > scope.as_of {
                continue;
            }
            if !matches!(
                op.kind,
                OpKind::Observe { .. } | OpKind::Assert { .. } | OpKind::Supersede { .. }
            ) {
                continue;
            }
            if let Some(proposition) = op.kind.direct_proposition() {
                visible
                    .entry(proposition.clone())
                    .or_default()
                    .insert(member_id.clone());
                if &op.author == member_id {
                    authored
                        .entry(proposition.clone())
                        .or_default()
                        .entry(member_id.clone())
                        .or_default()
                        .insert(op.id.clone());
                }
            }
        }
    }

    let member_ids: BTreeSet<String> = scope.members.iter().map(|m| m.id.clone()).collect();
    let mut out = CommonGround::default();

    for (proposition, believers) in visible {
        if believers != member_ids {
            out.asymmetric.insert(proposition, believers);
            continue;
        }
        let empty = BTreeMap::new();
        let by_author = authored.get(&proposition).unwrap_or(&empty);
        let mut level = GroundLevel::Distributed;
        if mutually_known(replica, by_author, &member_ids) {
            level = GroundLevel::Mutual { depth: 2 };
            if public_anchor(replica, &views, by_author, &member_ids).is_some() {
                level = GroundLevel::PubliclyGrounded;
            }
        }
        out.levels.insert(proposition, level);
    }

    out
}

fn mutually_known(
    replica: &Replica,
    authored: &BTreeMap<String, BTreeSet<OpId>>,
    members: &BTreeSet<String>,
) -> bool {
    for observer in members {
        let observer_ops: &BTreeSet<OpId> = match authored.get(observer) {
            Some(ops) => ops,
            None => return false,
        };
        let observer_ancestry: BTreeSet<OpId> = observer_ops
            .iter()
            .flat_map(|id| replica.ancestry(id))
            .collect();
        for other in members {
            if other == observer {
                continue;
            }
            let other_ops = match authored.get(other) {
                Some(ops) => ops,
                None => return false,
            };
            if !other_ops.iter().any(|id| observer_ancestry.contains(id)) {
                return false;
            }
        }
    }
    true
}

fn public_anchor(
    replica: &Replica,
    views: &BTreeMap<String, RedactedReplica>,
    authored: &BTreeMap<String, BTreeSet<OpId>>,
    members: &BTreeSet<String>,
) -> Option<OpId> {
    let candidates: BTreeSet<OpId> = authored.values().flatten().cloned().collect();
    candidates.into_iter().find(|candidate| {
        members.iter().all(|member| {
            views
                .get(member)
                .map(|view| view.replica.get(candidate).is_some())
                .unwrap_or(false)
                && authored
                    .get(member)
                    .map(|ops| {
                        ops.iter()
                            .all(|id| replica.ancestry(id).contains(candidate))
                    })
                    .unwrap_or(false)
        })
    })
}

/// 23.44's four partition consistency modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsistencyMode {
    ReadOnly,
    LocalTentative,
    BoundedEffects,
    StopAndSynchronize,
}

/// "Security-sensitive grants and irreversible effects should not rely only on eventual epistemic
/// convergence."
///
/// The one place this module says no to something. E3 and above need a mode that is not relying on
/// convergence, which leaves read-only and stop-and-synchronize.
pub fn may_proceed_under_partition(
    class: crate::effect::Irreversibility,
    mode: ConsistencyMode,
) -> Result<(), GroundError> {
    let relies_on_convergence = matches!(
        mode,
        ConsistencyMode::LocalTentative | ConsistencyMode::BoundedEffects
    );
    if class >= crate::effect::Irreversibility::E3 && relies_on_convergence {
        return Err(GroundError::EffectRequiresConvergedView { class, mode });
    }
    Ok(())
}

/// 23.44's eight evidence-weighting factors.
///
/// Weighting is a policy input, and the one rule this crate enforces on it is the last line of
/// 23.44's trust section: "Agent reputation never substitutes for missing evidence."
/// [`Weighting::weigh`] returns `None` when there is no evidence, whatever the reputation term
/// says, so a highly-reputed participant asserting something unsupported weighs nothing rather
/// than something small.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Weighting {
    pub directness_bp: u32,
    pub method_bp: u32,
    pub freshness_bp: u32,
    pub independence_bp: u32,
    pub calibration_bp: u32,
    pub conflict_of_interest_penalty_bp: u32,
    pub custody_bp: u32,
    pub domain_validity_bp: u32,
}

impl Weighting {
    /// `evidence_count` is the number of supporting operations. Zero means no evidence.
    pub fn weigh(&self, evidence_count: usize) -> Option<u32> {
        if evidence_count == 0 {
            return None;
        }
        let positive = self.directness_bp
            + self.method_bp
            + self.freshness_bp
            + self.independence_bp
            + self.calibration_bp
            + self.custody_bp
            + self.domain_validity_bp;
        Some(positive.saturating_sub(self.conflict_of_interest_penalty_bp))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GroundError {
    #[error("canonical encoding failed: {0}")]
    Encoding(String),

    #[error("a {class:?} effect may not proceed under {mode:?}: it would rely on eventual epistemic convergence")]
    EffectRequiresConvergedView {
        class: crate::effect::Irreversibility,
        mode: ConsistencyMode,
    },
}
