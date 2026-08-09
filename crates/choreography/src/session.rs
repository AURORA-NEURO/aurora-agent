//! Multiparty session types and projection.
//!
//! Blueprint 23.06: a choreography is written once as a *global* type describing the whole
//! conversation, and the compiler projects it into a *local* type per role, so that no central
//! prompt is the only source of coordination truth.
//!
//! The theorem this module exists to make available is the classic multiparty session-types
//! result: if the global type is well formed, the projected local types cannot deadlock. The
//! design consequence is that projection is not a fallible operation exposed to callers.
//! [`WellFormedGlobal::check`] performs the well-formedness analysis *by* projecting every role,
//! so a global type that survives the check has, by construction, a local type for each of its
//! roles; [`WellFormedGlobal::project`] then cannot fail. An ill-formed global type never yields a
//! local type at all — it yields a [`ProtocolError`] naming the role and the choice that the role
//! could not follow. The failure mode being avoided is a projection that "succeeds" into a local
//! type describing an obligation no participant can discharge.
//!
//! Deliberately not implemented: subtyping between local types, asynchronous subtyping,
//! parallel composition (23.06's `par` block — its projection interacts with the channel model in
//! [`crate::check`] and would need interleaving semantics this crate does not have),
//! parameterised and indexed roles, delegation of a session over a session, dynamic role
//! admission, and the timeout, budget-guard and loop-bound annotations 23.06 attaches to
//! iteration. Loop bounds live in the model checker instead, where they are an analysis parameter
//! rather than a type. The merge operator is the standard one over external choices; where it
//! fails the result is [`ProtocolError::UninformedOfChoice`], never a silently repaired
//! projection.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A participant's protocol role. Not a participant: 23.06 projects onto roles, and 23.12 binds
/// agents to roles afterwards, so the same choreography survives a rebind (23.22).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Role(String);

