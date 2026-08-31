//! Bounded OpenTelemetry Protocol JSON ingestion.
//!
//! OTLP is an integration boundary, not a second trajectory model. This adapter accepts the JSON
//! export shape (`resourceSpans -> scopeSpans -> spans`), preserves the source span beside the
//! normalized Event IR payload, and records every semantic decision that is not directly carried by
//! the wire format. In particular, inferred event kinds, missing timestamps, unresolved parents,
//! duplicate attributes, and fields that are retained but not interpreted keep a trace from being
//! called lossless or compilable.
//!
//! The adapter deliberately does not create an exporter, read a clock, contact an OTLP endpoint,
//! or claim support for vendor-specific conventions. It is a deterministic importer for recorded
//! JSON, which makes it useful offline while keeping the boundary honest.

use crate::event::{Event, EventKind, Trace};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// The largest span count accepted by one import, independent of the caller's byte bound.
pub const MAX_SPANS: usize = 100_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OtelError {
    #[error("invalid OTLP JSON: {0}")]
    InvalidJson(String),
    #[error("OTLP document must be an object containing a resourceSpans array")]
    InvalidRoot,
    #[error("max_spans must be between 1 and {MAX_SPANS}")]
    InvalidLimit,
    #[error("OTLP document contains more than {maximum} spans")]
    TooManySpans { maximum: usize },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelFieldLoss {
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelDroppedSpan {
    pub path: String,
    pub name: Option<String>,
    pub detail: String,
}

/// Semantic and structural information the adapter could not carry into a safe Event IR.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelLoss {
    pub dropped_spans: Vec<OtelDroppedSpan>,
    pub dropped_span_events: Vec<OtelFieldLoss>,
    pub unmapped_fields: Vec<OtelFieldLoss>,
    pub duplicate_attributes: Vec<OtelFieldLoss>,
    pub inferred_kinds: Vec<OtelFieldLoss>,
    pub missing_start_times: Vec<OtelFieldLoss>,
    pub unresolved_parents: Vec<OtelFieldLoss>,
    pub multiple_trace_ids: Vec<OtelFieldLoss>,
}

impl OtelLoss {
    pub fn is_lossless(&self) -> bool {
        self.dropped_spans.is_empty()
            && self.dropped_span_events.is_empty()
            && self.unmapped_fields.is_empty()
            && self.duplicate_attributes.is_empty()
            && self.inferred_kinds.is_empty()
            && self.missing_start_times.is_empty()
            && self.unresolved_parents.is_empty()
            && self.multiple_trace_ids.is_empty()
    }

    pub fn dropped_events(&self) -> usize {
        self.dropped_spans.len() + self.dropped_span_events.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OtelMapping {
    pub format: String,
    pub resource_count: usize,
    pub scope_count: usize,
    pub source_span_count: usize,
    pub accepted_span_count: usize,
    pub span_event_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OtelIngestion {
    trace: Trace,
    loss: OtelLoss,
    mapping: OtelMapping,
}

impl OtelIngestion {
    pub fn trace(&self) -> &Trace {
        &self.trace
    }

    pub fn loss(&self) -> &OtelLoss {
        &self.loss
    }

    pub fn mapping(&self) -> &OtelMapping {
        &self.mapping
    }

    pub fn is_compilable(&self) -> bool {
        !self.trace.is_empty()
            && self.loss.is_lossless()
            && crate::ingest::validate(&self.trace).is_ok()
    }

    pub fn into_parts(self) -> (Trace, OtelLoss, OtelMapping) {
        (self.trace, self.loss, self.mapping)
    }
}

#[derive(Debug, Clone)]
struct Attribute {
    key: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct PendingSpan {
    event: Event,
    span_id: String,
    parent_span_id: Option<String>,
    start_time_unix_nano: Option<u64>,
    input_order: usize,
}

/// Import an OTLP JSON export into the existing trajectory IR.
///
/// The caller supplies the logical trajectory id and success flag because OTLP records telemetry,
/// not the benchmark's outcome semantics. A source export containing multiple trace ids is kept
/// for inspection but marked non-compilable in the loss report.
pub fn from_otlp_json(
    trace_id: impl Into<String>,
    text: &str,
    succeeded: bool,
    max_spans: usize,
) -> Result<OtelIngestion, OtelError> {
    if max_spans == 0 || max_spans > MAX_SPANS {
        return Err(OtelError::InvalidLimit);
    }
    let root: Value =
        serde_json::from_str(text).map_err(|error| OtelError::InvalidJson(error.to_string()))?;
    let Some(root_object) = root.as_object() else {
        return Err(OtelError::InvalidRoot);
    };
    let Some(resource_spans) = root_object.get("resourceSpans").and_then(Value::as_array) else {
        return Err(OtelError::InvalidRoot);
    };

    let mut loss = OtelLoss::default();
    record_unknown_fields(root_object, &["resourceSpans"], "root", &mut loss);

    let mut pending = Vec::new();
    let mut resource_count = 0;
    let mut scope_count = 0;
    let mut source_span_count = 0;
    let mut span_event_count = 0;
    let mut trace_ids = BTreeSet::new();

    for (resource_index, raw_resource_span) in resource_spans.iter().enumerate() {
        let resource_path = format!("resourceSpans[{resource_index}]");
        let Some(resource_span) = raw_resource_span.as_object() else {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: resource_path,
                detail: "resource span entry is not an object".into(),
            });
            continue;
        };
        resource_count += 1;
        record_unknown_fields(
            resource_span,
            &[
                "resource",
                "scopeSpans",
                "instrumentationLibrarySpans",
                "schemaUrl",
            ],
            &resource_path,
            &mut loss,
        );

        let resource_attributes = parse_attributes(
            resource_span
                .get("resource")
                .and_then(Value::as_object)
                .and_then(|resource| resource.get("attributes")),
            &format!("{resource_path}.resource.attributes"),
            &mut loss,
        );
        if let Some(resource) = resource_span.get("resource").and_then(Value::as_object) {
            record_unknown_fields(
                resource,
                &["attributes", "droppedAttributesCount", "schemaUrl"],
                &format!("{resource_path}.resource"),
                &mut loss,
            );
        }

        let current_scopes = resource_span.get("scopeSpans");
        let legacy_scopes = resource_span.get("instrumentationLibrarySpans");
        if current_scopes.is_some() && legacy_scopes.is_some() {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: resource_path.clone(),
                detail: "both scopeSpans and instrumentationLibrarySpans are present; only scopeSpans is imported".into(),
            });
        }
        let scopes = current_scopes.or(legacy_scopes);
        let Some(scopes) = scopes.and_then(Value::as_array) else {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: resource_path,
                detail: "resource span has no scopeSpans array".into(),
            });
            continue;
        };

        for (scope_index, raw_scope) in scopes.iter().enumerate() {
            let scope_path = format!("{resource_path}.scopeSpans[{scope_index}]");
            let Some(scope) = raw_scope.as_object() else {
                loss.unmapped_fields.push(OtelFieldLoss {
                    path: scope_path,
                    detail: "scope span entry is not an object".into(),
                });
                continue;
            };
            scope_count += 1;
            record_unknown_fields(scope, &["scope", "spans"], &scope_path, &mut loss);
            if let Some(scope_metadata) = scope.get("scope").and_then(Value::as_object) {
                record_unknown_fields(
                    scope_metadata,
                    &["name", "version", "attributes", "droppedAttributesCount"],
                    &format!("{scope_path}.scope"),
                    &mut loss,
                );
                if scope_metadata
                    .get("attributes")
                    .and_then(Value::as_array)
                    .is_some_and(|attributes| !attributes.is_empty())
                {
                    loss.unmapped_fields.push(OtelFieldLoss {
                        path: format!("{scope_path}.scope.attributes"),
                        detail: "instrumentation scope attributes are retained in source metadata but not projected into Event.visible".into(),
                    });
                }
            }
            let Some(spans) = scope.get("spans").and_then(Value::as_array) else {
                loss.unmapped_fields.push(OtelFieldLoss {
                    path: scope_path,
                    detail: "scope span entry has no spans array".into(),
                });
                continue;
            };

            for (span_index, raw_span) in spans.iter().enumerate() {
                source_span_count += 1;
                if source_span_count > max_spans {
                    return Err(OtelError::TooManySpans { maximum: max_spans });
                }
                let span_path = format!("{scope_path}.spans[{span_index}]");
                let Some(span) = raw_span.as_object() else {
                    loss.dropped_spans.push(OtelDroppedSpan {
                        path: span_path,
                        name: None,
                        detail: "span entry is not an object".into(),
                    });
                    continue;
                };
                let name = span
                    .get("name")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned);
                let Some(trace_id_value) = span
                    .get("traceId")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    loss.dropped_spans.push(OtelDroppedSpan {
                        path: span_path,
                        name,
                        detail: "span has no non-empty traceId string".into(),
                    });
                    continue;
                };
                let Some(span_id) = span
                    .get("spanId")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                else {
                    loss.dropped_spans.push(OtelDroppedSpan {
                        path: span_path,
                        name,
                        detail: "span has no non-empty spanId string".into(),
                    });
                    continue;
                };
                let Some(name) = name else {
                    loss.dropped_spans.push(OtelDroppedSpan {
                        path: span_path,
                        name: None,
                        detail: "span has no name string".into(),
                    });
                    continue;
                };

                trace_ids.insert(trace_id_value.to_owned());
                let span_attributes = parse_attributes(
                    span.get("attributes"),
                    &format!("{span_path}.attributes"),
                    &mut loss,
                );
                let span_events = parse_span_events(
                    span.get("events"),
                    &format!("{span_path}.events"),
                    &mut loss,
                );
                span_event_count += span_events.len();
                let start_time_unix_nano = parse_u64(
                    span.get("startTimeUnixNano"),
                    &format!("{span_path}.startTimeUnixNano"),
                    &mut loss,
                );
                if start_time_unix_nano.is_none() {
                    loss.missing_start_times.push(OtelFieldLoss {
                        path: format!("{span_path}.startTimeUnixNano"),
                        detail: "span is retained but import order is used after timestamp sorting"
                            .into(),
                    });
                }
                if let Some(links) = span.get("links").and_then(Value::as_array) {
                    if !links.is_empty() {
                        loss.unmapped_fields.push(OtelFieldLoss {
                            path: format!("{span_path}.links"),
                            detail: "span links are retained but are not converted into Event IR causal parents".into(),
                        });
                    }
                } else if span.get("links").is_some() {
                    loss.unmapped_fields.push(OtelFieldLoss {
                        path: format!("{span_path}.links"),
                        detail: "span links must be an array".into(),
                    });
                }
                record_unknown_fields(
                    span,
                    &[
                        "traceId",
                        "spanId",
                        "parentSpanId",
                        "name",
                        "kind",
                        "startTimeUnixNano",
                        "endTimeUnixNano",
                        "attributes",
                        "events",
                        "links",
                        "status",
                        "traceState",
                        "flags",
                    ],
                    &span_path,
                    &mut loss,
                );

                let kind = event_kind(&name, &span_attributes, &span_path, &mut loss);
                let mut payload = Map::new();
                payload.insert("source".into(), Value::String("opentelemetry".into()));
                payload.insert("name".into(), Value::String(name.clone()));
                payload.insert("trace_id".into(), Value::String(trace_id_value.into()));
                payload.insert("span_id".into(), Value::String(span_id.into()));
                payload.insert(
                    "parent_span_id".into(),
                    span.get("parentSpanId").cloned().unwrap_or(Value::Null),
                );
                payload.insert(
                    "span_kind".into(),
                    span.get("kind").cloned().unwrap_or(Value::Null),
                );
                payload.insert(
                    "start_time_unix_nano".into(),
                    span.get("startTimeUnixNano")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                payload.insert(
                    "end_time_unix_nano".into(),
                    span.get("endTimeUnixNano").cloned().unwrap_or(Value::Null),
                );
                payload.insert("attributes".into(), attributes_value(&span_attributes));
                payload.insert(
                    "resource_attributes".into(),
                    attributes_value(&resource_attributes),
                );
                payload.insert("events".into(), Value::Array(span_events));
                payload.insert(
                    "links".into(),
                    span.get("links")
                        .cloned()
                        .unwrap_or_else(|| Value::Array(Vec::new())),
                );
                payload.insert(
                    "status".into(),
                    span.get("status").cloned().unwrap_or(Value::Null),
                );
                payload.insert(
                    "trace_state".into(),
                    span.get("traceState").cloned().unwrap_or(Value::Null),
                );
                payload.insert(
                    "flags".into(),
                    span.get("flags").cloned().unwrap_or(Value::Null),
                );
                payload.insert("raw_span".into(), Value::Object(span.clone()));

                let visible = resource_attributes
                    .iter()
                    .chain(span_attributes.iter())
                    .map(|attribute| attribute.key.clone())
                    .collect::<Vec<_>>();
                let parent_span_id = match span.get("parentSpanId") {
                    Some(Value::String(value)) if !value.is_empty() => Some(value.clone()),
                    Some(Value::String(_)) | None => None,
                    Some(_) => {
                        loss.unmapped_fields.push(OtelFieldLoss {
                            path: format!("{span_path}.parentSpanId"),
                            detail: "parentSpanId is not a string and cannot be resolved".into(),
                        });
                        None
                    }
                };
                pending.push(PendingSpan {
                    event: Event::new(0, kind, Value::Object(payload)).seeing(visible),
                    span_id: span_id.to_owned(),
                    parent_span_id,
                    start_time_unix_nano,
                    input_order: pending.len(),
                });
            }
        }
    }

    if trace_ids.len() > 1 {
        loss.multiple_trace_ids.push(OtelFieldLoss {
            path: "resourceSpans".into(),
            detail: format!(
                "one trajectory contains {} source trace ids: {}",
                trace_ids.len(),
                trace_ids.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
        });
    }

    pending.sort_by_key(|span| {
        (
            span.start_time_unix_nano.unwrap_or(u64::MAX),
            span.input_order,
        )
    });
    let mut span_steps = BTreeMap::new();
    for (step, span) in pending.iter().enumerate() {
        if span_steps.insert(span.span_id.clone(), step).is_some() {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: format!("spanId:{}", span.span_id),
                detail: "duplicate spanId makes parent resolution ambiguous".into(),
            });
        }
    }

    let mut events = Vec::with_capacity(pending.len());
    for (step, mut span) in pending.into_iter().enumerate() {
        span.event.step = step;
        if let Some(parent_span_id) = span.parent_span_id {
            match span_steps.get(&parent_span_id).copied() {
                Some(parent_step) if parent_step < step => {
                    span.event.caused_by = Some(parent_step);
                }
                Some(parent_step) => loss.unresolved_parents.push(OtelFieldLoss {
                    path: format!("spanId:{}", span.span_id),
                    detail: format!(
                        "parent span resolves to step {parent_step}, not an earlier step"
                    ),
                }),
                None => loss.unresolved_parents.push(OtelFieldLoss {
                    path: format!("spanId:{}", span.span_id),
                    detail: format!("parent span {parent_span_id} is not present in this export"),
                }),
            }
        }
        events.push(span.event);
    }

    let accepted_span_count = events.len();
    Ok(OtelIngestion {
        trace: Trace::new(trace_id, events, succeeded),
        loss,
        mapping: OtelMapping {
            format: "otlp_json".into(),
            resource_count,
            scope_count,
            source_span_count,
            accepted_span_count,
            span_event_count,
        },
    })
}

