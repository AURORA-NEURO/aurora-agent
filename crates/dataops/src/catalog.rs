//! Relational catalog schema (12.03): the constraints, not the tables.
//!
//! 12.03 is the densest of the seven modules — thirty unique lines against a section median of
//! twenty — and almost all of that density is in six constraint sentences. There is no relational
//! engine here and there will not be one; what is implemented is the set of predicates a
//! conforming schema would have to hold, expressed over an in-memory catalog so they can be
//! violated in a test.
//!
//! | 12.03 clause | here |
//! |---|---|
//! | "digest uniqueness by artifact media type" | [`CatalogError::DuplicateDigest`] |
//! | "one active alias target per scope" | structural: [`AliasBinding`] holds one target and a history |
//! | "foreign-key closure" | [`CatalogError::DanglingReference`], checked on every reference |
//! | "no deletion of a revision referenced by a published result" | [`Catalog::retire`] |
//! | "append-only status history" | [`Revision::status_history`], with no removal method |
//! | "publication writes … atomically" | [`Catalog::publish`] validates before it mutates |
//! | "search updates consume the outbox" | [`Catalog::projection_basis`] |
//! | "meaningful mutations include actor, reason, previous/new state, trace" | a `bioprism-ledger` event |
//!
//! # The audit trail is not a table here
//!
//! 12.03 asks for an `audit_events` table carrying actor, operation, reason, previous and new
//! state, timestamp and trace id. That is an event log, `bioprism-ledger` already is one with
//! bitemporal semantics and a hash chain, and a second one inside a catalog would be a second one.
//! So [`Catalog::publish`] *builds* a [`bioprism_ledger::Event`] and hands it back in the receipt;
//! it holds no log and cannot append. The caller appends it to the ledger, and if it does not,
//! the missing audit record is visible as an unappended value rather than hidden inside a
//! subsystem that thinks it did the logging. Redaction of sensitive fields is likewise the
//! ledger's: its payload commits to a digest, so a redacted payload keeps the chain intact and
//! the action visible, which is exactly what 12.03 asks for and is not reimplemented here.
//!
//! # Where the freshness question enters
//!
//! 12.03's transaction paragraph ends *"Search updates consume the outbox"* and stops. It never
//! says what a query answered from a projection with unconsumed outbox events means. A conforming
//! implementation may answer from a projection arbitrarily far behind and report nothing.
//! [`Catalog::projection_basis`] is the refusal to do that: it returns
//! [`Basis::Derived`](crate::basis::Basis::Derived) whose lag is the count of unconsumed events,
//! and it returns `Derived` **even when the lag is zero**, because a projection that happens to be
//! caught up is still not the catalog and the next write will make that true again.
//!
//! # Not implemented
//!
//! No SQL, no storage, no transactions in any durable sense — [`Catalog::publish`]'s atomicity is
//! validate-then-mutate inside one `&mut self` call, which is real for a single process and is
//! nothing at all across two. No row-level security enforcement in a database; [`Visibility`] is a
//! filter this crate applies, and 12.03's "enforced in the data access layer **and database where
//! supported**" is half unimplementable here by construction. No signing keys, no vulnerabilities
//! or health-assessment tables, no reviews, no withdrawals workflow, no migrations — 12.03 lists
//! twenty-six table names and this models six of them. Opaque sortable ids are derived
//! deterministically from the object and an ordinal rather than allocated from a sequence.

