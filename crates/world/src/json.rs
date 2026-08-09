//! Typed extraction helpers.
//!
//! Parsing is explicit rather than derived so that every rejection names the field and the
//! subject. Blueprint 40.36 requires typed failures, and a `serde` "invalid type" message that
//! does not say *which fact* was malformed is not actionable in a 761-fact world.

use crate::error::WorldError;
use serde_json::{Map, Value};

pub(crate) fn object<'a>(value: &'a Value, subject: &str) -> Result<&'a Map<String, Value>, WorldError> {
    value.as_object().ok_or_else(|| WorldError::WrongType {
        field: "<root>",
        subject: subject.to_string(),
        expected: "object",
    })
}

pub(crate) fn required<'a>(
    map: &'a Map<String, Value>,
    field: &'static str,
    subject: &str,
) -> Result<&'a Value, WorldError> {
    map.get(field).ok_or_else(|| WorldError::MissingField {
        field,
        subject: subject.to_string(),
    })
}

pub(crate) fn required_str(
    map: &Map<String, Value>,
    field: &'static str,
    subject: &str,
) -> Result<String, WorldError> {
    required(map, field, subject)?
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| WorldError::WrongType {
            field,
            subject: subject.to_string(),
            expected: "string",
        })
}

pub(crate) fn string_list(
    map: &Map<String, Value>,
    field: &'static str,
    subject: &str,
) -> Result<Vec<String>, WorldError> {
    match map.get(field) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(str::to_string).ok_or(WorldError::WrongType {
                    field,
                    subject: subject.to_string(),
                    expected: "array of strings",
                })
            })
            .collect(),
        Some(_) => Err(WorldError::WrongType {
            field,
            subject: subject.to_string(),
            expected: "array of strings",
        }),
    }
}

pub(crate) fn required_string_list(
    map: &Map<String, Value>,
    field: &'static str,
    subject: &str,
) -> Result<Vec<String>, WorldError> {
    required(map, field, subject)?;
    string_list(map, field, subject)
}

pub(crate) fn optional_f64(
    map: &Map<String, Value>,
    field: &'static str,
    subject: &str,
) -> Result<Option<f64>, WorldError> {
    match map.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_f64().map(Some).ok_or(WorldError::WrongType {
            field,
            subject: subject.to_string(),
            expected: "finite number",
        }),
        Some(_) => Err(WorldError::WrongType {
            field,
            subject: subject.to_string(),
            expected: "number",
        }),
    }
}
