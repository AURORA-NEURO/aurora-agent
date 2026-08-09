//! Shared memory, blackboard coordination and replicated state.
//!
//! Blueprint 23.17.
//!
//! # Append, do not overwrite
//!
//! 23.17: "Agents do not overwrite a shared 'answer.'" There is therefore no `set` and no
//! `get_mut` on [`Blackboard`]. The only mutation is [`Blackboard::publish`], and a "current view"
//! is [`Blackboard::project`], recomputed from history every time. Two agents publishing
//! contradictory claims both appear in the projection, because "concurrent conflicting claims are
//! valid".
//!
//! # The CRDT boundary, enforced
//!
//! 23.17: "They are not an epistemic oracle. A mergeable data structure cannot decide which
//! scientific claim is correct or which patch should win." [`Reducer::for_topic`] refuses to hand
//! back a reducer for a topic carrying claims or exclusive resources — a grow-only set and a
//! counter are available, a "latest wins" is not, and asking for one on a claim topic is
//! [`BlackboardError::NoReducerForEpistemicTopic`] rather than a silently wrong merge.
//!
//! # One-writer leases
//!
//! Concurrent conflicting *claims* are fine; concurrent writes to the same external resource are
//! not. [`LeaseTable`] is affine: a second lease on a held resource is refused, naming the holder.
//!
//! # Not implemented
//!
//! No storage, no index, no search, no cache. The eight storage layers of 23.17 are named in
//! [`StorageLayer`] and only the canonical event log is realised, which is the only one 23.17 calls
//! canonical anyway — "Only the event log and immutable artifacts are canonical. Other views are
//! rebuildable." No poisoning *detection*: [`Blackboard::promote`] enforces the rule that a local note may not
//! become verified ground without independent verification, which is a gate, not a detector.

use crate::flow::{FlowDecision, Labelling, Principal};
use crate::reputation::LogicalTime;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.17's eight storage layers. Only [`StorageLayer::CanonicalEventLog`] is implemented here; the
/// rest are named so a system architect can see what this crate is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageLayer {
    CanonicalEventLog,
    ArtifactStore,
    EpistemicLedger,
    CommitmentLedger,
    AuthorityLedger,
    BudgetLedger,
    WorkspaceIndex,
    LocalAgentCache,
}

impl StorageLayer {
    /// Whether a layer is canonical or rebuildable.
    pub fn is_canonical(&self) -> bool {
        matches!(
            self,
            StorageLayer::CanonicalEventLog | StorageLayer::ArtifactStore
        )
    }
}

/// 23.17's six memory scopes, widest last.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    TurnLocal,
    ParticipantLocal,
    ThreadShared,
    MoleculeShared,
    OrganizationMemory,
    PublicRegistry,
}

/// "Flows between scopes require explicit policy."
///
/// Widening without a policy is refused. Narrowing is always permitted — moving a public fact into
/// a thread costs nothing — which is the asymmetry that makes this worth a function.
pub fn flow_between_scopes(
    from: MemoryScope,
    to: MemoryScope,
    policy: Option<&str>,
) -> Result<(), BlackboardError> {
    if to <= from {
        return Ok(());
    }
    match policy {
        Some(policy) if !policy.is_empty() => Ok(()),
        _ => Err(BlackboardError::ScopeWideningWithoutPolicy { from, to }),
    }
}

/// A typed topic, e.g. `claims/current`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Topic(pub String);

impl Topic {
    pub fn new(name: impl Into<String>) -> Self {
        Topic(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this topic carries epistemic content, which is what the CRDT boundary turns on.
    ///
    /// Decided by prefix, from 23.17's own topic list: `claims/`, `hypotheses/` and `disputes/` are
    /// epistemic, the rest are coordination state.
    pub fn is_epistemic(&self) -> bool {
        ["claims/", "hypotheses/", "disputes/"]
            .iter()
            .any(|prefix| self.0.starts_with(prefix))
    }
}

/// The six things 23.17 says an agent may append.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "entry")]
pub enum EntryKind {
    Observation { value: String },
    Endorsement { target: String },
    Retraction { target: String },
    Supersession { target: String, value: String },
    Challenge { target: String, reason: String },
    Resolution { target: String, decision: String },
}

/// One append-only blackboard entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub topic: Topic,
    pub author: String,
    pub sequence: u64,
    pub label: Labelling,
    pub scope: MemoryScope,
    pub kind: EntryKind,
    /// 23.17's idempotency key. Two publishes with the same key are one entry.
    pub idempotency_key: Option<String>,
    pub expires_at: Option<LogicalTime>,
    /// Whether this entry has been independently verified. Set only through [`Blackboard::promote`].
    verified: bool,
}

impl Entry {
    pub fn new(
        id: impl Into<String>,
        topic: Topic,
        author: impl Into<String>,
        kind: EntryKind,
    ) -> Self {
        Entry {
            id: id.into(),
            topic,
            author: author.into(),
            sequence: 0,
            label: Labelling::Unlabelled,
            scope: MemoryScope::ThreadShared,
            kind,
            idempotency_key: None,
            expires_at: None,
            verified: false,
        }
    }