use crate::basis::Basis;
use crate::error::{check_name, describe, CatalogError};
use bioprism_ids::ContentHash;
use bioprism_infra::Epoch;
use bioprism_ledger::{Actor, Event, EventClass, EventKind, EventTimes, SubjectKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

macro_rules! catalog_id {
    ($name:ident, $field:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            pub fn parse(value: impl Into<String>) -> Result<Self, CatalogError> {
                let value = value.into();
                if !check_name(&value) {
                    return Err(CatalogError::MalformedField {
                        field: $field,
                        value,
                    });
                }
                Ok($name(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl TryFrom<String> for $name {
            type Error = CatalogError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $name::parse(value)
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

catalog_id!(Namespace, "namespace");
catalog_id!(ObjectId, "object id");
catalog_id!(MediaType, "media type");
catalog_id!(AliasName, "alias name");
catalog_id!(PublicationId, "publication id");

/// An opaque sortable revision id, derived rather than allocated.
///
/// 12.03 wants "opaque sortable IDs for rows". A sequence would need a counter that differs
/// between two runs of the same pipeline, so the id is `object@ordinal`: sortable within an
/// object, deterministic, and carrying no meaning a caller should parse back out.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RevisionId(String);

impl RevisionId {
    fn derive(object: &ObjectId, ordinal: u64) -> Self {
        RevisionId(format!("{}@{}", object.as_str(), ordinal))
    }

    pub fn parse(value: impl Into<String>) -> Result<Self, CatalogError> {
        let value = value.into();
        if !check_name(&value) {
            return Err(CatalogError::MalformedField {
                field: "revision id",
                value,
            });
        }
        Ok(RevisionId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RevisionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for RevisionId {
    type Error = CatalogError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RevisionId::parse(value)
    }
}

impl From<RevisionId> for String {
    fn from(value: RevisionId) -> Self {
        value.0
    }
}

/// Who may read a revision.
///
/// 12.03's "encode tenancy and visibility". `bioprism-infra` states plainly that it has no tenant
/// concept at all and that adding one after the fact is how a cross-tenant leak happens; this is
/// the smallest honest version — a per-revision visibility the catalog applies on read, with no
/// claim that a database is enforcing anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// Readable from any namespace.
    Public,
    /// Readable only from the owning namespace.
    Private,
}

/// A point in a revision's append-only status history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Draft,
    Released,
    Withdrawn,
}

impl Status {
    pub fn name(self) -> &'static str {
        match self {
            Status::Draft => "draft",
            Status::Released => "released",
            Status::Withdrawn => "withdrawn",
        }
    }
}

/// One entry in the append-only status history.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusEntry {
    pub status: Status,
    pub at: Epoch,
}

/// An object header: a stable identity with a namespace and a media type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectHeader {
    pub id: ObjectId,
    pub namespace: Namespace,
    pub media_type: MediaType,
}

/// An immutable revision of an object.
///
/// The status history is a `Vec` with no removal method and no setter. Transitions push; the
/// current status is the last element. That is the whole of 12.03's "append-only status history"
/// made structural rather than asserted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision {
    id: RevisionId,
    object: ObjectId,
    ordinal: u64,
    digest: ContentHash,
    visibility: Visibility,
    status_history: Vec<StatusEntry>,
}

impl Revision {
    pub fn id(&self) -> &RevisionId {
        &self.id
    }

    pub fn object(&self) -> &ObjectId {
        &self.object
    }

    pub fn ordinal(&self) -> u64 {
        self.ordinal
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn status_history(&self) -> &[StatusEntry] {
        &self.status_history
    }

    /// The last entry in the history. There is always at least one.
    pub fn status(&self) -> Status {
        self.status_history
            .last()
            .map(|entry| entry.status)
            .unwrap_or(Status::Draft)
    }
}

/// A mutable human slug with exactly one active target and a record of every previous one.
///
/// "One active alias target per scope" is enforced by the shape: there is one `target` field.
/// Two active targets are not a constraint violation this type can express, which is the version
/// of the rule that survives a refactor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AliasBinding {
    target: RevisionId,
    history: Vec<(RevisionId, Epoch)>,
}

impl AliasBinding {
    pub fn target(&self) -> &RevisionId {
        &self.target
    }

    /// Every target this alias previously pointed at, oldest first.
    pub fn history(&self) -> &[(RevisionId, Epoch)] {
        &self.history
    }
}

/// A published result binding a set of revisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Publication {
    pub id: PublicationId,
    pub revisions: BTreeSet<RevisionId>,
    pub at: Epoch,
}

/// What a publication asks for, before anything is checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationRequest {
    pub id: PublicationId,
    pub revisions: BTreeSet<RevisionId>,
    pub at: Epoch,
}

/// An entry in the outbox a search projection consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub sequence: u64,
    pub kind: String,
    pub subject: String,
    pub at: Epoch,
}

/// Where a projection has read up to.
///
/// Held by the consumer, not the catalog. A catalog that tracked its own consumers' cursors would
/// be able to answer "am I current" without asking them, which is the failure this type exists to
/// prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OutboxCursor(u64);

impl OutboxCursor {
    pub const fn new(consumed: u64) -> Self {
        OutboxCursor(consumed)
    }

    pub const fn consumed(self) -> u64 {
        self.0
    }
}

