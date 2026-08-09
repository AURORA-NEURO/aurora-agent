//! The general infrastructure event ledger: append-only, bitemporal, projectable, compactable.
//!
//! Implements blueprint 40.09 (event ledger and temporal semantics) with the lifecycle
//! requirements of 40.06 (storage layout and data lifecycle) and section 12 (data and
//! infrastructure) — specifically 12.04's event and object models, 12.17's idempotency domains,
//! and 12.22's retention, garbage collection and privacy deletion.
//!
//! # Why this exists separately from `bioprism-weave`
//!
//! `bioprism-weave` already has a hash-chained ledger. That one records *coordination*: acts
//! between agents, the commitments they create and the claims they contest. This one records
//! *infrastructure*: every material, epistemic, execution, policy and evaluation transition the
//! platform makes, with the temporal semantics that make "what did we know, and when" a
//! question with an answer. The hash-chaining idea is shared and deliberately so; nothing else
//! is.
//!
//! # The one idea worth carrying away
//!
//! Three time axes, never conflated:
//!
//! | axis | question it answers | example of getting it wrong |
//! |---|---|---|
//! | [`ValidTime`] | when was it true in the world | dating a relapse by when the registry was updated |
//! | [`RecordTime`] | when did we learn it | claiming a 2021 decision used 2023 knowledge |
//! | [`ReleaseTime`] | when did it become readable | scoring a model on evidence unblinded after its cutoff |
//!
//! Each is a separate newtype with no conversions between them, so the mistakes above are
//! compile errors rather than plausible-looking numbers. `bioprism-fiber`'s temporal cut already
//! depends on availability being distinct from occurrence; this crate is where that distinction
//! is made structural.
//!
//! # Shape of the API
//!
//! ```
//! use bioprism_ledger::{
//!     Actor, Event, EventClass, EventKind, EventLedger, EventTimes, RecordTime, SubjectKey,
//!     TemporalCut, ValidTime,
//! };
//! use serde_json::json;
//!
//! let mut ledger = EventLedger::new();
//! let observed = Event::new(
//!     EventClass::Material,
//!     EventKind::parse("lesion.measured")?,
//!     Actor::new("radiology-core", "curator")?,
//!     SubjectKey::parse("patient-7/lesion-1")?,
//!     EventTimes::published_on_record(
//!         ValidTime::parse("2021-03-01T00:00:00Z")?,
//!         RecordTime::parse("2021-03-04T00:00:00Z")?,
//!     ),
//!     json!({ "diameter_mm": 14 }),
//! )?;
//! let receipt = ledger.append(observed)?;
//! assert!(receipt.admission.recorded_id().is_some());
//!
//! let known_before = ledger.cut(&TemporalCut::known_at(RecordTime::parse("2021-03-02T00:00:00Z")?))?;
//! assert!(known_before.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # What is deliberately not here
//!
//! - **Durability.** Everything is in memory in one `Vec`. There is no write-ahead log, no
//!   fsync, no crash recovery. 40.09's "append transactionally" and "update outbox" are storage
//!   concerns; the semantics they protect are implemented, the storage is not.
//! - **Concurrency.** Single process, single writer, no locks, no leases. 12.17's lease
//!   lifecycle belongs to the job worker, not here.
//! - **Clocks.** Nothing reads the system time. Every instant is supplied by the caller, which
//!   is what makes every digest in this crate reproducible on every machine.
//! - **Grace periods and legal-hold expiry.** Compaction deletes immediately or not at all;
//!   pins stand in for holds.
//! - **Subscriptions.** Projections are pulled, not pushed.

pub mod entry;
pub mod error;
pub mod event;
pub mod ledger;
pub mod projection;
pub mod retention;
pub mod time;

pub use entry::{ChainStatus, LedgerEntry};
pub use error::LedgerError;
pub use event::{
    Actor, Event, EventClass, EventKind, IdempotencyKey, Payload, PayloadBody, SchemaCatalog,
    SubjectKey,
};
pub use ledger::{Admission, AppendReceipt, ClockAnomaly, EventLedger, QuarantineEntry};
pub use projection::{
    Checkpoint, ClassCounts, LatestFact, Projection, ProjectionRun, SubjectLatest,
};
pub use retention::{
    CompactionAnchor, CompactionReport, Redaction, RetentionPolicy, RetentionWindow,
};
pub use time::{EventTimes, RecordTime, ReleaseTime, TemporalCut, ValidTime};
