//! Observability, telemetry and audit, built on the distinction between what was observed and what
//! was asserted.
//!
//! Implements blueprint 40.34 (Observability, Telemetry and Audit).
//!
//! # The distinction is `crates/safety`'s, and it is the load-bearing part
//!
//! `bioprism_safety::attest::Statement` splits an audit row into `Observed`, carrying a closed enum
//! of computations that process actually performed, and `Asserted`, carrying free text and the name
//! of whoever said it. That split is the reason a safety audit log is worth reading, and this
//! module applies it one level up, to metrics.
//!
//! An operational metric is a derived number. The failure mode is that a dashboard shows
//! `trace_coverage = 0.98` when nothing counted the denominator, and nobody can tell the difference
//! between a measured 0.98 and a hopeful one. So [`MetricValue`] has private fields, no public
//! constructor and no `Deserialize`, and the only way to obtain one is
//! [`MetricDefinition::evaluate`], which requires **every** input signal to carry an observed
//! sample. An asserted sample is stored, is visible, and does not count:
//! [`OpsError::UnsupportedMetric`] names the inputs that were missing. A derived metric no
//! observation supports is therefore not a number that is wrong, it is a value that does not exist.
//!
//! # One finding about the neighbour
//!
//! `bioprism_safety::attest::Observation` is closed over safety's own computations — a digest
//! comparison, a refused boundary crossing, a produced witness, a recomputed chain link — and there
//! is no `Other(String)`. That closure is correct and it has a consequence: **an operational metric
//! computed here cannot enter a safety audit log as observed**, however well supported it is,
//! because none of the five variants describes it. [`audit_statement`] therefore returns
//! `Statement::Asserted` and says so in its docs rather than reaching for a variant that would
//! misdescribe what happened. Widening safety's enum would be the alternative and it is not this
//! crate's to widen.
//!
//! # Telemetry is a projection, in the direction the type system can see
//!
//! 40.34's first two invariants are *domain events remain canonical* and *telemetry is an
//! import/export projection*. [`RedactionPolicy::project`] maps a [`DomainEvent`] to a
//! [`TelemetryRecord`] and there is no function anywhere in this crate with the opposite signature:
//! no `From<TelemetryRecord> for DomainEvent`, no `TelemetryRecord::into_event`, no `Deserialize`
//! on the record. A projection that could be inverted would be a second copy of the truth.
//!
//! Every projection returns a [`SemanticLoss`] beside the record, because 40.34 lists *export with
//! semantic-loss report* as a required output and an export that does not say what it dropped
//! invites a reader to treat it as complete.
//!
//! # Redaction denies by default and refuses to export the unclassified
//!
//! [`RedactionPolicy`] is keyed by `bioprism_scope::ScopeClass` rather than by field name, so a new
//! field carrying an identity is covered by the policy the moment it is classified. A class with no
//! declared treatment raises [`OpsError::RedactionMiss`]: forgetting to decide is not the same as
//! deciding to emit. `ScopeClass::Unclassified` may not be declared emittable at all
//! ([`OpsError::UnclassifiedEmission`]) — `bioprism-scope` introduced that variant precisely so an
//! unknown dimension is reported rather than treated as an opaque string, and exporting it would
//! undo that.
//!
//! # Trace identity is not event identity
//!
//! 40.34's fourth invariant is *trace IDs connect but do not define semantics*. Two records may
//! share a [`TraceId`]; that makes them related, not identical. [`ExportBatch::correlated`] returns
//! a slice for that reason and there is no `by_trace` returning one record. Identity is the domain
//! event id, and [`TelemetryRecord::check_projects`] is how a caller asserting that a record belongs
//! to an event finds out it does not — 40.34's `trace/domain event mismatch`, decidable.
//!
//! # What is deliberately not implemented
//!
//! * **No telemetry backend, no exporter, no OTLP, no network, no file, no process.** There is no
//!   OpenTelemetry dependency: this workspace builds offline against pinned crates and adding one
//!   is not available. [`ExportBatch`] is a `Vec` and exporting is something a caller does with it.
//!   40.34's `telemetry backend down` failure is consequently not modelled, because nothing here
//!   can be down.
//! * **No sampling.** 40.34's verification plan names sampling rules. A sampler that dropped
//!   records would make [`SemanticLoss`] a lie unless it were part of it, and choosing a sampling
//!   policy is a deployment decision.
//! * **No clock and no spans.** A [`TraceId`] is an opaque string the caller supplies; nothing here
//!   starts, ends or nests a span, and no duration is measured. Every ordering is an
//!   `Epoch`.
//! * **No log formatting.** 40.34 names structured logging under Interfaces. This crate emits
//!   values, not lines.
//! * **No audit storage.** The hash-linked log is `bioprism_safety::attest::AuditLog`, which is
//!   itself a `Vec`, and it is used rather than reimplemented.