    pub fn labelled(mut self, label: Labelling) -> Self {
        self.label = label;
        self
    }

    pub fn in_scope(mut self, scope: MemoryScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn keyed(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    pub fn expiring_at(mut self, at: u64) -> Self {
        self.expires_at = Some(LogicalTime(at));
        self
    }

    pub fn is_verified(&self) -> bool {
        self.verified
    }
}

/// An append-only shared workspace.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blackboard {
    entries: Vec<Entry>,
    keys: BTreeSet<String>,
}

impl Blackboard {
    pub fn new() -> Self {
        Blackboard::default()
    }

    /// Append. The only mutation.
    ///
    /// Sequence numbers are assigned here so ordering is the blackboard's, not a caller's clock.
    pub fn publish(&mut self, mut entry: Entry) -> Result<u64, BlackboardError> {
        if let Some(key) = &entry.idempotency_key {
            if !self.keys.insert(key.clone()) {
                return Err(BlackboardError::DuplicateIdempotencyKey { key: key.clone() });
            }
        }
        let sequence = self.entries.len() as u64;
        entry.sequence = sequence;
        self.entries.push(entry);
        Ok(sequence)
    }

    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries a subscription admits: matching topic, within clearance, unexpired.
    pub fn watch(&self, subscription: &Subscription, as_of: LogicalTime) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| subscription.matches(entry))
            .filter(|entry| entry.expires_at.map(|at| as_of < at).unwrap_or(true))
            .filter(|entry| {
                matches!(
                    entry.label.flows_to(&subscription.subscriber.clearance),
                    FlowDecision::Permitted
                )
            })
            .collect()
    }

    /// The current view of a topic: the live value for each target, with history preserved.
    ///
    /// Retraction removes a value from the view and never from the log. Two live values on the same
    /// target are both returned, because collapsing them is exactly the epistemic-oracle move the
    /// CRDT boundary forbids.
    pub fn project(&self, topic: &Topic, as_of: LogicalTime) -> TopicView {
        let mut live: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut retracted = BTreeSet::new();
        let mut challenged = BTreeSet::new();
        let mut resolved = BTreeSet::new();
        for entry in self.entries.iter().filter(|e| &e.topic == topic) {
            if entry.expires_at.map(|at| as_of >= at).unwrap_or(false) {
                continue;
            }
            match &entry.kind {
                EntryKind::Observation { value } => {
                    live.entry(entry.id.clone()).or_default().insert(value.clone());
                }
                EntryKind::Supersession { target, value } => {
                    retracted.insert(target.clone());
                    live.entry(entry.id.clone()).or_default().insert(value.clone());
                }
                EntryKind::Retraction { target } => {
                    retracted.insert(target.clone());
                }
                EntryKind::Challenge { target, .. } => {
                    challenged.insert(target.clone());
                }
                EntryKind::Resolution { target, .. } => {
                    resolved.insert(target.clone());
                }
                EntryKind::Endorsement { .. } => {}
            }
        }
        for target in &retracted {
            live.remove(target);
        }
        TopicView {
            topic: topic.clone(),
            live,
            retracted,
            unresolved_challenges: challenged.difference(&resolved).cloned().collect(),
            history_length: self.entries.iter().filter(|e| &e.topic == topic).count(),
        }
    }

    /// "No automatic promotion from local note to verified ground."
    ///
    /// Requires independent verifiers: distinct principals, none of them the author. Refuses
    /// otherwise, naming what was missing.
    pub fn promote(
        &mut self,
        entry_id: &str,
        verifiers: &BTreeSet<String>,
        required: usize,
    ) -> Result<(), BlackboardError> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.id == entry_id)
            .ok_or_else(|| BlackboardError::NoSuchEntry {
                id: entry_id.to_string(),
            })?;
        let independent: BTreeSet<&String> =
            verifiers.iter().filter(|v| **v != entry.author).collect();
        if independent.len() < required {
            return Err(BlackboardError::InsufficientIndependentVerification {
                id: entry_id.to_string(),
                required,
                independent: independent.len(),
            });
        }
        entry.verified = true;
        Ok(())
    }
}