fn record_unknown_fields(
    object: &Map<String, Value>,
    known: &[&str],
    path: &str,
    loss: &mut OtelLoss,
) {
    for field in object
        .keys()
        .filter(|field| !known.contains(&field.as_str()))
    {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: format!("{path}.{field}"),
            detail: "field is retained in raw_span or source metadata but not interpreted by this adapter".into(),
        });
    }
}

fn parse_attributes(raw: Option<&Value>, path: &str, loss: &mut OtelLoss) -> Vec<Attribute> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Some(values) = raw.as_array() else {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: path.into(),
            detail: "attributes must be an array".into(),
        });
        return Vec::new();
    };
    let mut attributes = Vec::with_capacity(values.len());
    for (index, raw_attribute) in values.iter().enumerate() {
        let attribute_path = format!("{path}[{index}]");
        let Some(attribute) = raw_attribute.as_object() else {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: attribute_path,
                detail: "attribute is not an object".into(),
            });
            continue;
        };
        let Some(key) = attribute.get("key").and_then(Value::as_str) else {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: attribute_path,
                detail: "attribute has no key string".into(),
            });
            continue;
        };
        record_unknown_fields(attribute, &["key", "value"], &attribute_path, loss);
        let value = normalize_value(
            attribute.get("value"),
            &format!("{attribute_path}.value"),
            loss,
        );
        if attributes.iter().any(|item: &Attribute| item.key == key) {
            loss.duplicate_attributes.push(OtelFieldLoss {
                path: format!("{path}.{key}"),
                detail: "duplicate OTLP attribute key retained in source order".into(),
            });
        }
        attributes.push(Attribute {
            key: key.into(),
            value,
        });
    }
    attributes
}

