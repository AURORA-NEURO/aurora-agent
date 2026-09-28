//! Canonical JSON serialization.
//!
//! The byte encoding produced here is defined to be identical to the Python reference
//! runtime's `json.dumps(obj, sort_keys=True, separators=(",", ":"), ensure_ascii=False)`
//! followed by `.encode("utf-8")`. Content hashes in Context Certificates are taken over
//! these bytes, so any divergence from Python silently breaks cross-language replay.
//!
//! The two places where a naive implementation diverges are float formatting (Rust's
//! `{}`/`{:e}` and CPython's `repr` disagree on when to use exponential notation and on
//! exponent padding) and non-finite numbers (CPython emits bare `NaN`/`Infinity`, which is
//! not JSON). Both are handled explicitly below.

use crate::error::CanonicalError;
use serde::ser::{
    self, Serialize, SerializeMap, SerializeSeq, SerializeStruct, SerializeStructVariant,
    SerializeTuple, SerializeTupleStruct, SerializeTupleVariant,
};
use serde_json::{Map, Value};
use std::fmt;

impl ser::Error for CanonicalError {
    fn custom<T: fmt::Display>(message: T) -> Self {
        CanonicalError::Serialization(message.to_string())
    }
}

/// Serialize Rust data into JSON values while preserving the distinction between non-finite
/// floats and explicit JSON nulls.
pub(crate) fn finite_json_value<T: ?Sized + Serialize>(value: &T) -> Result<Value, CanonicalError> {
    value.serialize(FiniteValueSerializer)
}

#[derive(Clone, Copy)]
struct FiniteValueSerializer;

struct SingleEntryMap<'a, T: ?Sized>(&'a T);

impl<T: ?Sized + Serialize> Serialize for SingleEntryMap<'_, T> {
    fn serialize<S: ser::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry(self.0, &())?;
        map.end()
    }
}

enum CompoundKind {
    Sequence,
    Object,
    TupleVariant(String),
    StructVariant(String),
}

struct FiniteCompound {
    kind: CompoundKind,
    sequence: Vec<Value>,
    object: Map<String, Value>,
    pending_key: Option<String>,
}

impl FiniteCompound {
    fn sequence(length: Option<usize>) -> Self {
        Self {
            kind: CompoundKind::Sequence,
            sequence: Vec::with_capacity(length.unwrap_or(0)),
            object: Map::new(),
            pending_key: None,
        }
    }

    fn object(length: Option<usize>) -> Self {
        Self {
            kind: CompoundKind::Object,
            sequence: Vec::new(),
            object: Map::with_capacity(length.unwrap_or(0)),
            pending_key: None,
        }
    }

    fn tuple_variant(variant: &str, length: usize) -> Self {
        Self {
            kind: CompoundKind::TupleVariant(variant.to_owned()),
            sequence: Vec::with_capacity(length),
            object: Map::new(),
            pending_key: None,
        }
    }

    fn struct_variant(variant: &str, length: usize) -> Self {
        Self {
            kind: CompoundKind::StructVariant(variant.to_owned()),
            sequence: Vec::new(),
            object: Map::with_capacity(length),
            pending_key: None,
        }
    }