use crate::error::{well_formed_name, OpsError};
use bioprism_safety::attest::Statement;
use bioprism_scope::ScopeClass;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The name of something measurable.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SignalId(String);

impl SignalId {
    pub fn parse(value: impl Into<String>) -> Result<Self, OpsError> {
        Ok(SignalId(well_formed_name("signal id", &value.into())?))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SignalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for SignalId {
    type Error = OpsError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        SignalId::parse(value)
    }
}

impl From<SignalId> for String {
    fn from(value: SignalId) -> Self {
        value.0
    }
}

/// How a sample came to exist. The same split `bioprism_safety::attest::Statement` draws.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provenance", rename_all = "snake_case")]
pub enum Sampling {
    /// Something counted it. `by` names the instrument, so a reader can go and look at it.
    Observed { by: String },
    /// Somebody said it. Recorded, visible, and never an input to a derived metric.
    Asserted { by: String },
}

impl Sampling {
    pub fn is_observed(&self) -> bool {
        matches!(self, Sampling::Observed { .. })
    }

    pub fn by(&self) -> &str {
        match self {
            Sampling::Observed { by } | Sampling::Asserted { by } => by,
        }
    }
}

/// One reading of one signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub signal: SignalId,
    pub value: f64,
    pub sampling: Sampling,
    pub epoch: u64,
}

impl Sample {
    pub fn observed(signal: SignalId, value: f64, by: impl Into<String>, epoch: u64) -> Self {
        Sample {
            signal,
            value,
            sampling: Sampling::Observed { by: by.into() },
            epoch,
        }
    }

    pub fn asserted(signal: SignalId, value: f64, by: impl Into<String>, epoch: u64) -> Self {
        Sample {
            signal,
            value,
            sampling: Sampling::Asserted { by: by.into() },
            epoch,
        }
    }
}

/// The samples available to a metric evaluation.
///
/// Holds asserted samples as well as observed ones. Keeping them is the point: a reader can see
/// that somebody claimed a denominator, and see that the metric was refused anyway.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Observations {
    samples: BTreeMap<SignalId, Sample>,
}

impl Observations {
    pub fn new() -> Self {
        Observations::default()
    }

    /// Records a sample, replacing any earlier one for the same signal.
    pub fn record(mut self, sample: Sample) -> Self {
        self.samples.insert(sample.signal.clone(), sample);
        self
    }

    /// The value, if and only if something observed it.
    pub fn observed(&self, signal: &SignalId) -> Option<f64> {
        self.samples
            .get(signal)
            .filter(|sample| sample.sampling.is_observed())
            .map(|sample| sample.value)
    }