fn parse_span_events(raw: Option<&Value>, path: &str, loss: &mut OtelLoss) -> Vec<Value> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    let Some(values) = raw.as_array() else {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: path.into(),
            detail: "span events must be an array".into(),
        });
        return Vec::new();
    };
    let mut events = Vec::with_capacity(values.len());
    for (index, raw_event) in values.iter().enumerate() {
        let event_path = format!("{path}[{index}]");
        let Some(event) = raw_event.as_object() else {
            loss.dropped_span_events.push(OtelFieldLoss {
                path: event_path,
                detail: "span event is not an object".into(),
            });
            continue;
        };
        let Some(name) = event.get("name").and_then(Value::as_str) else {
            loss.dropped_span_events.push(OtelFieldLoss {
                path: event_path,
                detail: "span event has no name string".into(),
            });
            continue;
        };
        record_unknown_fields(
            event,
            &["name", "timeUnixNano", "attributes"],
            &event_path,
            loss,
        );
        let event_time = parse_u64(
            event.get("timeUnixNano"),
            &format!("{event_path}.timeUnixNano"),
            loss,
        );
        if event_time.is_none() {
            loss.missing_start_times.push(OtelFieldLoss {
                path: format!("{event_path}.timeUnixNano"),
                detail: "span event is retained without a timestamp".into(),
            });
        }
        let attributes = parse_attributes(
            event.get("attributes"),
            &format!("{event_path}.attributes"),
            loss,
        );
        events.push(serde_json::json!({
            "name": name,
            "time_unix_nano": event.get("timeUnixNano").cloned().unwrap_or(Value::Null),
            "attributes": attributes_value(&attributes),
        }));
    }
    events
}

