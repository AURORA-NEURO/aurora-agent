//! The append-only, bitemporal event ledger.
//!
//! Blueprint 40.09 (event ledger and temporal semantics) and 12.04 (metadata, event and object
//! models). The four non-negotiable invariants of 40.09 map onto code as follows:
//!
//! 1. *Events are append-only* — [`LedgerEntry`] has no mutating method, [`EventLedger::append`]
//!    is the only way in, and sequence numbers are never reused.
//! 2. *Corrections supersede rather than delete* — [`crate::event::Event::superseding`] records
//!    a link from the correction to the original; the original stays readable forever and is
//!    still the right answer to a record-time query from before the correction was learned.
//! 3. *Valid time is distinct from record and release time* — enforced by the type system in
//!    [`crate::time`], not by convention.
//! 4. *Causal parents resolve or the event is quarantined* — an event naming an unknown parent
//!    is held in [`EventLedger::quarantined`] and admitted automatically when the parent
//!    arrives, rather than being rejected (out-of-order delivery is normal) or admitted with a
//!    dangling edge (which would make the causal graph a lie).
//!
//! What this is not: durable, concurrent, or distributed. Everything lives in one `Vec` in one
//! process, there is no write-ahead log, no outbox worker, and no transaction beyond the fact
//! that `append` either pushes an entry or returns an error. The blueprint's "append
//! transactionally" and "update outbox" steps are storage concerns; the semantics they are
//! meant to protect are what is implemented here, and a durable backend can be built beneath
//! this interface without changing any of it.
//!
//! Nor does it read a clock. Every instant on every event is supplied by the caller, so two
//! runs from the same inputs produce byte-identical digests. The cost is that the ledger cannot
//! detect a caller that lies about time; the benefit is that the whole module is deterministic
//! and its tests do not flake.

use crate::entry::{ChainStatus, LedgerEntry};
use crate::error::LedgerError;
use crate::event::{Event, EventClass, IdempotencyKey, SchemaCatalog, SubjectKey};
use crate::retention::{
    CompactionAnchor, CompactionReport, Redaction, RetentionPolicy, RetentionWindow,
};
use crate::time::{RecordTime, TemporalCut};
use bioprism_ids::EventId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// An event held back because something it names is not here yet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub key: IdempotencyKey,
    pub event: Event,
    /// Causal parents still unresolved. Empty means the event is unblocked but was refused for
    /// the reason in `note` — most often a supersession fork opened while it waited.
    pub missing: Vec<EventId>,
    pub note: Option<String>,
}

/// What happened to a submitted event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "admission")]
pub enum Admission {
    Recorded {
        id: EventId,
        seq: u64,
    },
    /// The identical event was already recorded under this key. Not an error: 12.17 makes
    /// "event consumption by event ID" an idempotency domain, and a retried submission must
    /// converge rather than duplicate.
    Duplicate {
        id: EventId,
    },
    Quarantined {
        key: IdempotencyKey,
        missing: Vec<EventId>,
    },
}

impl Admission {
    pub fn recorded_id(&self) -> Option<&EventId> {
        match self {
            Admission::Recorded { id, .. } | Admission::Duplicate { id } => Some(id),
            Admission::Quarantined { .. } => None,
        }
    }

    pub fn is_duplicate(&self) -> bool {
        matches!(self, Admission::Duplicate { .. })
    }

    pub fn is_quarantined(&self) -> bool {
        matches!(self, Admission::Quarantined { .. })
    }
}

/// The outcome of one `append`, including any knock-on admissions it unblocked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppendReceipt {
    pub admission: Admission,
    /// Previously quarantined events that this append made resolvable, in admission order.
    /// 40.09 lists "invalidation notifications" as an output; this is the honest minimum —
    /// telling the caller that the log grew by more than the one event they submitted.
    pub released: Vec<EventId>,
}

/// A place where record time went backwards between consecutive entries.
///
/// Not an error. Record time is data supplied by the caller, and refusing a regression would
/// make the ledger unable to ingest a backfill from a slower upstream. But 40.09 names "clock
/// inconsistency" as a failure class, and 12.04 requires an actionable diagnostic rather than a
/// silent repair, so it is reported.
///
/// The consequence matters for checkpoints: on a log with no anomalies, a record-time cut is a
/// sequence prefix and could be checkpointed; on a log with anomalies it is a filter, and only
/// sequence-prefix projections are checkpointable. See [`crate::projection`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClockAnomaly {
    pub seq: u64,
    pub previous_record: String,
    pub record: String,
}

