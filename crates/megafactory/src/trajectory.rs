//! Trajectory capture and research-workflow mining.
//!
//! Blueprint 35.06: instrument notebooks, command lines, agents, tools and review decisions to
//! discover authentic decision boundaries.
//!
//! Most of that is instrumentation. There is no notebook kernel here, no OpenTelemetry exporter, no
//! shell hook and no process to attach to; a library of plain Rust types cannot capture anything.
//! `bioprism-trace` already owns the captured artifact downstream — its trace IR, its
//! mandatory import-loss report, its divergence comparison and its candidate ranking — and this
//! module does not restate any of it.
//!
//! Two of 35.06's required components are owned by nobody, and both are predicates:
//!
//! ## Privacy-aware redaction, with the digest taken afterwards
//!
//! A field is [`Field::Recorded`], [`Field::Redacted`] with a reason, or [`Field::NotCaptured`].
//! Three states, no `Default`, and [`Field::value`] returns `Option<&str>` for the two that have no
//! value — a redacted secret and a field the instrumentation never saw are different facts about
//! the world and must not share a representation.
//!
//! The sharp part is ordering. [`RedactedSession::digest`] hashes the session **after** redaction,
//! and that is not a stylistic choice: a digest taken over the raw payload is a confirmation oracle.
//! Anyone holding a guess at the secret can hash their guess and compare. Publishing a redacted
//! transcript beside a digest of its unredacted form leaks exactly the values the redaction was
//! for. [`RedactedSession`] has no constructor that takes a pre-computed digest, so the ordering
//! cannot be got wrong by a caller; it is a property of the type rather than a note in a runbook.
//!
//! [`LeakageScan`] closes the loop for the caller who knows what the secrets were: it searches the
//! serialised redacted session for supplied probe strings and reports what it found. 35.06 lists
//! privacy leakage as an operational metric; this is the only form of it that can be measured
//! without an adversary.
//!
//! ## A gap is a gap, not a zero
//!
//! Spans carry a monotone sequence number assigned by the producer. [`Completeness`] is either
//! `Complete` or `Gapped` with the missing numbers listed, and
//! [`CaptureSession::require_complete`] is the gate [`crate::boundary`] passes through. A boundary
//! inferred across a gap is a guess about events nobody recorded, and the workspace's position is
//! that a right answer from an incomplete basis is not a pass.
//!
//! ## What is measured here and what is not
//!
//! Trace completeness and privacy leakage: measured. Decision yield: [`crate::boundary`]'s.
//! Author review agreement: [`crate::boundary`]'s, because agreement is against annotated
//! boundaries rather than against spans. **Capture overhead is not measured** — it is a runtime
//! cost of instrumentation that does not exist in this crate, and a number for it would be
//! invented. [`RedactedSession::redaction_share`] reports how much of the session was withheld,
//! which is a different quantity and is labelled as one.

use crate::error::CaptureError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

/// What kind of thing a captured span records.
///
/// Deliberately coarse. This is the shape of an instrumentation event, not a taxonomy of decisions;
/// the decision taxonomy is [`crate::boundary::BoundaryKind`] and it answers a different question.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanKind {
    /// A cell, command, or agent step ran.
    Execution,
    /// A tool or external service was called.
    Tool,
    /// Something failed.
    Error,
    /// A person recorded a judgement in the workflow.
    Annotation,
    /// A file, dataset, or model state was snapshotted.
    Snapshot,
}

impl SpanKind {
    pub const ALL: [SpanKind; 5] = [
        SpanKind::Execution,
        SpanKind::Tool,
        SpanKind::Error,
        SpanKind::Annotation,
        SpanKind::Snapshot,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SpanKind::Execution => "execution",
            SpanKind::Tool => "tool",
            SpanKind::Error => "error",
            SpanKind::Annotation => "annotation",
            SpanKind::Snapshot => "snapshot",
        }
    }
}

/// One field of a span, in the three states a captured field can be in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "field", rename_all = "snake_case")]
pub enum Field {
    Recorded {
        value: String,
    },
    /// The instrumentation saw it and a policy removed it. The reason is required.
    Redacted {
        reason: String,
    },
    /// The instrumentation never saw it. Not an empty value and not a redaction.
    NotCaptured,
}

impl Field {
    pub fn recorded(value: impl Into<String>) -> Self {
        Field::Recorded {
            value: value.into(),
        }
    }

    pub fn redacted(reason: impl Into<String>) -> Self {
        Field::Redacted {
            reason: reason.into(),
        }
    }

    /// The value, when there is one. No placeholder is returned for the other two states.
    pub fn value(&self) -> Option<&str> {
        match self {
            Field::Recorded { value } => Some(value.as_str()),
            Field::Redacted { .. } | Field::NotCaptured => None,
        }
    }

    pub fn is_redacted(&self) -> bool {
        matches!(self, Field::Redacted { .. })
    }
}