/// The current view of a topic.
///
/// Not [`crate::flow::Projection`], which is a different thing: that one is a recipient-specific
/// disclosure, this one is a temporal fold over one topic's history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopicView {
    pub topic: Topic,
    pub live: BTreeMap<String, BTreeSet<String>>,
    pub retracted: BTreeSet<String>,
    pub unresolved_challenges: BTreeSet<String>,
    /// Entries in the log for this topic. Always at least the size of the live view; the gap is
    /// what a "current answer" model would have thrown away.
    pub history_length: usize,
}

impl TopicView {
    pub fn history_preserved(&self) -> bool {
        self.history_length >= self.live.len()
    }
}

/// A label-aware, predicate-filtered subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subscription {
    pub subscriber: Principal,
    pub topic_prefix: String,
    pub authors: Option<BTreeSet<String>>,
}

impl Subscription {
    pub fn new(subscriber: Principal, topic_prefix: impl Into<String>) -> Self {
        Subscription {
            subscriber,
            topic_prefix: topic_prefix.into(),
            authors: None,
        }
    }

    pub fn from_authors(mut self, authors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.authors = Some(authors.into_iter().map(Into::into).collect());
        self
    }

    fn matches(&self, entry: &Entry) -> bool {
        entry.topic.as_str().starts_with(&self.topic_prefix)
            && self
                .authors
                .as_ref()
                .map(|set| set.contains(&entry.author))
                .unwrap_or(true)
    }
}

/// The merge strategies 23.17 permits for replicated metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reducer {
    GrowOnlySet,
    Counter,
    /// Presence and subscription state: last writer per author, which is well defined because each
    /// author writes only its own entry.
    PerAuthorLatest,
}

impl Reducer {
    /// The reducer a topic may use, or a refusal.
    ///
    /// A claim topic gets no reducer at all. This is the CRDT boundary and it is the only reason
    /// this function returns `Result`.
    pub fn for_topic(topic: &Topic) -> Result<Reducer, BlackboardError> {
        if topic.is_epistemic() {
            return Err(BlackboardError::NoReducerForEpistemicTopic {
                topic: topic.clone(),
            });
        }
        Ok(match topic.as_str() {
            t if t.starts_with("presence/") || t.starts_with("subscriptions/") => {
                Reducer::PerAuthorLatest
            }
            t if t.starts_with("budgets/") => Reducer::Counter,
            _ => Reducer::GrowOnlySet,
        })
    }
}

/// One-writer leases for mutable external resources. Affine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseTable {
    held: BTreeMap<String, String>,
}

impl LeaseTable {
    pub fn new() -> Self {
        LeaseTable::default()
    }

    /// Take an exclusive lease. Refuses if held, naming the holder.
    pub fn acquire(
        &mut self,
        resource: impl Into<String>,
        holder: impl Into<String>,
    ) -> Result<(), BlackboardError> {
        let resource = resource.into();
        let holder = holder.into();
        match self.held.get(&resource) {
            Some(existing) => Err(BlackboardError::ResourceAlreadyLeased {
                resource,
                holder: existing.clone(),
            }),
            None => {
                self.held.insert(resource, holder);
                Ok(())
            }
        }
    }

    pub fn release(&mut self, resource: &str, holder: &str) -> Result<(), BlackboardError> {
        match self.held.get(resource) {
            Some(existing) if existing == holder => {
                self.held.remove(resource);
                Ok(())
            }
            Some(existing) => Err(BlackboardError::ResourceAlreadyLeased {
                resource: resource.to_string(),
                holder: existing.clone(),
            }),
            None => Ok(()),
        }
    }

    pub fn holder(&self, resource: &str) -> Option<&String> {
        self.held.get(resource)
    }

    pub fn outstanding(&self) -> usize {
        self.held.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BlackboardError {
    #[error("idempotency key {key} has already been published")]
    DuplicateIdempotencyKey { key: String },

    #[error("no entry {id}")]
    NoSuchEntry { id: String },

    #[error("{id} needs {required} independent verifiers and has {independent}")]
    InsufficientIndependentVerification {
        id: String,
        required: usize,
        independent: usize,
    },

    #[error("{resource} is leased to {holder}")]
    ResourceAlreadyLeased { resource: String, holder: String },

    #[error("{topic:?} carries claims; a mergeable data structure cannot decide which claim wins")]
    NoReducerForEpistemicTopic { topic: Topic },

    #[error("moving memory from {from:?} to {to:?} widens its scope and needs an explicit policy")]
    ScopeWideningWithoutPolicy {
        from: MemoryScope,
        to: MemoryScope,
    },
}