/// Everything a mutation needs to produce an audit record.
///
/// Required by every mutating method rather than optional, so an unaudited write is not
/// expressible. The times are the caller's — this crate reads no clock, and `bioprism-ledger`
/// refuses to either.
#[derive(Debug, Clone)]
pub struct AuditContext {
    pub actor: Actor,
    pub times: EventTimes,
    pub reason: String,
    pub trace: String,
}

/// What a publication produced: the record, the outbox entry, and the audit event to append.
///
/// The audit event is returned rather than stored. Dropping it is a visible act.
#[derive(Debug, Clone)]
pub struct PublicationReceipt {
    pub publication: Publication,
    pub outbox: OutboxEvent,
    pub audit: Event,
}

/// The transactional catalog.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Catalog {
    objects: BTreeMap<ObjectId, ObjectHeader>,
    revisions: BTreeMap<RevisionId, Revision>,
    digests: BTreeMap<MediaType, BTreeMap<String, RevisionId>>,
    aliases: BTreeMap<Namespace, BTreeMap<AliasName, AliasBinding>>,
    lineage: BTreeMap<RevisionId, BTreeSet<RevisionId>>,
    publications: BTreeMap<PublicationId, Publication>,
    outbox: Vec<OutboxEvent>,
}

impl Catalog {
    pub fn new() -> Self {
        Catalog::default()
    }