/// One captured span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    /// Producer-assigned, monotone, and the only ordering this module trusts. Wall-clock order is
    /// not authoritative for concurrent tool calls and is not carried.
    pub seq: u64,
    pub kind: SpanKind,
    pub fields: BTreeMap<String, Field>,
}

impl Span {
    pub fn new(seq: u64, kind: SpanKind) -> Self {
        Span {
            seq,
            kind,
            fields: BTreeMap::new(),
        }
    }

    pub fn with(mut self, name: impl Into<String>, field: Field) -> Self {
        self.fields.insert(name.into(), field);
        self
    }

    pub fn field(&self, name: &str) -> Option<&Field> {
        self.fields.get(name)
    }
}

/// Whether the captured sequence has holes in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "completeness", rename_all = "snake_case")]
pub enum Completeness {
    Complete {
        spans: usize,
    },
    /// Sequence numbers the producer skipped. Listed, not counted, so a report names them.
    Gapped {
        missing: Vec<u64>,
    },
}

impl Completeness {
    pub fn is_complete(&self) -> bool {
        matches!(self, Completeness::Complete { .. })
    }
}

/// Missing sequence numbers strictly between the first and last span captured.
///
/// Only the interior is examined. Nothing here can know whether capture started late or stopped
/// early, and reporting a guess about the ends as a gap would be inventing evidence of loss.
fn completeness_of(spans: &[Span]) -> Completeness {
    let (Some(first), Some(last)) = (spans.first(), spans.last()) else {
        return Completeness::Complete { spans: 0 };
    };
    let present: BTreeSet<u64> = spans.iter().map(|span| span.seq).collect();
    let missing: Vec<u64> = (first.seq..=last.seq)
        .filter(|seq| !present.contains(seq))
        .collect();
    if missing.is_empty() {
        Completeness::Complete { spans: spans.len() }
    } else {
        Completeness::Gapped { missing }
    }
}

/// A captured research session, before redaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "CaptureSessionDocument")]
pub struct CaptureSession {
    pub id: String,
    spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
struct CaptureSessionDocument {
    id: String,
    spans: Vec<Span>,
}

impl TryFrom<CaptureSessionDocument> for CaptureSession {
    type Error = CaptureError;

    fn try_from(document: CaptureSessionDocument) -> Result<Self, Self::Error> {
        let mut session = CaptureSession::new(document.id);
        for span in document.spans {
            session.append(span)?;
        }
        Ok(session)
    }
}

impl CaptureSession {
    pub fn new(id: impl Into<String>) -> Self {
        CaptureSession {
            id: id.into(),
            spans: Vec::new(),
        }
    }

    /// Appends a span, refusing a repeated or out-of-order sequence number.
    ///
    /// Out-of-order is an error rather than a sort because the producer's ordering is the only
    /// evidence of ordering there is; silently sorting would manufacture a sequence the
    /// instrumentation never observed.
    pub fn append(&mut self, span: Span) -> Result<(), CaptureError> {
        if let Some(last) = self.spans.last() {
            if span.seq == last.seq {
                return Err(CaptureError::DuplicateSequence {
                    session: self.id.clone(),
                    seq: span.seq,
                });
            }
            if span.seq < last.seq {
                return Err(CaptureError::OutOfOrder {
                    session: self.id.clone(),
                    seq: span.seq,
                    previous: last.seq,
                });
            }
        }
        self.spans.push(span);
        Ok(())
    }

    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Whether the captured sequence has interior holes.
    pub fn completeness(&self) -> Completeness {
        completeness_of(&self.spans)
    }

    /// The gate a boundary compilation passes through.
    pub fn require_complete(&self) -> Result<(), CaptureError> {
        match self.completeness() {
            Completeness::Complete { .. } => Ok(()),
            Completeness::Gapped { missing } => Err(CaptureError::GappedSession {
                session: self.id.clone(),
                missing,
            }),
        }
    }

    /// Applies `policy`, producing a session whose digest is taken over redacted content.
    ///
    /// Consumes the session. The unredacted original is not retained anywhere in the result, so a
    /// caller who wants to keep it has to keep it deliberately rather than by accident.
    pub fn redact(self, policy: &RedactionPolicy) -> Result<RedactedSession, CaptureError> {
        let carried: BTreeSet<&str> = self
            .spans
            .iter()
            .flat_map(|span| span.fields.keys().map(String::as_str))
            .collect();
        for target in &policy.fields {
            if !carried.contains(target.as_str()) {
                return Err(CaptureError::RedactionTargetAbsent(target.clone()));
            }
        }

        let mut redacted_fields = 0usize;
        let mut total_fields = 0usize;
        let spans: Vec<Span> = self
            .spans
            .into_iter()
            .map(|span| {
                let Span { seq, kind, fields } = span;
                let fields = fields
                    .into_iter()
                    .map(|(name, field)| {
                        total_fields += 1;
                        if policy.fields.contains(&name) && !matches!(field, Field::NotCaptured) {
                            redacted_fields += 1;
                            (name, Field::redacted(policy.reason.clone()))
                        } else {
                            (name, field)
                        }
                    })
                    .collect();
                Span { seq, kind, fields }
            })
            .collect();

        Ok(RedactedSession {
            id: self.id,
            spans,
            policy: policy.clone(),
            redacted_fields,
            total_fields,
        })
    }
}

/// Which field names to remove, and the reason recorded in their place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    pub fields: BTreeSet<String>,
    pub reason: String,
}