fn normalize_value(raw: Option<&Value>, path: &str, loss: &mut OtelLoss) -> Value {
    let Some(raw) = raw else {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: path.into(),
            detail: "attribute has no value object".into(),
        });
        return Value::Null;
    };
    let Some(object) = raw.as_object() else {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: path.into(),
            detail: "attribute value is not an OTLP typed value object".into(),
        });
        return Value::Null;
    };
    let variants = [
        "stringValue",
        "boolValue",
        "intValue",
        "doubleValue",
        "bytesValue",
        "arrayValue",
        "kvlistValue",
    ];
    let present = variants
        .iter()
        .filter(|variant| object.contains_key(**variant))
        .copied()
        .collect::<Vec<_>>();
    if present.len() != 1 {
        loss.unmapped_fields.push(OtelFieldLoss {
            path: path.into(),
            detail: format!(
                "expected exactly one OTLP value variant, found {}",
                present.len()
            ),
        });
        return Value::Null;
    }
    record_unknown_fields(object, &variants, path, loss);
    match present[0] {
        "stringValue" | "boolValue" | "intValue" | "doubleValue" | "bytesValue" => {
            object.get(present[0]).cloned().unwrap_or(Value::Null)
        }
        "arrayValue" => {
            let values = object
                .get("arrayValue")
                .and_then(Value::as_object)
                .and_then(|array| array.get("values"))
                .and_then(Value::as_array);
            let Some(values) = values else {
                loss.unmapped_fields.push(OtelFieldLoss {
                    path: format!("{path}.arrayValue.values"),
                    detail: "arrayValue has no values array".into(),
                });
                return Value::Array(Vec::new());
            };
            Value::Array(
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        normalize_value(
                            Some(value),
                            &format!("{path}.arrayValue.values[{index}]"),
                            loss,
                        )
                    })
                    .collect(),
            )
        }
        "kvlistValue" => {
            let values = object
                .get("kvlistValue")
                .and_then(Value::as_object)
                .and_then(|list| list.get("values"))
                .and_then(Value::as_array);
            let Some(values) = values else {
                loss.unmapped_fields.push(OtelFieldLoss {
                    path: format!("{path}.kvlistValue.values"),
                    detail: "kvlistValue has no values array".into(),
                });
                return Value::Object(Map::new());
            };
            let mut map = Map::new();
            for (index, value) in values.iter().enumerate() {
                let value_path = format!("{path}.kvlistValue.values[{index}]");
                let Some(value_object) = value.as_object() else {
                    loss.unmapped_fields.push(OtelFieldLoss {
                        path: value_path,
                        detail: "kvlist attribute is not an object".into(),
                    });
                    continue;
                };
                let Some(key) = value_object.get("key").and_then(Value::as_str) else {
                    loss.unmapped_fields.push(OtelFieldLoss {
                        path: value_path,
                        detail: "kvlist attribute has no key string".into(),
                    });
                    continue;
                };
                map.insert(
                    key.into(),
                    normalize_value(
                        value_object.get("value"),
                        &format!("{value_path}.value"),
                        loss,
                    ),
                );
            }
            Value::Object(map)
        }
        _ => {
            loss.unmapped_fields.push(OtelFieldLoss {
                path: path.into(),
                detail: "OTLP value variant is not supported".into(),
            });
            Value::Null
        }
    }
}