    fn push<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), CanonicalError> {
        self.sequence.push(finite_json_value(value)?);
        Ok(())
    }

    fn insert<T: ?Sized + Serialize>(
        &mut self,
        key: &str,
        value: &T,
    ) -> Result<(), CanonicalError> {
        self.object
            .insert(key.to_owned(), finite_json_value(value)?);
        Ok(())
    }

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), CanonicalError> {
        if self.pending_key.is_some() {
            return Err(CanonicalError::Serialization(
                "map key was serialized before the previous value".into(),
            ));
        }
        let key = serde_json::to_value(SingleEntryMap(key))
            .map_err(|error| CanonicalError::Serialization(error.to_string()))?
            .as_object()
            .and_then(|object| object.keys().next().cloned())
            .ok_or_else(|| {
                CanonicalError::Serialization("JSON map key did not produce an object key".into())
            })?;
        self.pending_key = Some(key);
        Ok(())
    }

    fn serialize_map_value<T: ?Sized + Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), CanonicalError> {
        let key = self.pending_key.take().ok_or_else(|| {
            CanonicalError::Serialization("map value was serialized without a key".into())
        })?;
        self.object.insert(key, finite_json_value(value)?);
        Ok(())
    }

    fn finish(self) -> Result<Value, CanonicalError> {
        if self.pending_key.is_some() {
            return Err(CanonicalError::Serialization(
                "map ended with a key that had no value".into(),
            ));
        }
        match self.kind {
            CompoundKind::Sequence => Ok(Value::Array(self.sequence)),
            CompoundKind::Object => Ok(Value::Object(self.object)),
            CompoundKind::TupleVariant(variant) => Ok(Value::Object(Map::from_iter([(
                variant,
                Value::Array(self.sequence),
            )]))),
            CompoundKind::StructVariant(variant) => Ok(Value::Object(Map::from_iter([(
                variant,
                Value::Object(self.object),
            )]))),
        }
    }
}

impl ser::Serializer for FiniteValueSerializer {
    type Ok = Value;
    type Error = CanonicalError;
    type SerializeSeq = FiniteCompound;
    type SerializeTuple = FiniteCompound;
    type SerializeTupleStruct = FiniteCompound;
    type SerializeTupleVariant = FiniteCompound;
    type SerializeMap = FiniteCompound;
    type SerializeStruct = FiniteCompound;
    type SerializeStructVariant = FiniteCompound;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(value))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        serde_json::to_value(value)
            .map_err(|error| CanonicalError::Serialization(error.to_string()))
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Number(value.into()))
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        serde_json::to_value(value)
            .map_err(|error| CanonicalError::Serialization(error.to_string()))
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(CanonicalError::NonFiniteNumber(value.to_string()));
        }
        serde_json::to_value(value)
            .map_err(|error| CanonicalError::Serialization(error.to_string()))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        if !value.is_finite() {
            return Err(CanonicalError::NonFiniteNumber(value.to_string()));
        }
        serde_json::to_value(value)
            .map_err(|error| CanonicalError::Serialization(error.to_string()))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(value.to_owned()))
    }

    fn serialize_bytes(self, value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Array(
            value.iter().copied().map(Value::from).collect(),
        ))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        finite_json_value(value)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Null)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(variant.to_owned()))
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        finite_json_value(value)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Object(Map::from_iter([(
            variant.to_owned(),
            finite_json_value(value)?,
        )])))
    }

    fn serialize_seq(self, length: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(FiniteCompound::sequence(length))
    }

    fn serialize_tuple(self, length: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Ok(FiniteCompound::sequence(Some(length)))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Ok(FiniteCompound::sequence(Some(length)))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Ok(FiniteCompound::tuple_variant(variant, length))
    }

    fn serialize_map(self, length: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Ok(FiniteCompound::object(length))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(FiniteCompound::object(Some(length)))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
        length: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Ok(FiniteCompound::struct_variant(variant, length))
    }
}