impl RedactionPolicy {
    /// Builds a policy. `reason` lands in every redacted field, so it is what a reader of the
    /// released session will see instead of the value.
    pub fn new(
        fields: impl IntoIterator<Item = impl Into<String>>,
        reason: impl Into<String>,
    ) -> Self {
        RedactionPolicy {
            fields: fields.into_iter().map(Into::into).collect(),
            reason: reason.into(),
        }
    }
}

/// A session after redaction. The only form this module will digest or release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RedactedSessionDocument")]
pub struct RedactedSession {
    pub id: String,
    spans: Vec<Span>,
    pub policy: RedactionPolicy,
    pub redacted_fields: usize,
    pub total_fields: usize,
}

#[derive(Debug, Deserialize)]
struct RedactedSessionDocument {
    id: String,
    spans: Vec<Span>,
    policy: RedactionPolicy,
    redacted_fields: usize,
    total_fields: usize,
}

impl TryFrom<RedactedSessionDocument> for RedactedSession {
    type Error = CaptureError;

    fn try_from(document: RedactedSessionDocument) -> Result<Self, Self::Error> {
        let mut previous = None;
        let mut observed_redacted = 0usize;
        let mut observed_total = 0usize;
        for span in &document.spans {
            if let Some(previous) = previous {
                if span.seq == previous {
                    return Err(CaptureError::DuplicateSequence {
                        session: document.id.clone(),
                        seq: span.seq,
                    });
                }
                if span.seq < previous {
                    return Err(CaptureError::OutOfOrder {
                        session: document.id.clone(),
                        seq: span.seq,
                        previous,
                    });
                }
            }
            previous = Some(span.seq);
            for field in span.fields.values() {
                observed_total += 1;
                if field.is_redacted() {
                    observed_redacted += 1;
                }
            }
        }
        if observed_redacted != document.redacted_fields || observed_total != document.total_fields
        {
            return Err(CaptureError::InconsistentAccounting {
                session: document.id,
                redacted: document.redacted_fields,
                total: document.total_fields,
                observed_redacted,
                observed_total,
            });
        }
        Ok(RedactedSession {
            id: document.id,
            spans: document.spans,
            policy: document.policy,
            redacted_fields: document.redacted_fields,
            total_fields: document.total_fields,
        })
    }
}

impl RedactedSession {
    pub fn spans(&self) -> &[Span] {
        &self.spans
    }

    pub fn len(&self) -> usize {
        self.spans.len()
    }

    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Whether the released sequence has interior holes. Redaction removes values, never spans.
    pub fn completeness(&self) -> Completeness {
        completeness_of(&self.spans)
    }

    /// The digest of the released session, taken over redacted content only.
    ///
    /// There is no constructor that accepts a digest computed elsewhere, and the raw session is
    /// consumed by [`CaptureSession::redact`], so no digest over unredacted content can reach a
    /// report through this type. See the module docs for why that ordering is a confidentiality
    /// property and not a convention.
    pub fn digest(&self) -> ContentHash {
        let body = self.canonical_body();
        ContentHash::of_value(&body).expect("spans serialise to finite JSON")
    }

    fn canonical_body(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "spans": self.spans,
            "policy": self.policy,
        })
    }

    /// Fraction of captured fields withheld.
    ///
    /// Not capture overhead. 35.06's capture-overhead metric is a runtime cost this crate cannot
    /// observe; this is how much of what was captured did not survive the policy.
    pub fn redaction_share(&self) -> f64 {
        if self.total_fields == 0 {
            0.0
        } else {
            self.redacted_fields as f64 / self.total_fields as f64
        }
    }

    /// Searches the released bytes for values the caller knows should not be there.
    pub fn scan_for_leakage(&self, probes: &[String]) -> LeakageScan {
        let serialised =
            serde_json::to_string(&self.canonical_body()).expect("spans serialise to finite JSON");
        let leaked: Vec<String> = probes
            .iter()
            .filter(|probe| !probe.is_empty() && serialised.contains(probe.as_str()))
            .cloned()
            .collect();
        LeakageScan {
            probes: probes.len(),
            leaked,
            session: self.id.clone(),
        }
    }
}

/// What a leakage scan found in the released bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakageScan {
    pub session: String,
    pub probes: usize,
    /// Probe strings still present after redaction. Non-empty means the policy missed a field.
    pub leaked: Vec<String>,
}

impl LeakageScan {
    /// Whether the scan found nothing.
    ///
    /// Not the same as "the session is safe". A scan can only find secrets the caller thought to
    /// supply, which is why the probe count travels with the result.
    pub fn found_nothing(&self) -> bool {
        self.leaked.is_empty()
    }
}