    /// A digest over the whole catalog state.
    ///
    /// Exists so that "a failed publication changed nothing" is testable as an equality rather
    /// than as a list of things somebody remembered to check.
    pub fn digest(&self) -> Result<ContentHash, CatalogError> {
        let value = serde_json::to_value(self).map_err(|error| CatalogError::Audit {
            detail: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error| CatalogError::Audit {
            detail: error.to_string(),
        })
    }

    pub fn declare_object(&mut self, header: ObjectHeader) -> Result<(), CatalogError> {
        self.objects.insert(header.id.clone(), header);
        Ok(())
    }

    /// Adds an immutable revision.
    ///
    /// Refuses a digest already recorded for the same media type. 12.03 scopes the uniqueness
    /// constraint by media type on purpose: the same bytes are legitimately both a manifest and a
    /// blob, and a global digest constraint would make one of the two unrepresentable.
    pub fn add_revision(
        &mut self,
        object: &ObjectId,
        digest: ContentHash,
        visibility: Visibility,
        at: Epoch,
    ) -> Result<RevisionId, CatalogError> {
        let header = self
            .objects
            .get(object)
            .ok_or_else(|| CatalogError::UnknownObject {
                object: object.to_string(),
            })?;
        let media_type = header.media_type.clone();
        if let Some(existing) = self
            .digests
            .get(&media_type)
            .and_then(|by_digest| by_digest.get(digest.as_str()))
        {
            return Err(CatalogError::DuplicateDigest {
                media_type: media_type.to_string(),
                digest: digest.as_str().to_string(),
                existing: existing.to_string(),
            });
        }
        let ordinal = self
            .revisions
            .values()
            .filter(|revision| revision.object() == object)
            .count() as u64
            + 1;
        let id = RevisionId::derive(object, ordinal);
        self.digests
            .entry(media_type)
            .or_default()
            .insert(digest.as_str().to_string(), id.clone());
        self.revisions.insert(
            id.clone(),
            Revision {
                id: id.clone(),
                object: object.clone(),
                ordinal,
                digest,
                visibility,
                status_history: vec![StatusEntry {
                    status: Status::Draft,
                    at,
                }],
            },
        );
        Ok(id)
    }

    pub fn revision(&self, id: &RevisionId) -> Option<&Revision> {
        self.revisions.get(id)
    }

    pub fn object(&self, id: &ObjectId) -> Option<&ObjectHeader> {
        self.objects.get(id)
    }

    /// Revisions a reader in `viewer` may see, sorted by revision id.
    ///
    /// A private revision belongs to the namespace of its object. The filter is applied here and
    /// nowhere else, so there is exactly one place to audit.
    pub fn visible_to(&self, viewer: &Namespace) -> Vec<&Revision> {
        self.revisions
            .values()
            .filter(|revision| match revision.visibility() {
                Visibility::Public => true,
                Visibility::Private => self
                    .objects
                    .get(revision.object())
                    .is_some_and(|header| &header.namespace == viewer),
            })
            .collect()
    }

    /// Points a slug at a revision, recording the previous target.
    ///
    /// An alias may not cross a namespace: 12.03 makes slugs "unique only within namespace", and
    /// an alias in one namespace resolving into another is a visibility hole dressed as a
    /// convenience.
    pub fn set_alias(
        &mut self,
        scope: &Namespace,
        alias: &AliasName,
        target: &RevisionId,
        at: Epoch,
    ) -> Result<(), CatalogError> {
        let revision = self
            .revisions
            .get(target)
            .ok_or_else(|| CatalogError::DanglingReference {
                reference: alias.to_string(),
                target: target.to_string(),
            })?;
        let header = self
            .objects
            .get(revision.object())
            .ok_or_else(|| CatalogError::UnknownObject {
                object: revision.object().to_string(),
            })?;
        if &header.namespace != scope {
            return Err(CatalogError::AliasCrossesNamespace {
                alias: alias.to_string(),
                scope: scope.to_string(),
                target_namespace: header.namespace.to_string(),
            });
        }
        let in_scope = self.aliases.entry(scope.clone()).or_default();
        match in_scope.get_mut(alias) {
            Some(binding) => {
                binding.history.push((binding.target.clone(), at));
                binding.target = target.clone();
            }
            None => {
                in_scope.insert(
                    alias.clone(),
                    AliasBinding {
                        target: target.clone(),
                        history: Vec::new(),
                    },
                );
            }
        }
        Ok(())
    }

    pub fn alias(&self, scope: &Namespace, alias: &AliasName) -> Option<&AliasBinding> {
        self.aliases.get(scope).and_then(|scoped| scoped.get(alias))
    }

    /// Records that `child` derives from `parent`.
    ///
    /// Both ends must exist, and the edge must not close a cycle. A cyclic lineage makes the
    /// closure check below non-terminating and makes "reconstructible from declared dependencies"
    /// false, so it is refused at insert rather than discovered during a publication.
    pub fn add_lineage(
        &mut self,
        child: &RevisionId,
        parent: &RevisionId,
    ) -> Result<(), CatalogError> {
        for end in [child, parent] {
            if !self.revisions.contains_key(end) {
                return Err(CatalogError::UnknownRevision {
                    revision: end.to_string(),
                });
            }
        }
        if child == parent || self.reaches(parent, child) {
            return Err(CatalogError::LineageCycle {
                child: child.to_string(),
                parent: parent.to_string(),
            });
        }
        self.lineage
            .entry(child.clone())
            .or_default()
            .insert(parent.clone());
        Ok(())
    }

    fn reaches(&self, from: &RevisionId, target: &RevisionId) -> bool {
        let mut seen: BTreeSet<&RevisionId> = BTreeSet::new();
        let mut stack = vec![from];
        while let Some(node) = stack.pop() {
            if node == target {
                return true;
            }
            if !seen.insert(node) {
                continue;
            }
            if let Some(parents) = self.lineage.get(node) {
                stack.extend(parents.iter());
            }
        }
        false
    }

    /// Every revision reachable from `revision` through lineage, including itself.
    pub fn closure(&self, revision: &RevisionId) -> BTreeSet<RevisionId> {
        let mut out = BTreeSet::new();
        let mut stack = vec![revision.clone()];
        while let Some(node) = stack.pop() {
            if !out.insert(node.clone()) {
                continue;
            }
            if let Some(parents) = self.lineage.get(&node) {
                stack.extend(parents.iter().cloned());
            }
        }
        out
    }

    /// Writes revision status, artifact closure, release status and the outbox entry, or nothing.
    ///
    /// Every check runs before the first mutation. That is what "atomically" can mean inside one
    /// process with no write-ahead log, and the limit is stated rather than implied: two
    /// processes calling this concurrently have no protection whatsoever, because there is no
    /// lock in this crate and 12.03's outbox pattern assumes a database transaction that does not
    /// exist here.
    pub fn publish(
        &mut self,
        request: PublicationRequest,
        audit: &AuditContext,
    ) -> Result<PublicationReceipt, CatalogError> {
        if self.publications.contains_key(&request.id) {
            return Err(CatalogError::DuplicatePublication {
                publication: request.id.to_string(),
            });
        }
        for revision in &request.revisions {
            if !self.revisions.contains_key(revision) {
                return Err(CatalogError::UnknownRevision {
                    revision: revision.to_string(),
                });
            }
        }
        let mut closure = BTreeSet::new();
        for revision in &request.revisions {
            closure.extend(self.closure(revision));
        }
        let missing: Vec<&RevisionId> = closure
            .iter()
            .filter(|id| !self.revisions.contains_key(*id))
            .collect();
        if let Some(first) = missing.first() {
            return Err(CatalogError::ClosureIncomplete {
                publication: request.id.to_string(),
                missing: first.to_string(),
            });
        }
        for revision in &closure {
            if self.revisions[revision].status() == Status::Withdrawn {
                return Err(CatalogError::ClosureWithdrawn {
                    publication: request.id.to_string(),
                    revision: revision.to_string(),
                });
            }
        }

        let sequence = self.outbox.len() as u64 + 1;
        let outbox = OutboxEvent {
            sequence,
            kind: "publication.released".to_string(),
            subject: request.id.to_string(),
            at: request.at,
        };
        let previous: Vec<Value> = closure
            .iter()
            .map(|id| json!({ "revision": id.as_str(), "status": self.revisions[id].status().name() }))
            .collect();
        for revision in &closure {
            let entry = StatusEntry {
                status: Status::Released,
                at: request.at,
            };
            if let Some(record) = self.revisions.get_mut(revision) {
                record.status_history.push(entry);
            }
        }
        let publication = Publication {
            id: request.id.clone(),
            revisions: closure.clone(),
            at: request.at,
        };
        self.publications
            .insert(publication.id.clone(), publication.clone());
        self.outbox.push(outbox.clone());

        let payload = json!({
            "operation": "publish",
            "reason": audit.reason,
            "trace": audit.trace,
            "previous": previous,
            "new": closure.iter().map(|id| id.as_str()).collect::<Vec<_>>(),
        });
        let event = Event::new(
            EventClass::Epistemic,
            EventKind::parse("catalog.publication.released").map_err(|error| {
                CatalogError::Audit {
                    detail: error.to_string(),
                }
            })?,
            audit.actor.clone(),
            SubjectKey::parse(request.id.as_str()).map_err(|error| CatalogError::Audit {
                detail: error.to_string(),
            })?,
            audit.times,
            payload,
        )
        .map_err(|error| CatalogError::Audit {
            detail: format!("{error}: {}", describe(&json!(request.id.as_str()))),
        })?;

        Ok(PublicationReceipt {
            publication,
            outbox,
            audit: event,
        })
    }

    /// Marks a revision withdrawn, refusing if a publication references it.
    ///
    /// 12.03: "no deletion of a revision referenced by a published result". The refusal names the
    /// publication, because the operator's next question is always which one.
    pub fn retire(&mut self, revision: &RevisionId, at: Epoch) -> Result<(), CatalogError> {
        if !self.revisions.contains_key(revision) {
            return Err(CatalogError::UnknownRevision {
                revision: revision.to_string(),
            });
        }
        if let Some(publication) = self
            .publications
            .values()
            .find(|publication| publication.revisions.contains(revision))
        {
            return Err(CatalogError::ReferencedByPublication {
                revision: revision.to_string(),
                publication: publication.id.to_string(),
            });
        }
        if let Some(record) = self.revisions.get_mut(revision) {
            record.status_history.push(StatusEntry {
                status: Status::Withdrawn,
                at,
            });
        }
        Ok(())
    }

    pub fn publication(&self, id: &PublicationId) -> Option<&Publication> {
        self.publications.get(id)
    }

    /// How many outbox events have been emitted in total.
    pub fn outbox_emitted(&self) -> u64 {
        self.outbox.len() as u64
    }

    pub fn outbox(&self) -> &[OutboxEvent] {
        &self.outbox
    }

    /// The basis a projection built from this outbox may claim.
    ///
    /// Always [`Basis::Derived`], never first-hand, even at zero lag. A projection caught up at
    /// this instant is still a copy, and the variant that says so is the one a consumer will
    /// still be matching on after the next write.
    pub fn projection_basis(&self, cursor: OutboxCursor) -> Result<Basis, CatalogError> {
        let emitted = self.outbox_emitted();
        if cursor.consumed() > emitted {
            return Err(CatalogError::OutboxCursorAhead {
                cursor: cursor.consumed(),
                emitted,
            });
        }
        Ok(Basis::Derived {
            source: "catalog-outbox".to_string(),
            lag_epochs: emitted - cursor.consumed(),
        })
    }
}