fn parse_u64(raw: Option<&Value>, path: &str, loss: &mut OtelLoss) -> Option<u64> {
    let raw = raw?;
    if let Some(value) = raw.as_u64() {
        return Some(value);
    }
    if let Some(value) = raw.as_str() {
        if let Ok(parsed) = value.parse::<u64>() {
            return Some(parsed);
        }
    }
    loss.unmapped_fields.push(OtelFieldLoss {
        path: path.into(),
        detail: "uint64 field is neither a non-negative JSON number nor a decimal string".into(),
    });
    None
}

fn attributes_value(attributes: &[Attribute]) -> Value {
    Value::Array(
        attributes
            .iter()
            .map(|attribute| serde_json::json!({ "key": attribute.key, "value": attribute.value }))
            .collect(),
    )
}

fn event_kind(name: &str, attributes: &[Attribute], path: &str, loss: &mut OtelLoss) -> EventKind {
    let explicit = ["prism.event.kind", "aurora.event.kind"]
        .iter()
        .find_map(|key| attribute_value(attributes, key).and_then(Value::as_str));
    if let Some(value) = explicit {
        if let Some(kind) = parse_event_kind(value) {
            return kind;
        }
        loss.unmapped_fields.push(OtelFieldLoss {
            path: format!("{path}.attributes"),
            detail: format!("unsupported explicit event kind {value:?}; a name inference is used"),
        });
    }

    let lower = name.to_ascii_lowercase();
    let inferred = if lower.contains("goal") || lower.contains("task") {
        EventKind::Goal
    } else if lower.contains("choice") || lower.contains("decision") {
        EventKind::Choice
    } else if lower.contains("tool")
        || lower.contains("action")
        || lower.contains("invoke")
        || lower.contains("execute")
    {
        EventKind::Action
    } else if lower.contains("result")
        || lower.contains("response")
        || lower.contains("complete")
        || lower.contains("finish")
    {
        EventKind::Result
    } else if lower.contains("claim") || lower.contains("assert") {
        EventKind::Claim
    } else if lower.contains("termination") || lower.contains("shutdown") {
        EventKind::Termination
    } else {
        EventKind::Observation
    };
    loss.inferred_kinds.push(OtelFieldLoss {
        path: format!("{path}.name"),
        detail: format!("event kind {:?} inferred from span name {name:?}; add prism.event.kind for compilable evidence", inferred.as_str()),
    });
    inferred
}