    pub fn get(&self, signal: &SignalId) -> Option<&Sample> {
        self.samples.get(signal)
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// The signals present but merely asserted, in id order. The list a reader should be shown
    /// beside a refused metric.
    pub fn asserted_signals(&self) -> Vec<&SignalId> {
        self.samples
            .values()
            .filter(|sample| !sample.sampling.is_observed())
            .map(|sample| &sample.signal)
            .collect()
    }
}

/// How a metric is computed from signals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "derivation", rename_all = "snake_case")]
pub enum Derivation {
    /// One signal, reported as itself.
    Passthrough { signal: SignalId },
    /// A coverage or rate. Both halves must be observed, which is the case this module exists for.
    Ratio {
        numerator: SignalId,
        denominator: SignalId,
    },
    /// A total over several signals. Every one of them must be observed; a sum over a subset is a
    /// different quantity wearing the same name.
    Sum { signals: Vec<SignalId> },
    /// A difference, for gauges like queue age.
    Difference {
        minuend: SignalId,
        subtrahend: SignalId,
    },
}

impl Derivation {
    /// Every signal the derivation reads, in evaluation order.
    pub fn inputs(&self) -> Vec<&SignalId> {
        match self {
            Derivation::Passthrough { signal } => vec![signal],
            Derivation::Ratio {
                numerator,
                denominator,
            } => vec![numerator, denominator],
            Derivation::Sum { signals } => signals.iter().collect(),
            Derivation::Difference {
                minuend,
                subtrahend,
            } => vec![minuend, subtrahend],
        }
    }
}

/// A named derived metric.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetricDefinition {
    name: String,
    derivation: Derivation,
    unit: String,
}

impl MetricDefinition {
    pub fn new(
        name: impl Into<String>,
        derivation: Derivation,
        unit: impl Into<String>,
    ) -> Result<Self, OpsError> {
        Ok(MetricDefinition {
            name: well_formed_name("metric name", &name.into())?,
            derivation,
            unit: unit.into(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn derivation(&self) -> &Derivation {
        &self.derivation
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// The only way to obtain a [`MetricValue`].
    ///
    /// Fails when any input signal is absent or merely asserted, and fails separately when the
    /// arithmetic does not determine an answer — a ratio over a zero denominator is not zero and is
    /// not one, and reporting either is the lie this refuses.
    pub fn evaluate(&self, observations: &Observations) -> Result<MetricValue, OpsError> {
        let inputs = self.derivation.inputs();
        let missing: Vec<String> = inputs
            .iter()
            .filter(|signal| observations.observed(signal).is_none())
            .map(|signal| signal.to_string())
            .collect();
        if !missing.is_empty() {
            return Err(OpsError::UnsupportedMetric {
                metric: self.name.clone(),
                missing,
            });
        }

        let read = |signal: &SignalId| observations.observed(signal).expect("checked above");
        let value = match &self.derivation {
            Derivation::Passthrough { signal } => read(signal),
            Derivation::Ratio {
                numerator,
                denominator,
            } => {
                let bottom = read(denominator);
                if bottom == 0.0 {
                    return Err(OpsError::IndeterminateMetric {
                        metric: self.name.clone(),
                        reason: format!("{denominator} was observed as zero"),
                    });
                }
                read(numerator) / bottom
            }
            Derivation::Sum { signals } => signals.iter().map(read).sum(),
            Derivation::Difference {
                minuend,
                subtrahend,
            } => read(minuend) - read(subtrahend),
        };

        Ok(MetricValue {
            metric: self.name.clone(),
            unit: self.unit.clone(),
            value,
            supported_by: inputs.into_iter().cloned().collect(),
        })
    }
}

/// A metric that observation supports.
///
/// Private fields, no public constructor, `Serialize` and not `Deserialize`. The pattern is
/// `bioprism_fiber::View`'s: a value whose meaning is "this was computed here" must not be
/// reconstructible from a document somebody wrote.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MetricValue {
    metric: String,
    unit: String,
    value: f64,
    supported_by: Vec<SignalId>,
}

impl MetricValue {
    pub fn metric(&self) -> &str {
        &self.metric
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// The observed signals this value rests on. Never empty.
    pub fn supported_by(&self) -> &[SignalId] {
        &self.supported_by
    }
}

impl fmt::Display for MetricValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} = {} {}", self.metric, self.value, self.unit)
    }
}

/// An audit statement about a metric, for `bioprism_safety::attest::AuditLog`.
///
/// Always `Asserted`, and the reason is in the module docs: safety's `Observation` enum is closed
/// over safety's own computations and none of its five variants describes an operational metric. A
/// `MetricValue` cannot exist without observation, so what is being asserted is narrow and true —
/// but it is being asserted, and the log says so.
pub fn audit_statement(actor: impl Into<String>, metric: &MetricValue) -> Statement {
    Statement::asserted(
        actor,
        format!(
            "{} = {} {} supported by {}",
            metric.metric,
            metric.value,
            metric.unit,
            metric
                .supported_by
                .iter()
                .map(SignalId::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        ),
    )
}

/// What a redaction policy does with a class of field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "treatment", rename_all = "snake_case")]
pub enum Treatment {
    /// Exported as it is.
    Emit,
    /// Exported at reduced resolution. `to` names what survives, so the loss report can say it.
    Coarsen { to: String },
    /// Not exported.
    Drop,
}

/// A field of a domain event, with the scope class that decides its treatment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub value: Value,
    pub class: ScopeClass,
}

impl Field {
    pub fn new(value: Value, class: ScopeClass) -> Self {
        Field { value, class }
    }
}

/// A canonical domain event. The thing telemetry is a projection *of*.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DomainEvent {
    pub id: String,
    pub kind: String,
    pub fields: BTreeMap<String, Field>,
    pub epoch: u64,
}

impl DomainEvent {
    pub fn new(id: impl Into<String>, kind: impl Into<String>, epoch: u64) -> Self {
        DomainEvent {
            id: id.into(),
            kind: kind.into(),
            fields: BTreeMap::new(),
            epoch,
        }
    }

