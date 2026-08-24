//! Emitting `bioprism_domain::Predicate` back to its own wire form.
//!
//! The reader lives in `bioprism-domain` and is strict; the writer did not exist, because nothing
//! upstream ever needed to *produce* a predicate document — packs are authored by hand. A repair
//! plan is generated, so the writer has to exist somewhere. It lives here rather than in
//! `bioprism-domain` because this crate is the first caller with the need, and adding a
//! `Serialize` impl to a type whose canonical form is defined by a hand-written parser is a change
//! to a shared vocabulary that should be made by a crate that owns it.
//!
//! The pairing is checked rather than asserted: [`predicate_from_json`] is `bioprism-domain`'s own
//! reader, and the crate's tests round-trip every kind the generator can emit through it. If the
//! two ever disagree the round-trip fails, rather than a plan quietly carrying a predicate that
//! parses back as something else.
//!
//! One value is refused rather than encoded. `serde_json` turns a non-finite `f64` into `null`,
//! which would parse back as a missing `minimum` — a threshold silently becoming absent is exactly
//! the class of defect this workspace refuses, so a non-finite bound is
//! [`crate::RepairError::UnrepresentablePredicate`] instead.

use crate::RepairError;
use bioprism_domain::Predicate;
use serde_json::{Map, Value};

/// `bioprism-domain`'s strict reader, re-exported through this crate's error type so a caller
/// parsing a plan gets one error enum rather than two.
pub fn predicate_from_json(document: &Value) -> Result<Predicate, RepairError> {
    Predicate::from_json(document).map_err(RepairError::from)
}

/// The inverse of [`Predicate::from_json`], field for field.
pub fn predicate_to_json(predicate: &Predicate) -> Result<Value, RepairError> {
    let mut map = Map::new();
    match predicate {
        Predicate::Exists { variable } => {
            map.insert("kind".into(), "exists".into());
            map.insert("variable".into(), variable.as_str().into());
        }
        Predicate::Missing { variable } => {
            map.insert("kind".into(), "missing".into());
            map.insert("variable".into(), variable.as_str().into());
        }
        Predicate::Nonempty { variable } => {
            map.insert("kind".into(), "nonempty".into());
            map.insert("variable".into(), variable.as_str().into());
        }
        Predicate::Equals { variable, value } => {
            map.insert("kind".into(), "equals".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("value".into(), value.clone());
        }
        Predicate::NotEquals { variable, value } => {
            map.insert("kind".into(), "not_equals".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("value".into(), value.clone());
        }
        Predicate::Contains { variable, value } => {
            map.insert("kind".into(), "contains".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("value".into(), value.clone());
        }
        Predicate::NumberAtLeast { variable, minimum } => {
            map.insert("kind".into(), "number_at_least".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("minimum".into(), finite(*minimum, "minimum")?);
        }
        Predicate::NumberBelow { variable, maximum } => {
            map.insert("kind".into(), "number_below".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("maximum".into(), finite(*maximum, "maximum")?);
        }
        Predicate::StringBefore { variable, than } => {
            map.insert("kind".into(), "string_before".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("than".into(), than.as_str().into());
        }
        Predicate::StringAfter { variable, than } => {
            map.insert("kind".into(), "string_after".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("than".into(), than.as_str().into());
        }
        Predicate::HasKey { variable, key } => {
            map.insert("kind".into(), "has_key".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("key".into(), key.as_str().into());
        }
        Predicate::CountAtLeast { variable, minimum } => {
            map.insert("kind".into(), "count_at_least".into());
            map.insert("variable".into(), variable.as_str().into());
            map.insert("minimum".into(), Value::from(*minimum as u64));
        }
        Predicate::AllOf { predicates } => {
            map.insert("kind".into(), "all_of".into());
            map.insert("predicates".into(), limbs(predicates)?);
        }
        Predicate::AnyOf { predicates } => {
            map.insert("kind".into(), "any_of".into());
            map.insert("predicates".into(), limbs(predicates)?);
        }
        Predicate::Not { predicate } => {
            map.insert("kind".into(), "not".into());
            map.insert("predicate".into(), predicate_to_json(predicate)?);
        }
    }
    Ok(Value::Object(map))
}

fn limbs(predicates: &[Predicate]) -> Result<Value, RepairError> {
    if predicates.is_empty() {
        return Err(RepairError::UnrepresentablePredicate(
            "an all_of or any_of with no limbs has no truth value to declare, and the domain \
             reader refuses it"
                .into(),
        ));
    }
    Ok(Value::Array(
        predicates
            .iter()
            .map(predicate_to_json)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn finite(bound: f64, field: &str) -> Result<Value, RepairError> {
    Value::from(bound).as_f64().map(Value::from).ok_or_else(|| {
        RepairError::UnrepresentablePredicate(format!(
            "{field} {bound} is not finite; serde_json would encode it as null and it would parse \
             back as an absent threshold"
        ))
    })
}