fn parse_event_kind(value: &str) -> Option<EventKind> {
    match value {
        "goal" => Some(EventKind::Goal),
        "observation" => Some(EventKind::Observation),
        "choice" => Some(EventKind::Choice),
        "action" => Some(EventKind::Action),
        "result" => Some(EventKind::Result),
        "claim" => Some(EventKind::Claim),
        "termination" => Some(EventKind::Termination),
        _ => None,
    }
}

fn attribute_value<'a>(attributes: &'a [Attribute], key: &str) -> Option<&'a Value> {
    attributes
        .iter()
        .rev()
        .find(|attribute| attribute.key == key)
        .map(|attribute| &attribute.value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::validate;

    fn document(spans: Value) -> String {
        serde_json::json!({
            "resourceSpans": [{
                "resource": {"attributes": [
                    {"key": "service.name", "value": {"stringValue": "fixture-agent"}}
                ]},
                "scopeSpans": [{
                    "scope": {"name": "fixture", "version": "1"},
                    "spans": spans
                }]
            }]
        })
        .to_string()
    }

    #[test]
    fn imports_explicit_kinds_parentage_attributes_and_span_events() {
        let text = document(serde_json::json!([
            {
                "traceId": "trace-a",
                "spanId": "root",
                "name": "agent.goal",
                "startTimeUnixNano": "10",
                "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "goal"}}]
            },
            {
                "traceId": "trace-a",
                "spanId": "child",
                "parentSpanId": "root",
                "name": "agent.tool.call",
                "startTimeUnixNano": "20",
                "attributes": [
                    {"key": "prism.event.kind", "value": {"stringValue": "action"}},
                    {"key": "gen_ai.request.model", "value": {"stringValue": "fixture"}}
                ],
                "events": [{
                    "name": "tool.input",
                    "timeUnixNano": "21",
                    "attributes": [{"key": "arg.count", "value": {"intValue": "2"}}]
                }]
            }
        ]));
        let ingestion = from_otlp_json("trajectory", &text, false, 100).unwrap();
        assert!(ingestion.loss().is_lossless());
        assert!(ingestion.is_compilable());
        assert_eq!(ingestion.mapping().accepted_span_count, 2);
        assert_eq!(ingestion.mapping().span_event_count, 1);
        assert_eq!(ingestion.trace().events[0].kind, EventKind::Goal);
        assert_eq!(ingestion.trace().events[1].kind, EventKind::Action);
        assert_eq!(ingestion.trace().events[1].caused_by, Some(0));
        assert!(ingestion.trace().events[1]
            .visible
            .contains(&"service.name".into()));
        assert_eq!(
            ingestion.trace().events[1].payload["events"][0]["name"],
            "tool.input"
        );
        validate(ingestion.trace()).unwrap();
    }

    #[test]
    fn preserves_spans_but_blocks_compilation_when_semantics_are_inferred_or_missing() {
        let text = serde_json::json!({
            "extraRoot": true,
            "resourceSpans": [{
                "scopeSpans": [{
                    "spans": [
                        {"traceId": "a", "spanId": "one", "name": "tool.call"},
                        {"traceId": "b", "spanId": "two", "parentSpanId": "missing", "name": "result", "startTimeUnixNano": 1,
                         "attributes": [
                            {"key": "x", "value": {"stringValue": "first"}},
                            {"key": "x", "value": {"stringValue": "second"}},
                            {"key": "bad", "value": {"unknownValue": true}}
                         ]}
                    ]
                }]
            }]
        })
        .to_string();
        let ingestion = from_otlp_json("trajectory", &text, false, 100).unwrap();
        assert_eq!(ingestion.trace().len(), 2);
        assert!(!ingestion.loss().is_lossless());
        assert!(!ingestion.is_compilable());
        assert_eq!(ingestion.loss().dropped_spans.len(), 0);
        assert_eq!(ingestion.loss().inferred_kinds.len(), 2);
        assert_eq!(ingestion.loss().multiple_trace_ids.len(), 1);
        assert_eq!(ingestion.loss().unresolved_parents.len(), 1);
        assert_eq!(ingestion.loss().duplicate_attributes.len(), 1);
        assert!(!ingestion.loss().unmapped_fields.is_empty());
    }

    #[test]
    fn accepts_the_legacy_instrumentation_library_spans_shape() {
        let text = serde_json::json!({
            "resourceSpans": [{
                "instrumentationLibrarySpans": [{
                    "spans": [{
                        "traceId": "legacy-trace",
                        "spanId": "legacy-span",
                        "name": "legacy",
                        "startTimeUnixNano": "1",
                        "attributes": [{"key": "prism.event.kind", "value": {"stringValue": "observation"}}]
                    }]
                }]
            }]
        })
        .to_string();
        let ingestion = from_otlp_json("legacy", &text, false, 100).unwrap();
        assert_eq!(ingestion.mapping().scope_count, 1);
        assert_eq!(ingestion.mapping().accepted_span_count, 1);
        assert!(ingestion.loss().is_lossless());
    }

    #[test]
    fn rejects_malformed_roots_and_span_limits_before_unbounded_work() {
        assert_eq!(
            from_otlp_json("id", "[]", false, 100).unwrap_err(),
            OtelError::InvalidRoot
        );
        let text = document(serde_json::json!([
            {
                "traceId": "trace",
                "spanId": "span",
                "name": "x"
            },
            {
                "traceId": "trace",
                "spanId": "span-2",
                "name": "y"
            }
        ]));
        assert_eq!(
            from_otlp_json("id", &text, false, 0).unwrap_err(),
            OtelError::InvalidLimit
        );
        assert_eq!(
            from_otlp_json("id", &text, false, 1).unwrap_err(),
            OtelError::TooManySpans { maximum: 1 }
        );
    }
}