impl SerializeSeq for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeTuple for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeTupleStruct for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeTupleVariant for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.push(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeMap for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Self::Error> {
        FiniteCompound::serialize_key(self, key)
    }

    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
        self.serialize_map_value(value)
    }

    fn serialize_entry<K: ?Sized + Serialize, V: ?Sized + Serialize>(
        &mut self,
        key: &K,
        value: &V,
    ) -> Result<(), Self::Error> {
        FiniteCompound::serialize_key(self, key)?;
        self.serialize_map_value(value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeStruct for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.insert(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

impl SerializeStructVariant for FiniteCompound {
    type Ok = Value;
    type Error = CanonicalError;

    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Self::Error> {
        self.insert(key, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        self.finish()
    }
}

pub fn to_canonical_string(value: &Value) -> Result<String, CanonicalError> {
    let mut out = String::new();
    write_value(value, &mut out)?;
    Ok(out)
}

pub fn to_canonical_bytes(value: &Value) -> Result<Vec<u8>, CanonicalError> {
    to_canonical_string(value).map(String::into_bytes)
}

/// Canonicalize typed Rust data without allowing non-finite floats to become JSON nulls first.
///
/// Use this when canonical bytes must be derived from a typed value, such as a signature or a
/// receipt. Once data has been converted to `serde_json::Value`, an explicit null cannot be
/// distinguished from a non-finite float that an earlier serializer replaced with null.
pub fn to_canonical_string_serializable<T: ?Sized + Serialize>(
    value: &T,
) -> Result<String, CanonicalError> {
    to_canonical_string(&finite_json_value(value)?)
}

/// Return canonical UTF-8 bytes for typed Rust data, rejecting non-finite floats before conversion.
pub fn to_canonical_bytes_serializable<T: ?Sized + Serialize>(
    value: &T,
) -> Result<Vec<u8>, CanonicalError> {
    to_canonical_string_serializable(value).map(String::into_bytes)
}

fn write_value(value: &Value, out: &mut String) -> Result<(), CanonicalError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => write_number(n, out)?,
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort_unstable();
            out.push('{');
            for (i, key) in keys.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(&map[*key], out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

fn write_number(n: &serde_json::Number, out: &mut String) -> Result<(), CanonicalError> {
    if let Some(u) = n.as_u64() {
        out.push_str(&u.to_string());
        return Ok(());
    }
    if let Some(i) = n.as_i64() {
        out.push_str(&i.to_string());
        return Ok(());
    }
    match n.as_f64() {
        Some(f) if f.is_finite() => {
            out.push_str(&python_repr_f64(f));
            Ok(())
        }
        Some(f) => Err(CanonicalError::NonFiniteNumber(f.to_string())),
        None => Err(CanonicalError::UnrepresentableNumber(n.to_string())),
    }
}

/// Formats a finite `f64` exactly as CPython's `repr` would.
///
/// CPython decides between fixed and exponential notation on `decpt`, the position of the
/// decimal point relative to the shortest round-trip digit string: exponential is used when
/// `decpt <= -4 || decpt > 16`. Exponents are always signed and zero-padded to two digits,
/// and a fixed-notation value with no fractional part gains a trailing `.0`.
pub fn python_repr_f64(f: f64) -> String {
    debug_assert!(f.is_finite());

    if f == 0.0 {
        return if f.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }

    let negative = f < 0.0;
    let exp_form = format!("{:e}", f.abs());
    let (mantissa, exponent) = exp_form
        .split_once('e')
        .expect("Rust LowerExp always emits an exponent");
    let exponent: i32 = exponent
        .parse()
        .expect("Rust LowerExp emits a valid exponent");

    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let digits = digits.trim_end_matches('0');
    let digits = if digits.is_empty() { "0" } else { digits };

    let decpt = exponent + 1;
    let body = if decpt <= -4 || decpt > 16 {
        format_exponential(digits, exponent)
    } else {
        format_fixed(digits, decpt)
    };

    if negative {
        format!("-{body}")
    } else {
        body
    }
}

fn format_exponential(digits: &str, exponent: i32) -> String {
    let mut s = String::new();
    s.push_str(&digits[..1]);
    if digits.len() > 1 {
        s.push('.');
        s.push_str(&digits[1..]);
    }
    s.push('e');
    if exponent < 0 {
        s.push('-');
    } else {
        s.push('+');
    }
    let magnitude = exponent.unsigned_abs();
    if magnitude < 10 {
        s.push('0');
    }
    s.push_str(&magnitude.to_string());
    s
}

fn format_fixed(digits: &str, decpt: i32) -> String {
    let len = digits.len() as i32;
    if decpt <= 0 {
        let mut s = String::from("0.");
        for _ in 0..(-decpt) {
            s.push('0');
        }
        s.push_str(digits);
        s
    } else if decpt >= len {
        let mut s = String::from(digits);
        for _ in 0..(decpt - len) {
            s.push('0');
        }
        s.push_str(".0");
        s
    } else {
        let split = decpt as usize;
        format!("{}.{}", &digits[..split], &digits[split..])
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}
