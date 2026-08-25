//! Defensive field access over the input `Value`.
//!
//! Figures read only the fields they render, so a document may carry keys this crate never looks
//! at — but a field a figure *does* render must be present with the right type, and its absence
//! must surface as an error naming the dotted path. Two accessors are deliberately `Option`-shaped
//! rather than erroring: the artifact writers in this workspace omit oracle-derived keys on
//! refused rows because absence is semantic there, and an accessor that demanded them would refuse
//! exactly the honest documents.

use crate::error::FigureError;
use serde_json::Value;

pub(crate) fn path(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}.{name}")
    }
}

pub(crate) fn require<'a>(
    value: &'a Value,
    parent: &str,
    name: &str,
) -> Result<&'a Value, FigureError> {
    let object = value.as_object().ok_or_else(|| FigureError::WrongType {
        field: if parent.is_empty() {
            "input".to_string()
        } else {
            parent.to_string()
        },
        expected: "a JSON object",
    })?;
    object.get(name).ok_or_else(|| FigureError::MissingField {
        field: path(parent, name),
    })
}

pub(crate) fn str_field<'a>(
    value: &'a Value,
    parent: &str,
    name: &str,
) -> Result<&'a str, FigureError> {
    require(value, parent, name)?
        .as_str()
        .ok_or_else(|| FigureError::WrongType {
            field: path(parent, name),
            expected: "a string",
        })
}

pub(crate) fn count_field(value: &Value, parent: &str, name: &str) -> Result<u64, FigureError> {
    require(value, parent, name)?
        .as_u64()
        .ok_or_else(|| FigureError::WrongType {
            field: path(parent, name),
            expected: "a non-negative integer",
        })
}

pub(crate) fn f64_field(value: &Value, parent: &str, name: &str) -> Result<f64, FigureError> {
    require(value, parent, name)?
        .as_f64()
        .ok_or_else(|| FigureError::WrongType {
            field: path(parent, name),
            expected: "a number",
        })
}

pub(crate) fn bool_field(value: &Value, parent: &str, name: &str) -> Result<bool, FigureError> {
    require(value, parent, name)?
        .as_bool()
        .ok_or_else(|| FigureError::WrongType {
            field: path(parent, name),
            expected: "a boolean",
        })
}

pub(crate) fn array_field<'a>(
    value: &'a Value,
    parent: &str,
    name: &str,
) -> Result<&'a Vec<Value>, FigureError> {
    require(value, parent, name)?
        .as_array()
        .ok_or_else(|| FigureError::WrongType {
            field: path(parent, name),
            expected: "an array",
        })
}

/// The key must exist — its writer always emits it — but `null` is a legitimate value, distinct
/// from the key being dropped by a truncated or hand-edited document.
pub(crate) fn nullable_str_field<'a>(
    value: &'a Value,
    parent: &str,
    name: &str,
) -> Result<Option<&'a str>, FigureError> {
    match require(value, parent, name)? {
        Value::Null => Ok(None),
        Value::String(text) => Ok(Some(text)),
        _ => Err(FigureError::WrongType {
            field: path(parent, name),
            expected: "a string or null",
        }),
    }
}