/// The ledger.
#[derive(Debug, Clone)]
pub struct EventLedger {
    entries: Vec<LedgerEntry>,
    by_id: BTreeMap<EventId, usize>,
    by_key: BTreeMap<IdempotencyKey, EventId>,
    superseded_by: BTreeMap<EventId, EventId>,
    quarantine: Vec<QuarantineEntry>,
    catalog: SchemaCatalog,
    next_seq: u64,
    window: RetentionWindow,
    compactions: Vec<CompactionAnchor>,
}

impl Default for EventLedger {
    fn default() -> Self {
        EventLedger::new()
    }
}

impl EventLedger {
    /// A ledger that accepts any well-formed event kind.
    pub fn new() -> Self {
        EventLedger::with_schemas(SchemaCatalog::open())
    }

    /// A ledger that validates kinds against a catalog (40.09 step 1, "validate schema").
    pub fn with_schemas(catalog: SchemaCatalog) -> Self {
        EventLedger {
            entries: Vec::new(),
            by_id: BTreeMap::new(),
            by_key: BTreeMap::new(),
            superseded_by: BTreeMap::new(),
            quarantine: Vec::new(),
            catalog,
            next_seq: 0,
            window: RetentionWindow::unrestricted(),
            compactions: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[LedgerEntry] {
        &self.entries
    }

    /// Digest of the newest entry, or the empty string for an empty ledger. This is the value
    /// the next entry will chain to.
    pub fn head(&self) -> String {
        self.entries
            .last()
            .map(|entry| entry.digest.clone())
            .unwrap_or_default()
    }

    /// Next sequence number to be assigned. Never decreases, even across compaction, which is
    /// what lets [`EventLedger::verify_chain`] count removed entries.
    pub fn next_seq(&self) -> u64 {
        self.next_seq
    }

    pub fn window(&self) -> RetentionWindow {
        self.window
    }

    pub fn compactions(&self) -> &[CompactionAnchor] {
        &self.compactions
    }

    pub fn quarantined(&self) -> &[QuarantineEntry] {
        &self.quarantine
    }

    pub fn get(&self, id: &EventId) -> Option<&LedgerEntry> {
        self.by_id.get(id).map(|index| &self.entries[*index])
    }

    /// [`EventLedger::get`] with a typed failure, for callers that need the error rather than
    /// an `Option` they will unwrap anyway.
    pub fn require(&self, id: &EventId) -> Result<&LedgerEntry, LedgerError> {
        self.get(id)
            .ok_or_else(|| LedgerError::UnknownEvent(id.clone()))
    }

    /// The correction that replaced this entry, if one was recorded.
    pub fn superseded_by(&self, id: &EventId) -> Option<&EventId> {
        self.superseded_by.get(id)
    }

    /// Follows the correction chain to the entry that has not itself been superseded.
    ///
    /// Terminates because supersession always points from a later entry to an earlier one and
    /// forks are refused, so the links form a path, not a graph.
    pub fn current_version_of(&self, id: &EventId) -> Result<EventId, LedgerError> {
        self.require(id)?;
        let mut cursor = id.clone();
        while let Some(next) = self.superseded_by.get(&cursor) {
            cursor = next.clone();
        }
        Ok(cursor)
    }

    /// Appends an event, or explains why it could not be.
    ///
    /// This is 40.09's execution path: validate schema and actor, assign a monotonic id, append,
    /// and release anything the append unblocked.
    pub fn append(&mut self, event: Event) -> Result<AppendReceipt, LedgerError> {
        self.validate(&event)?;

        if let Some(admission) = self.check_idempotency(&event)? {
            return Ok(AppendReceipt {
                admission,
                released: Vec::new(),
            });
        }

        let missing = self.missing_parents(&event);
        if !missing.is_empty() {
            let key = event.idempotency_key();
            self.by_key.insert(key.clone(), quarantine_placeholder());
            self.quarantine.push(QuarantineEntry {
                key: key.clone(),
                event,
                missing: missing.clone(),
                note: None,
            });
            return Ok(AppendReceipt {
                admission: Admission::Quarantined { key, missing },
                released: Vec::new(),
            });
        }

        let seq = self.next_seq;
        let id = self.admit(event)?;
        let released = self.drain_quarantine()?;
        Ok(AppendReceipt {
            admission: Admission::Recorded { id, seq },
            released,
        })
    }

    /// Everything that must hold before an event can be admitted, independent of ordering.
    fn validate(&self, event: &Event) -> Result<(), LedgerError> {
        self.catalog.admits(&event.kind)?;

        if let Some(from) = self.window.answerable_from_record {
            if event.times.record < from {
                return Err(LedgerError::OutsideRetainedWindow {
                    axis: RecordTime::AXIS,
                    requested: event.times.record.to_string(),
                    retained_from: from.to_string(),
                });
            }
        }

        if let Some(target) = &event.supersedes {
            let original = self.require(target)?;
            if let Some(existing) = self.superseded_by.get(target) {
                return Err(LedgerError::AlreadySuperseded {
                    target: target.clone(),
                    by: existing.clone(),
                });
            }
            if event.times.record < original.event.times.record {
                return Err(LedgerError::CorrectionPrecedesOriginal {
                    target: target.clone(),
                    original: original.event.times.record.to_string(),
                    correction: event.times.record.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Resolves the submission against the keys already seen.
    ///
    /// An identical resubmission converges on the recorded entry; a different body under the
    /// same key is the 40.09 "duplicate idempotency key" failure, because silently accepting it
    /// would make the key meaningless and silently dropping it would lose an event.
    fn check_idempotency(&self, event: &Event) -> Result<Option<Admission>, LedgerError> {
        let key = event.idempotency_key();
        let digest = event.content_digest();

        if let Some(existing) = self.by_key.get(&key) {
            if let Some(entry) = self.get(existing) {
                return if entry.event.content_digest() == digest {
                    Ok(Some(Admission::Duplicate {
                        id: entry.id.clone(),
                    }))
                } else {
                    Err(LedgerError::IdempotencyConflict {
                        key: key.into(),
                        existing: entry.id.clone(),
                    })
                };
            }
            let Some(held) = self.quarantine.iter().find(|held| held.key == key) else {
                return Err(LedgerError::InvariantViolation {
                    detail: format!(
                        "idempotency key `{key}` is reserved but has neither a recorded entry nor a quarantine record"
                    ),
                });
            };
            return if held.event.content_digest() == digest {
                Ok(Some(Admission::Quarantined {
                    key,
                    missing: held.missing.clone(),
                }))
            } else {
                Err(LedgerError::IdempotencyConflict {
                    key: key.into(),
                    existing: quarantine_placeholder(),
                })
            };
        }
        Ok(None)
    }

    fn missing_parents(&self, event: &Event) -> Vec<EventId> {
        event
            .causal_parents()
            .iter()
            .filter(|parent| !self.by_id.contains_key(*parent))
            .cloned()
            .collect()
    }

    fn admit(&mut self, event: Event) -> Result<EventId, LedgerError> {
        let seq = self.next_seq;
        let next_seq =
            self.next_seq
                .checked_add(1)
                .ok_or_else(|| LedgerError::InvariantViolation {
                    detail: "event sequence exhausted at u64::MAX".to_string(),
                })?;
        let id = EventId::parse(format!("evt-{seq:012}"))?;
        let key = event.idempotency_key();
        let target = event.supersedes.clone();

        let entry = LedgerEntry::seal(seq, id.clone(), event, self.head());
        self.by_id.insert(id.clone(), self.entries.len());
        self.entries.push(entry);
        self.by_key.insert(key, id.clone());
        if let Some(target) = target {
            self.superseded_by.insert(target, id.clone());
        }
        self.next_seq = next_seq;
        Ok(id)
    }

    /// Admits everything whose blockers have arrived, repeating until nothing more moves.
    ///
    /// The loop is required rather than defensive: releasing one event can supply the parent
    /// another was waiting on, and a chain of out-of-order arrivals should resolve in one pass
    /// from the caller's point of view.
    fn drain_quarantine(&mut self) -> Result<Vec<EventId>, LedgerError> {
        let mut released = Vec::new();
        loop {
            let mut progressed = false;
            let mut index = 0;
            while index < self.quarantine.len() {
                let missing = self.missing_parents(&self.quarantine[index].event);
                if !missing.is_empty() {
                    self.quarantine[index].missing = missing;
                    index += 1;
                    continue;
                }
                match self.validate(&self.quarantine[index].event) {
                    Ok(()) => {
                        let held = self.quarantine.remove(index);
                        match self.admit(held.event.clone()) {
                            Ok(id) => released.push(id),
                            Err(error) => {
                                self.quarantine.insert(index, held);
                                return Err(error);
                            }
                        }
                        progressed = true;
                    }
                    Err(refusal) => {
                        self.quarantine[index].missing = Vec::new();
                        self.quarantine[index].note = Some(refusal.to_string());
                        index += 1;
                    }
                }
            }
            if !progressed {
                return Ok(released);
            }
        }
    }

    /// Re-derives the hash chain and the removal accounting.
    ///
    /// On an uncompacted ledger this is total: every entry's digest is recomputed from its own
    /// contents and every link is checked, so any edit, insertion or deletion is detected.
    ///
    /// On a compacted ledger, content tamper-evidence is still total, but link contiguity can
    /// only be checked across surviving neighbours. What replaces it is accounting: the number
    /// of absent sequence numbers must equal the number of removals the compaction anchors
    /// declare, and every anchor's digest must recompute. Removing one more entry than was
    /// declared is therefore still detected; what is lost is the ability to say *which* entries
    /// a dishonest operator removed.
    pub fn verify_chain(&self) -> ChainStatus {
        let mut previous: Option<&LedgerEntry> = None;
        for entry in &self.entries {
            if entry.recompute_digest() != entry.digest {
                return ChainStatus::broken(entry.seq, "entry digest does not match its contents");
            }
            match previous {
                None => {
                    if self.compactions.is_empty() && (entry.seq != 0 || !entry.previous.is_empty())
                    {
                        return ChainStatus::broken(entry.seq, "log does not begin at genesis");
                    }
                }
                Some(prior) => {
                    if entry.seq <= prior.seq {
                        return ChainStatus::broken(
                            entry.seq,
                            "sequence numbers are not ascending",
                        );
                    }
                    if entry.seq == prior.seq + 1 && entry.previous != prior.digest {
                        return ChainStatus::broken(
                            entry.seq,
                            "previous digest does not match the prior entry",
                        );
                    }
                }
            }
            previous = Some(entry);
        }

        let absent = self.next_seq.saturating_sub(self.entries.len() as u64);
        let declared: u64 = self
            .compactions
            .iter()
            .map(|anchor| anchor.removed_count)
            .sum();
        if absent != declared {
            return ChainStatus::broken(
                self.entries.first().map(|entry| entry.seq).unwrap_or(0),
                format!(
                    "{absent} sequence numbers are absent but {declared} removals are declared"
                ),
            );
        }
        for anchor in &self.compactions {
            if anchor.recompute_digest() != anchor.digest {
                return ChainStatus::broken(
                    anchor.head_seq_before,
                    "compaction anchor digest does not match its contents",
                );
            }
        }
        ChainStatus::Intact
    }

    /// Confirms that every causal edge points backwards in the log.
    ///
    /// A cycle cannot be constructed through `append`, because a parent must already be
    /// admitted, which means it already has a lower sequence number. This re-derives that from
    /// the stored entries rather than trusting the constructor — the property is what makes
    /// causal replay terminate.
    pub fn verify_causal_acyclicity(&self) -> ChainStatus {
        for entry in &self.entries {
            for parent in entry.event.causal_parents() {
                match self.get(parent) {
                    Some(ancestor) if ancestor.seq < entry.seq => {}
                    Some(ancestor) => {
                        return ChainStatus::broken(
                            entry.seq,
                            format!("causal parent {} does not precede it", ancestor.id),
                        );
                    }
                    None => {
                        return ChainStatus::broken(
                            entry.seq,
                            format!("causal parent {parent} is not retained"),
                        );
                    }
                }
            }
        }
        ChainStatus::Intact
    }

    /// Every place record time went backwards, in sequence order.
    pub fn clock_consistency(&self) -> Vec<ClockAnomaly> {
        let mut anomalies = Vec::new();
        let mut previous: Option<&LedgerEntry> = None;
        for entry in &self.entries {
            if let Some(prior) = previous {
                if entry.event.times.record < prior.event.times.record {
                    anomalies.push(ClockAnomaly {
                        seq: entry.seq,
                        previous_record: prior.event.times.record.to_string(),
                        record: entry.event.times.record.to_string(),
                    });
                }
            }
            previous = Some(entry);
        }
        anomalies
    }

    /// The entries visible at a cut, in sequence order.
    ///
    /// Visible means two things at once, and the second is what makes the ledger bitemporal
    /// rather than merely timestamped: the entry's own stamps fall inside the cut, *and* no
    /// entry that supersedes it is also inside the cut. A correction learned in 2023 does not
    /// retroactively hide the 2021 entry from a query asking what was known in 2022.
    pub fn cut(&self, cut: &TemporalCut) -> Result<Vec<&LedgerEntry>, LedgerError> {
        self.window.admits(cut)?;
        Ok(self.cut_unchecked(cut))
    }

    fn cut_unchecked(&self, cut: &TemporalCut) -> Vec<&LedgerEntry> {
        let in_cut: BTreeSet<&EventId> = self
            .entries
            .iter()
            .filter(|entry| cut.admits(&entry.event.times))
            .map(|entry| &entry.id)
            .collect();

        self.entries
            .iter()
            .filter(|entry| in_cut.contains(&entry.id))
            .filter(|entry| {
                self.superseded_by
                    .get(&entry.id)
                    .is_none_or(|corrector| !in_cut.contains(corrector))
            })
            .collect()
    }

    /// The state of the world implied by a cut: one entry per subject.
    ///
    /// The winner is the entry with the greatest valid time, with the sequence number breaking
    /// ties. Ordering by valid time rather than by arrival is the difference between "the most
    /// recent thing we were told" and "the most recent thing that was true", and only the
    /// second is a state.
    pub fn latest_by_subject(
        &self,
        cut: &TemporalCut,
    ) -> Result<BTreeMap<SubjectKey, &LedgerEntry>, LedgerError> {
        self.window.admits(cut)?;
        Ok(self.latest_by_subject_unchecked(cut))
    }

    fn latest_by_subject_unchecked(&self, cut: &TemporalCut) -> BTreeMap<SubjectKey, &LedgerEntry> {
        let mut state: BTreeMap<SubjectKey, &LedgerEntry> = BTreeMap::new();
        for entry in self.cut_unchecked(cut) {
            let key = entry.event.subject.clone();
            let better = match state.get(&key) {
                None => true,
                Some(current) => {
                    (entry.event.times.valid, entry.seq) > (current.event.times.valid, current.seq)
                }
            };
            if better {
                state.insert(key, entry);
            }
        }
        state
    }

    /// What the ledger says about one subject at a cut.
    pub fn state_of(
        &self,
        subject: &SubjectKey,
        cut: &TemporalCut,
    ) -> Result<Option<&LedgerEntry>, LedgerError> {
        Ok(self.latest_by_subject(cut)?.remove(subject))
    }

    /// Counts by transition family, for the operational view 40.09 asks for.
    pub fn class_histogram(&self) -> BTreeMap<EventClass, u64> {
        let mut counts = BTreeMap::new();
        for entry in &self.entries {
            *counts.entry(entry.event.class).or_insert(0) += 1;
        }
        counts
    }

    /// Destroys a payload while keeping the entry, its digest and the chain intact.
    ///
    /// 12.22's privacy deletion: "remove or cryptographically destroy protected payloads while
    /// retaining minimal non-identifying tombstones and impact notices". This is the one
    /// operation that touches a recorded entry, and it is safe only because the chain commits
    /// to the payload's digest rather than to its bytes — a fact the returned [`Redaction`]
    /// invites the caller to verify rather than assume.
    pub fn redact(
        &mut self,
        id: &EventId,
        reason: impl Into<String>,
    ) -> Result<Redaction, LedgerError> {
        let index = *self
            .by_id
            .get(id)
            .ok_or_else(|| LedgerError::UnknownEvent(id.clone()))?;
        let reason = reason.into();
        let entry = &mut self.entries[index];
        entry.event.payload.redact(reason.clone());
        Ok(Redaction {
            id: id.clone(),
            reason,
            payload_digest: entry.event.payload.digest().as_str().to_string(),
            entry_digest: entry.digest.clone(),
        })
    }

    /// Removes history older than the policy's bound, or reports what removal would do.
    ///
    /// Four things survive the bound regardless, and each corresponds to a way a naive
    /// truncation would leave the log lying:
    ///
    /// - **carry-forward**: the newest entry about each subject as of the bound, without which
    ///   a state query just inside the retained window would report that a subject the ledger
    ///   still knows about has no state at all;
    /// - **causal closure**: any parent of a survivor, without which
    ///   [`EventLedger::verify_causal_acyclicity`] would find a dangling edge;
    /// - **supersession closure**: the original of any surviving correction, without which
    ///   invariant 2 would hold only for the recent past;
    /// - **pins**: legal holds and public-release references (12.22's retention graph).
    ///
    /// The dry run is the default and mutates nothing; call [`RetentionPolicy::applying`] to
    /// actually delete.
    pub fn compact(&mut self, policy: &RetentionPolicy) -> Result<CompactionReport, LedgerError> {
        let bound = policy.compact_before_record;
        let candidates: BTreeSet<EventId> = self
            .entries
            .iter()
            .filter(|entry| entry.event.times.record < bound)
            .map(|entry| entry.id.clone())
            .collect();

        if candidates.is_empty() {
            return Ok(CompactionReport {
                dry_run: policy.dry_run,
                examined: 0,
                removed: 0,
                retained_by_carry_forward: BTreeSet::new(),
                retained_by_causal_closure: BTreeSet::new(),
                retained_by_supersession_closure: BTreeSet::new(),
                retained_by_pin: BTreeSet::new(),
                window: self.window,
                anchor: None,
            });
        }

        let boundary_cut = TemporalCut::known_at(bound_exclusive(self, bound));
        let carry_forward: BTreeSet<EventId> = self
            .latest_by_subject_unchecked(&boundary_cut)
            .values()
            .map(|entry| entry.id.clone())
            .filter(|id| candidates.contains(id))
            .collect();

        let pinned: BTreeSet<EventId> = policy
            .pinned
            .iter()
            .filter(|id| candidates.contains(*id))
            .cloned()
            .collect();

        let mut keep: BTreeSet<EventId> = carry_forward.union(&pinned).cloned().collect();
        let mut causal = BTreeSet::new();
        let mut supersession = BTreeSet::new();
        loop {
            let survivors: Vec<&LedgerEntry> = self
                .entries
                .iter()
                .filter(|entry| !candidates.contains(&entry.id) || keep.contains(&entry.id))
                .collect();
            let mut grew = false;
            let mut discovered = Vec::new();
            for survivor in survivors {
                for parent in survivor.event.causal_parents() {
                    if candidates.contains(parent) && !keep.contains(parent) {
                        discovered.push((parent.clone(), true));
                    }
                }
                if let Some(target) = &survivor.event.supersedes {
                    if candidates.contains(target) && !keep.contains(target) {
                        discovered.push((target.clone(), false));
                    }
                }
            }
            for (id, is_causal) in discovered {
                if keep.insert(id.clone()) {
                    grew = true;
                    if is_causal {
                        causal.insert(id);
                    } else {
                        supersession.insert(id);
                    }
                }
            }
            if !grew {
                break;
            }
        }

        let removed: Vec<EventId> = candidates.difference(&keep).cloned().collect();
        let latest_candidate_valid = self
            .entries
            .iter()
            .filter(|entry| candidates.contains(&entry.id))
            .map(|entry| entry.event.times.valid)
            .max();

        let projected = RetentionWindow {
            answerable_from_record: Some(bound),
            answerable_from_valid: latest_candidate_valid,
            earliest_retained_seq: self
                .entries
                .iter()
                .filter(|entry| !removed.contains(&entry.id))
                .map(|entry| entry.seq)
                .min()
                .unwrap_or(self.next_seq),
        };
        let window = self.window.tighten(projected);

        let mut report = CompactionReport {
            dry_run: policy.dry_run,
            examined: candidates.len() as u64,
            removed: removed.len() as u64,
            retained_by_carry_forward: carry_forward,
            retained_by_causal_closure: causal,
            retained_by_supersession_closure: supersession,
            retained_by_pin: pinned,
            window,
            anchor: None,
        };
        if policy.dry_run {
            return Ok(report);
        }

        let anchor = CompactionAnchor::seal(
            bound,
            removed.len() as u64,
            self.next_seq.saturating_sub(1),
            self.head(),
            window,
        );
        let doomed: BTreeSet<EventId> = removed.into_iter().collect();
        self.entries.retain(|entry| !doomed.contains(&entry.id));
        self.by_key.retain(|_, id| !doomed.contains(id));
        self.superseded_by.retain(|original, corrector| {
            !doomed.contains(original) && !doomed.contains(corrector)
        });
        self.reindex();
        self.window = window;
        self.compactions.push(anchor.clone());
        report.anchor = Some(anchor);
        Ok(report)
    }

    fn reindex(&mut self) {
        self.by_id = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| (entry.id.clone(), index))
            .collect();
    }
}

/// The id reserved for a key whose event is still quarantined.
///
/// Quarantined events have no sequence number yet, so they cannot have a real id; reserving the
/// key under a sentinel is what stops the same blocked event being queued twice.
fn quarantine_placeholder() -> EventId {
    EventId::parse("evt-quarantined").expect("literal is a well-formed identifier")
}

/// The largest record time strictly below the bound that any entry actually carries.
///
/// Cuts are inclusive, so selecting "everything before the bound" means cutting at the newest
/// stamp that is still below it rather than at the bound itself.
fn bound_exclusive(ledger: &EventLedger, bound: RecordTime) -> RecordTime {
    ledger
        .entries
        .iter()
        .map(|entry| entry.event.times.record)
        .filter(|record| *record < bound)
        .max()
        .unwrap_or(bound)
}

#[cfg(test)]
mod tests {
    //! Cases that need to reach inside the ledger to construct the state they check.
    //!
    //! A hash chain is only interesting when something tampers with it, and nothing outside this
    //! module can: `entries` is private and there is no mutating accessor. These tests forge the
    //! states an attacker would produce and confirm `verify_chain` names them.

    use super::*;
    use crate::event::{Actor, EventKind};
    use crate::time::{RecordTime, ValidTime};
    use crate::EventTimes;
    use serde_json::json;

    fn sample(subject: &str, day: u32) -> Event {
        let stamp = |prefix: &str| format!("2021-01-{day:02}T00:00:0{prefix}Z");
        Event::new(
            EventClass::Material,
            EventKind::parse("lesion.measured").expect("fixture kind"),
            Actor::new("registry-core", "curator").expect("fixture actor"),
            SubjectKey::parse(subject).expect("fixture subject"),
            EventTimes::published_on_record(
                ValidTime::parse(&stamp("0")).expect("fixture instant"),
                RecordTime::parse(&stamp("0")).expect("fixture instant"),
            ),
            json!({ "day": day }),
        )
        .expect("fixture event")
    }

    fn filled() -> EventLedger {
        let mut ledger = EventLedger::new();
        for day in 1..=6 {
            ledger
                .append(sample(&format!("patient-{}", day % 3), day))
                .expect("append succeeds");
        }
        ledger
    }

    #[test]
    fn rewriting_an_entry_in_place_is_detected_at_the_sequence_it_happened() {
        let mut ledger = filled();
        ledger.entries[3].event.subject =
            SubjectKey::parse("patient-substituted").expect("fixture subject");

        assert_eq!(
            ledger.verify_chain(),
            ChainStatus::broken(3, "entry digest does not match its contents")
        );
    }

    #[test]
    fn relinking_an_entry_to_hide_a_deletion_is_detected() {
        let mut ledger = filled();
        let doomed = ledger.entries.remove(2);
        ledger.entries[2].previous = doomed.previous.clone();
        ledger.by_id.clear();
        ledger.reindex();

        assert!(!ledger.verify_chain().is_intact());
    }

    #[test]
    fn an_undeclared_removal_from_a_compacted_ledger_is_still_detected_by_the_accounting() {
        let mut ledger = filled();
        ledger
            .compact(
                &RetentionPolicy::before(
                    RecordTime::parse("2021-01-04T00:00:00Z").expect("instant"),
                )
                .applying(),
            )
            .expect("compaction applies");
        assert_eq!(ledger.verify_chain(), ChainStatus::Intact);

        ledger.entries.pop();
        ledger.reindex();
        assert!(!ledger.verify_chain().is_intact());
    }

    #[test]
    fn a_quarantined_event_reserves_its_idempotency_key_under_a_sentinel_id() {
        let mut ledger = EventLedger::new();
        let key = ledger
            .append(sample("patient-1", 2).caused_by([quarantine_placeholder()]))
            .expect("quarantined")
            .admission;
        let Admission::Quarantined { key, .. } = key else {
            panic!("expected quarantine");
        };

        assert_eq!(ledger.by_key.get(&key), Some(&quarantine_placeholder()));
        assert!(ledger.get(&quarantine_placeholder()).is_none());
    }
}