    pub fn with_field(mut self, name: impl Into<String>, field: Field) -> Self {
        self.fields.insert(name.into(), field);
        self
    }
}

/// An opaque correlation handle.
///
/// Carries no semantics. Nothing in this crate decides anything from a trace id, and nothing looks
/// an event up by one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TraceId(String);

impl TraceId {
    pub fn new(value: impl Into<String>) -> Self {
        TraceId(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Per-class export rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionPolicy {
    version: String,
    treatments: BTreeMap<ScopeClass, Treatment>,
}

impl RedactionPolicy {
    pub fn new(version: impl Into<String>) -> Self {
        RedactionPolicy {
            version: version.into(),
            treatments: BTreeMap::new(),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Declares a treatment, refusing to make unclassified fields emittable.
    pub fn declare(mut self, class: ScopeClass, treatment: Treatment) -> Result<Self, OpsError> {
        if class == ScopeClass::Unclassified && treatment == Treatment::Emit {
            return Err(OpsError::UnclassifiedEmission {
                policy: self.version.clone(),
            });
        }
        self.treatments.insert(class, treatment);
        Ok(self)
    }

    /// Projects an event into a telemetry record and the loss that projection caused.
    ///
    /// Deny by default: a field whose class has no declared treatment is a failure, not an
    /// emission.
    pub fn project(&self, event: &DomainEvent, trace: TraceId) -> Result<Projected, OpsError> {
        let mut attributes = Map::new();
        let mut dropped = Vec::new();
        let mut coarsened = Vec::new();

        for (name, field) in &event.fields {
            let treatment =
                self.treatments
                    .get(&field.class)
                    .ok_or_else(|| OpsError::RedactionMiss {
                        field: name.clone(),
                        class: field.class.as_str().to_string(),
                        policy: self.version.clone(),
                    })?;
            match treatment {
                Treatment::Emit => {
                    attributes.insert(name.clone(), field.value.clone());
                }
                Treatment::Coarsen { to } => {
                    attributes.insert(name.clone(), Value::String(to.clone()));
                    coarsened.push(name.clone());
                }
                Treatment::Drop => dropped.push(name.clone()),
            }
        }

        Ok(Projected {
            record: TelemetryRecord {
                event_id: event.id.clone(),
                kind: event.kind.clone(),
                trace,
                attributes,
                epoch: event.epoch,
                policy: self.version.clone(),
            },
            loss: SemanticLoss { dropped, coarsened },
        })
    }
}

/// What a projection gave up. 40.34's "export with semantic-loss report".
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct SemanticLoss {
    pub dropped: Vec<String>,
    pub coarsened: Vec<String>,
}

impl SemanticLoss {
    pub fn is_lossless(&self) -> bool {
        self.dropped.is_empty() && self.coarsened.is_empty()
    }
}

/// A record and the loss that producing it caused, together.
///
/// A pair rather than two returns, so that a caller who wants the record has the loss report in
/// hand and has to discard it deliberately.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Projected {
    pub record: TelemetryRecord,
    pub loss: SemanticLoss,
}

/// The exportable projection of a domain event.
///
/// Private fields, `Serialize` only, and there is no inverse anywhere: no `Deserialize`, no
/// `into_event`, no `From<TelemetryRecord> for DomainEvent`. 40.34's first two invariants say the
/// domain event is canonical and telemetry is a projection of it; a type that could be turned back
/// would be a second original.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TelemetryRecord {
    event_id: String,
    kind: String,
    trace: TraceId,
    attributes: Map<String, Value>,
    epoch: u64,
    policy: String,
}

impl TelemetryRecord {
    /// The identity of the record, which is the identity of the event it projects.
    pub fn event_id(&self) -> &str {
        &self.event_id
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The correlation handle. Connects; does not identify.
    pub fn trace(&self) -> &TraceId {
        &self.trace
    }

    pub fn attributes(&self) -> &Map<String, Value> {
        &self.attributes
    }

    pub fn policy(&self) -> &str {
        &self.policy
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Checks a caller's claim that this record projects that event. 40.34's `trace/domain event
    /// mismatch`.
    pub fn check_projects(&self, event: &DomainEvent) -> Result<(), OpsError> {
        if self.event_id == event.id {
            return Ok(());
        }
        Err(OpsError::TraceMismatch {
            record: self.event_id.clone(),
            event: event.id.clone(),
        })
    }
}

/// A batch of records held for export.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ExportBatch {
    records: Vec<TelemetryRecord>,
}

impl ExportBatch {
    pub fn new() -> Self {
        ExportBatch::default()
    }

    pub fn push(&mut self, projected: Projected) {
        self.records.push(projected.record);
    }

    pub fn records(&self) -> &[TelemetryRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The record projecting a given event, of which there is at most one.
    pub fn by_event(&self, event_id: &str) -> Option<&TelemetryRecord> {
        self.records.iter().find(|r| r.event_id == event_id)
    }

    /// Every record sharing a trace, of which there may be any number.
    ///
    /// Returns a collection and not an option on purpose. A trace groups; it does not name.
    pub fn correlated(&self, trace: &TraceId) -> Vec<&TelemetryRecord> {
        self.records.iter().filter(|r| &r.trace == trace).collect()
    }
}

/// A ceiling on how many distinct label values a signal may carry.
///
/// 40.34 lists `cardinality explosion` as a failure and cardinality as an operational metric.
/// A budget that is checked at record time turns the explosion into a refusal at the point it
/// happens, where the offending label is still in hand.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LabelBudget {
    ceiling: usize,
    seen: BTreeMap<SignalId, BTreeSet<String>>,
}

impl LabelBudget {
    pub fn new(ceiling: usize) -> Self {
        LabelBudget {
            ceiling,
            seen: BTreeMap::new(),
        }
    }

    /// Records a label against a signal.
    ///
    /// Refuses the label that would exceed the ceiling and does not record it, so a caller that
    /// ignores the error does not get a budget that has silently grown past its own limit.
    pub fn record(&mut self, signal: &SignalId, label: impl Into<String>) -> Result<(), OpsError> {
        let label = label.into();
        let entry = self.seen.entry(signal.clone()).or_default();
        if entry.contains(&label) {
            return Ok(());
        }
        if entry.len() + 1 > self.ceiling {
            return Err(OpsError::CardinalityExceeded {
                signal: signal.to_string(),
                distinct: entry.len() + 1,
                ceiling: self.ceiling,
            });
        }
        entry.insert(label);
        Ok(())
    }

    pub fn distinct(&self, signal: &SignalId) -> usize {
        self.seen.get(signal).map(BTreeSet::len).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_safety::attest::{AuditEvent, AuditLog, AuditRecord};

    fn signal(name: &str) -> SignalId {
        SignalId::parse(name).expect("well-formed")
    }

    fn coverage() -> MetricDefinition {
        MetricDefinition::new(
            "trace_coverage",
            Derivation::Ratio {
                numerator: signal("spans_emitted"),
                denominator: signal("operations_total"),
            },
            "ratio",
        )
        .expect("well-formed")
    }

    #[test]
    fn a_derived_metric_no_observation_supports_cannot_be_obtained() {
        let observations = Observations::new().record(Sample::observed(
            signal("spans_emitted"),
            98.0,
            "span-counter",
            1,
        ));
        let error = coverage().evaluate(&observations).unwrap_err();
        match error {
            OpsError::UnsupportedMetric { metric, missing } => {
                assert_eq!(metric, "trace_coverage");
                assert_eq!(missing, vec!["operations_total".to_string()]);
            }
            other => panic!("expected an unsupported metric, got {other}"),
        }
    }

    #[test]
    fn an_asserted_denominator_does_not_support_a_coverage_metric() {
        let observations = Observations::new()
            .record(Sample::observed(
                signal("spans_emitted"),
                98.0,
                "counter",
                1,
            ))
            .record(Sample::asserted(
                signal("operations_total"),
                100.0,
                "operator",
                1,
            ));
        let error = coverage().evaluate(&observations).unwrap_err();
        assert!(matches!(error, OpsError::UnsupportedMetric { .. }));
        assert_eq!(observations.asserted_signals().len(), 1);
    }

    #[test]
    fn a_metric_value_carries_the_observations_that_support_it() {
        let observations = Observations::new()
            .record(Sample::observed(
                signal("spans_emitted"),
                98.0,
                "counter",
                1,
            ))
            .record(Sample::observed(
                signal("operations_total"),
                100.0,
                "counter",
                1,
            ));
        let value = coverage().evaluate(&observations).expect("supported");
        assert!((value.value() - 0.98).abs() < 1e-12);
        assert_eq!(value.supported_by().len(), 2);
        assert!(!value.supported_by().is_empty());
    }

    #[test]
    fn a_ratio_over_a_zero_denominator_is_indeterminate_rather_than_zero_or_one() {
        let observations = Observations::new()
            .record(Sample::observed(signal("spans_emitted"), 0.0, "counter", 1))
            .record(Sample::observed(
                signal("operations_total"),
                0.0,
                "counter",
                1,
            ));
        let error = coverage().evaluate(&observations).unwrap_err();
        assert!(matches!(error, OpsError::IndeterminateMetric { .. }));
    }

    #[test]
    fn a_sum_over_a_subset_of_its_signals_is_refused_rather_than_reported() {
        let definition = MetricDefinition::new(
            "export_bytes_total",
            Derivation::Sum {
                signals: vec![signal("bytes_metrics"), signal("bytes_traces")],
            },
            "bytes",
        )
        .unwrap();
        let observations =
            Observations::new().record(Sample::observed(signal("bytes_metrics"), 12.0, "sink", 1));
        assert!(matches!(
            definition.evaluate(&observations).unwrap_err(),
            OpsError::UnsupportedMetric { .. }
        ));
    }

    #[test]
    fn an_ops_metric_enters_the_safety_audit_log_asserted_because_its_observation_set_is_closed() {
        let observations = Observations::new()
            .record(Sample::observed(
                signal("spans_emitted"),
                98.0,
                "counter",
                1,
            ))
            .record(Sample::observed(
                signal("operations_total"),
                100.0,
                "counter",
                1,
            ));
        let value = coverage().evaluate(&observations).unwrap();
        let statement = audit_statement("ops", &value);
        assert!(
            !statement.is_observed(),
            "safety's Observation enum is closed over safety's own computations"
        );

        let mut log = AuditLog::new();
        log.append(AuditRecord::new(
            AuditEvent::PolicyChange,
            "ops",
            "trace_coverage",
            statement,
            1,
        ))
        .expect("appends");
        assert_eq!(log.assertions().len(), 1);
        assert!(log.verify().is_ok());
    }

    fn policy() -> RedactionPolicy {
        RedactionPolicy::new("v1")
            .declare(ScopeClass::Identity, Treatment::Drop)
            .unwrap()
            .declare(
                ScopeClass::Specimen,
                Treatment::Coarsen {
                    to: "cohort".into(),
                },
            )
            .unwrap()
            .declare(ScopeClass::Policy, Treatment::Emit)
            .unwrap()
    }

    fn event() -> DomainEvent {
        DomainEvent::new("evt-1", "compile.finished", 7)
            .with_field(
                "subject",
                Field::new(Value::String("PT-0042".into()), ScopeClass::Identity),
            )
            .with_field(
                "specimen",
                Field::new(Value::String("SPC-9".into()), ScopeClass::Specimen),
            )
            .with_field(
                "policy",
                Field::new(Value::String("research-only".into()), ScopeClass::Policy),
            )
    }

    #[test]
    fn a_projection_reports_exactly_what_it_dropped_and_what_it_coarsened() {
        let projected = policy()
            .project(&event(), TraceId::new("trace-a"))
            .expect("projects");
        assert_eq!(projected.loss.dropped, vec!["subject".to_string()]);
        assert_eq!(projected.loss.coarsened, vec!["specimen".to_string()]);
        assert!(!projected.loss.is_lossless());
        assert!(!projected.record.attributes().contains_key("subject"));
        assert_eq!(
            projected.record.attributes().get("specimen"),
            Some(&Value::String("cohort".into()))
        );
    }

    #[test]
    fn a_field_whose_class_has_no_declared_treatment_is_refused_rather_than_emitted() {
        let event = event().with_field(
            "region",
            Field::new(Value::String("posterior-fossa".into()), ScopeClass::Region),
        );
        let error = policy()
            .project(&event, TraceId::new("trace-a"))
            .unwrap_err();
        match error {
            OpsError::RedactionMiss { field, class, .. } => {
                assert_eq!(field, "region");
                assert_eq!(class, "region");
            }
            other => panic!("expected a redaction miss, got {other}"),
        }
    }

    #[test]
    fn a_policy_cannot_declare_unclassified_fields_emittable() {
        let error = RedactionPolicy::new("v1")
            .declare(ScopeClass::Unclassified, Treatment::Emit)
            .unwrap_err();
        assert!(matches!(error, OpsError::UnclassifiedEmission { .. }));
        assert!(RedactionPolicy::new("v1")
            .declare(ScopeClass::Unclassified, Treatment::Drop)
            .is_ok());
    }

    #[test]
    fn two_records_sharing_a_trace_id_are_not_the_same_event() {
        let policy = policy();
        let trace = TraceId::new("trace-a");
        let mut batch = ExportBatch::new();
        batch.push(policy.project(&event(), trace.clone()).unwrap());
        batch.push(
            policy
                .project(
                    &DomainEvent::new("evt-2", "compile.finished", 8),
                    trace.clone(),
                )
                .unwrap(),
        );
        assert_eq!(batch.correlated(&trace).len(), 2);
        assert_eq!(batch.by_event("evt-1").unwrap().event_id(), "evt-1");
        assert_ne!(batch.records()[0].event_id(), batch.records()[1].event_id());
    }

    #[test]
    fn a_record_correlated_with_an_event_it_does_not_project_is_a_typed_mismatch() {
        let projected = policy().project(&event(), TraceId::new("trace-a")).unwrap();
        assert!(projected.record.check_projects(&event()).is_ok());
        let error = projected
            .record
            .check_projects(&DomainEvent::new("evt-2", "compile.finished", 8))
            .unwrap_err();
        assert!(matches!(error, OpsError::TraceMismatch { .. }));
    }

    #[test]
    fn a_label_that_would_exceed_the_cardinality_budget_is_refused_and_not_recorded() {
        let mut budget = LabelBudget::new(2);
        let signal = signal("compile.duration");
        budget.record(&signal, "world-a").unwrap();
        budget.record(&signal, "world-b").unwrap();
        budget.record(&signal, "world-a").expect("a repeat is free");
        let error = budget.record(&signal, "world-c").unwrap_err();
        assert!(matches!(error, OpsError::CardinalityExceeded { .. }));
        assert_eq!(budget.distinct(&signal), 2);
    }
}