impl Role {
    pub fn new(name: impl Into<String>) -> Self {
        Role(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Role {
    fn from(name: &str) -> Self {
        Role(name.to_string())
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The branch selector of an interaction. The payload type is deliberately absent: this module
/// checks control flow, and 23.23 owns schema negotiation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    pub fn new(name: impl Into<String>) -> Self {
        Label(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Label {
    fn from(name: &str) -> Self {
        Label(name.to_string())
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// One arm of a global choice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalBranch {
    pub label: Label,
    pub continuation: GlobalType,
}

/// The global choreography: the conversation as an outside observer would write it.
///
/// [`GlobalType::Interaction`] covers both the sequence and the choice of 23.06 — a single-branch
/// interaction is a plain message, a multi-branch one is a choice whose chooser is `from`. Making
/// the chooser syntactically the sender is how "choices have an unambiguous decision owner"
/// becomes unrepresentable-otherwise rather than a check that could be forgotten.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum GlobalType {
    /// The conversation is over. Every role is at its terminal state.
    End,
    /// `from` chooses a label and sends it to `to`; the protocol continues per branch.
    Interaction {
        from: Role,
        to: Role,
        branches: Vec<GlobalBranch>,
    },
    /// Iteration. 23.06 requires loops to carry a bound; the bound lives in the model checker
    /// ([`crate::check::ExplorationBound`]) because it is an analysis parameter, not a type.
    Rec { var: String, body: Box<GlobalType> },
    /// A recursion variable, jumping back to its binding [`GlobalType::Rec`].
    Var { var: String },
}

impl GlobalType {
    /// `from -> to: label . continuation`.
    pub fn message(
        from: impl Into<Role>,
        to: impl Into<Role>,
        label: impl Into<Label>,
        continuation: GlobalType,
    ) -> Self {
        GlobalType::Interaction {
            from: from.into(),
            to: to.into(),
            branches: vec![GlobalBranch {
                label: label.into(),
                continuation,
            }],
        }
    }

    /// `from -> to { label_i . G_i }`, with `from` as the chooser.
    pub fn choice(
        from: impl Into<Role>,
        to: impl Into<Role>,
        branches: Vec<(Label, GlobalType)>,
    ) -> Self {
        GlobalType::Interaction {
            from: from.into(),
            to: to.into(),
            branches: branches
                .into_iter()
                .map(|(label, continuation)| GlobalBranch {
                    label,
                    continuation,
                })
                .collect(),
        }
    }

    pub fn rec(var: impl Into<String>, body: GlobalType) -> Self {
        GlobalType::Rec {
            var: var.into(),
            body: Box::new(body),
        }
    }

    pub fn var(var: impl Into<String>) -> Self {
        GlobalType::Var { var: var.into() }
    }

    /// Every role named anywhere in the protocol.
    pub fn roles(&self) -> BTreeSet<Role> {
        let mut found = BTreeSet::new();
        self.collect_roles(&mut found);
        found
    }

    fn collect_roles(&self, into: &mut BTreeSet<Role>) {
        match self {
            GlobalType::End | GlobalType::Var { .. } => {}
            GlobalType::Interaction { from, to, branches } => {
                into.insert(from.clone());
                into.insert(to.clone());
                for branch in branches {
                    branch.continuation.collect_roles(into);
                }
            }
            GlobalType::Rec { body, .. } => body.collect_roles(into),
        }
    }

    /// Checks well-formedness and returns the witness that carries the projections.
    pub fn well_formed(self) -> Result<WellFormedGlobal, ProtocolError> {
        WellFormedGlobal::check(self)
    }
}

/// One arm of a local choice.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct LocalBranch {
    pub label: Label,
    pub continuation: LocalType,
}

/// A single role's view of the choreography: what it must send, what it must be ready to receive.
///
/// Ordered and hashable because [`crate::check`] uses residual local types as the program
/// counters of its transition system and needs them as map keys.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "node", rename_all = "snake_case")]
pub enum LocalType {
    /// This role has nothing left to do.
    End,
    /// Internal choice: this role picks a label and sends it. It is never blocked here.
    Select { to: Role, branches: Vec<LocalBranch> },
    /// External choice: this role waits, and must handle every label the peer may send.
    Offer { from: Role, branches: Vec<LocalBranch> },
    Rec { var: String, body: Box<LocalType> },
    Var { var: String },
}

impl LocalType {
    /// Whether this role ever sends or receives. A role that neither sends nor receives projects
    /// to [`LocalType::End`], which is why recursion with no local action collapses instead of
    /// producing `rec X. X` — a term that would loop forever without a transition.
    fn has_action(&self) -> bool {
        match self {
            LocalType::End | LocalType::Var { .. } => false,
            LocalType::Select { .. } | LocalType::Offer { .. } => true,
            LocalType::Rec { body, .. } => body.has_action(),
        }
    }

    /// Unfolds a top-level `rec X. B` into `B[rec X. B / X]`.
    ///
    /// The substituted tail is the original `rec` term, so a loop returns to a syntactically
    /// identical state. That is what keeps the model checker's state space finite for recursive
    /// protocols without any widening or abstraction.
    ///
    /// Projection of a well-formed global type is guarded, so one round always exposes a `select`
    /// or an `offer`. Hand-written local types need not be, and `rec X. rec Y. X` would unfold
    /// forever, so the loop stops at [`UNFOLD_LIMIT`] and returns the still-folded term. The model
    /// checker then finds no transition for that role and reports it as stuck, which is the
    /// truthful reading of a recursion that never produces an action.
    pub fn unfold(&self) -> LocalType {
        let mut current = self.clone();
        for _ in 0..UNFOLD_LIMIT {
            let next = match &current {
                LocalType::Rec { var, body } => substitute(body, var, &current),
                _ => return current,
            };
            current = next;
        }
        current
    }
}

/// How many times [`LocalType::unfold`] will rewrite a nested recursion before giving up.
pub const UNFOLD_LIMIT: usize = 64;

fn substitute(target: &LocalType, var: &str, with: &LocalType) -> LocalType {
    match target {
        LocalType::End => LocalType::End,
        LocalType::Var { var: v } => {
            if v == var {
                with.clone()
            } else {
                LocalType::Var { var: v.clone() }
            }
        }
        LocalType::Select { to, branches } => LocalType::Select {
            to: to.clone(),
            branches: substitute_branches(branches, var, with),
        },
        LocalType::Offer { from, branches } => LocalType::Offer {
            from: from.clone(),
            branches: substitute_branches(branches, var, with),
        },
        LocalType::Rec { var: v, body } => {
            if v == var {
                target.clone()
            } else {
                LocalType::Rec {
                    var: v.clone(),
                    body: Box::new(substitute(body, var, with)),
                }
            }
        }
    }
}

fn substitute_branches(branches: &[LocalBranch], var: &str, with: &LocalType) -> Vec<LocalBranch> {
    branches
        .iter()
        .map(|branch| LocalBranch {
            label: branch.label.clone(),
            continuation: substitute(&branch.continuation, var, with),
        })
        .collect()
}

/// Why a global type has no coherent set of local types.
///
/// Every variant names the role and the syntactic site, because the point of rejecting at
/// projection time is that a human can repair the choreography. An error that said only
/// "not projectable" would push the diagnosis to runtime, which is the situation 23.45 exists
/// to avoid.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "error", rename_all = "snake_case")]
pub enum ProtocolError {
    #[error("{role} sends to itself; a self-communication has no projection because the same local type would have to both send and be waiting to receive")]
    SelfCommunication { role: Role },

    #[error("interaction {from} -> {to} offers no branches, so no label can ever be sent")]
    EmptyChoice { from: Role, to: Role },

    #[error("interaction {from} -> {to} repeats label {label}; the receiver could not tell the branches apart")]
    DuplicateLabel { from: Role, to: Role, label: Label },

    #[error("recursion variable {var} is not bound by any enclosing rec")]
    UnboundRecursionVariable { var: String },

    #[error("recursion on {var} is unguarded: the loop can return to itself without an interaction, so it makes no progress and admits no bounded analysis")]
    UnguardedRecursion { var: String },

    #[error("{role} is not informed of the choice {from} -> {to}: it must behave differently after {left} than after {right}, but no message tells it which was taken")]
    UninformedOfChoice {
        role: Role,
        from: Role,
        to: Role,
        left: Label,
        right: Label,
    },

    #[error("{role} does not appear in this protocol, so there is nothing to project onto it")]
    RoleNotInProtocol { role: Role },

    #[error("protocol is not canonically serialisable: {reason}")]
    NotSerialisable { reason: String },
}

/// A global type that has passed [`WellFormedGlobal::check`], carrying its projections.
///
/// The witness exists so that "well formed" is a property of a value rather than of a call that
/// happened earlier and might not have. It cannot be constructed by any other route, including
/// deserialisation: serde re-runs the check on the way in, so a `WellFormedGlobal` read from disk
/// is as trustworthy as one built in memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "GlobalType", into = "GlobalType")]
pub struct WellFormedGlobal {
    global: GlobalType,
    projections: BTreeMap<Role, LocalType>,
}

impl TryFrom<GlobalType> for WellFormedGlobal {
    type Error = ProtocolError;

    fn try_from(global: GlobalType) -> Result<Self, Self::Error> {
        WellFormedGlobal::check(global)
    }
}

impl From<WellFormedGlobal> for GlobalType {
    fn from(checked: WellFormedGlobal) -> Self {
        checked.global
    }
}

impl WellFormedGlobal {
    /// The whole of 23.06's static analysis, in the order the errors are most useful.
    ///
    /// Structural checks first (self-communication, empty and duplicated branches, recursion
    /// binding and guardedness), because those make the projection meaningless rather than
    /// merely impossible. Projection of every role second, because that is where "the role is
    /// not informed of the choice" is discovered, and there is no cheaper way to discover it.
    pub fn check(global: GlobalType) -> Result<Self, ProtocolError> {
        check_structure(&global, &mut Vec::new())?;

        let mut projections = BTreeMap::new();
        for role in global.roles() {
            let local = project(&global, &role)?;
            projections.insert(role, local);
        }

        Ok(WellFormedGlobal {
            global,
            projections,
        })
    }

    pub fn global(&self) -> &GlobalType {
        &self.global
    }

    /// The local type for a role, or [`ProtocolError::RoleNotInProtocol`].
    ///
    /// Fallible only in the trivial sense: the role is unknown. It cannot fail because the
    /// protocol is bad, since a bad protocol never produced a `WellFormedGlobal`.
    pub fn project(&self, role: &Role) -> Result<&LocalType, ProtocolError> {
        self.projections
            .get(role)
            .ok_or_else(|| ProtocolError::RoleNotInProtocol { role: role.clone() })
    }

    pub fn projections(&self) -> &BTreeMap<Role, LocalType> {
        &self.projections
    }

    pub fn roles(&self) -> impl Iterator<Item = &Role> {
        self.projections.keys()
    }

    /// A content hash over the global type, for pinning a running thread to its choreography
    /// version (23.06, "a running thread pins its choreography").
    ///
    /// Migration between versions is deliberately not implemented: 23.06 requires a migration
    /// mapping, compatibility checks, preserved commitments and a rollback path, and a digest
    /// comparison is only the trigger for that work, not a substitute for it.
    pub fn digest(&self) -> Result<String, ProtocolError> {
        let value =
            serde_json::to_value(&self.global).map_err(|error| ProtocolError::NotSerialisable {
                reason: error.to_string(),
            })?;
        ContentHash::of_value(&value)
            .map(|hash| hash.as_str().to_string())
            .map_err(|error| ProtocolError::NotSerialisable {
                reason: error.to_string(),
            })
    }
}

fn check_structure(global: &GlobalType, bound: &mut Vec<String>) -> Result<(), ProtocolError> {
    match global {
        GlobalType::End => Ok(()),
        GlobalType::Var { var } => {
            if bound.iter().any(|b| b == var) {
                Ok(())
            } else {
                Err(ProtocolError::UnboundRecursionVariable { var: var.clone() })
            }
        }
        GlobalType::Interaction { from, to, branches } => {
            if from == to {
                return Err(ProtocolError::SelfCommunication { role: from.clone() });
            }
            if branches.is_empty() {
                return Err(ProtocolError::EmptyChoice {
                    from: from.clone(),
                    to: to.clone(),
                });
            }
            let mut seen: BTreeSet<&Label> = BTreeSet::new();
            for branch in branches {
                if !seen.insert(&branch.label) {
                    return Err(ProtocolError::DuplicateLabel {
                        from: from.clone(),
                        to: to.clone(),
                        label: branch.label.clone(),
                    });
                }
                check_structure(&branch.continuation, bound)?;
            }
            Ok(())
        }
        GlobalType::Rec { var, body } => {
            if !guarded(body, var) {
                return Err(ProtocolError::UnguardedRecursion { var: var.clone() });
            }
            bound.push(var.clone());
            let result = check_structure(body, bound);
            bound.pop();
            result
        }
    }
}

/// Contractiveness: every occurrence of `var` sits under at least one interaction.
fn guarded(body: &GlobalType, var: &str) -> bool {
    match body {
        GlobalType::End => true,
        GlobalType::Var { var: v } => v != var,
        GlobalType::Interaction { .. } => true,
        GlobalType::Rec { var: v, body } => v == var || guarded(body, var),
    }
}

fn project(global: &GlobalType, role: &Role) -> Result<LocalType, ProtocolError> {
    match global {
        GlobalType::End => Ok(LocalType::End),
        GlobalType::Var { var } => Ok(LocalType::Var { var: var.clone() }),
        GlobalType::Rec { var, body } => {
            let projected = project(body, role)?;
            if projected.has_action() {
                Ok(LocalType::Rec {
                    var: var.clone(),
                    body: Box::new(projected),
                })
            } else {
                Ok(LocalType::End)
            }
        }
        GlobalType::Interaction { from, to, branches } => {
            let mut projected = Vec::with_capacity(branches.len());
            for branch in branches {
                projected.push(LocalBranch {
                    label: branch.label.clone(),
                    continuation: project(&branch.continuation, role)?,
                });
            }

            if role == from {
                Ok(LocalType::Select {
                    to: to.clone(),
                    branches: projected,
                })
            } else if role == to {
                Ok(LocalType::Offer {
                    from: from.clone(),
                    branches: projected,
                })
            } else {
                merge(&projected, role, from, to)
            }
        }
    }
}

/// A role uninvolved in a choice must end up with one local type covering every branch.
///
/// The merge is the standard one: two external choices from the *same* peer combine by taking the
/// union of their labels, because a role that is merely waiting can be told which way the choice
/// went by the label it eventually receives. Anything else must be identical. In particular two
/// *internal* choices do not merge — a role cannot be asked to pick a different message depending
/// on a decision nobody told it about — and neither does "act in one branch, finish in the other".
///
/// When the merge fails, the role has been asked to react to information it was never sent.
/// Reporting that as [`ProtocolError::UninformedOfChoice`] instead of picking a branch is the
/// whole point: the alternative silently produces a local type whose peer may be waiting for the
/// label the other branch would have sent.
fn merge(
    branches: &[LocalBranch],
    role: &Role,
    from: &Role,
    to: &Role,
) -> Result<LocalType, ProtocolError> {
    let first = &branches[0];
    let mut merged = first.continuation.clone();
    for other in &branches[1..] {
        merged = merge_pair(&merged, &other.continuation).ok_or_else(|| {
            ProtocolError::UninformedOfChoice {
                role: role.clone(),
                from: from.clone(),
                to: to.clone(),
                left: first.label.clone(),
                right: other.label.clone(),
            }
        })?;
    }
    Ok(merged)
}

fn merge_pair(left: &LocalType, right: &LocalType) -> Option<LocalType> {
    if left == right {
        return Some(left.clone());
    }
    match (left, right) {
        (
            LocalType::Offer {
                from: left_peer,
                branches: left_branches,
            },
            LocalType::Offer {
                from: right_peer,
                branches: right_branches,
            },
        ) if left_peer == right_peer => {
            let mut combined = left_branches.clone();
            for branch in right_branches {
                match combined
                    .iter_mut()
                    .find(|existing| existing.label == branch.label)
                {
                    Some(existing) => {
                        existing.continuation =
                            merge_pair(&existing.continuation, &branch.continuation)?;
                    }
                    None => combined.push(branch.clone()),
                }
            }
            combined.sort_by(|a, b| a.label.cmp(&b.label));
            Some(LocalType::Offer {
                from: left_peer.clone(),
                branches: combined,
            })
        }
        _ => None,
    }
}
