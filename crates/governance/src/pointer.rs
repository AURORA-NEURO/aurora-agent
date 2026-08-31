//! Dotted-path access into a JSON document.
//!
//! Not JSON Pointer. RFC 6901 escapes `~` and `/` and starts every path with a slash, which would
//! make the field paths in a [`crate::descriptor::SchemaDescriptor`] unreadable for the one thing
//! they are for: naming a field in a diff. The formats governed here use plain identifier keys, so
//! a dotted path is unambiguous over them, and a key containing a literal `.` is simply not
//! addressable — [`splits`] says so rather than guessing.

use serde_json::{Map, Value};

/// The segments of a dotted path.
pub(crate) fn splits(path: &str) -> Vec<&str> {
    path.split('.').collect()
}

pub(crate) fn get<'a>(document: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cursor = document;
    for segment in splits(path) {
        cursor = cursor.as_object()?.get(segment)?;
    }
    Some(cursor)
}

/// Inserts a value, creating intermediate objects. Returns the value that was displaced.
pub(crate) fn insert(
    document: &mut Value,
    path: &str,
    value: Value,
) -> Result<Option<Value>, String> {
    if path.is_empty() {
        return Err("a dotted path must contain a field name".into());
    }
    let segments = splits(path);
    let Some((last, parents)) = segments.split_last() else {
        return Err("a dotted path must contain a field name".into());
    };
    let mut cursor = document;
    for segment in parents {
        if !cursor.is_object() {
            return Err(format!("{segment:?} in {path:?} is not an object"));
        }
        let Some(object) = cursor.as_object_mut() else {
            return Err(format!("{segment:?} in {path:?} is not an object"));
        };
        cursor = object
            .entry((*segment).to_string())
            .or_insert_with(|| Value::Object(Map::new()));
    }
    let map = cursor
        .as_object_mut()
        .ok_or_else(|| format!("the parent of {path:?} is not an object"))?;
    Ok(map.insert((*last).to_string(), value))
}

/// Removes a value. Returns what was there, or `None` if the path was absent.
pub(crate) fn remove(document: &mut Value, path: &str) -> Option<Value> {
    if path.is_empty() {
        return None;
    }
    let segments = splits(path);
    let (last, parents) = segments.split_last()?;
    let mut cursor = document;
    for segment in parents {
        cursor = cursor.as_object_mut()?.get_mut(*segment)?;
    }
    cursor.as_object_mut()?.remove(*last)
}

pub(crate) fn contains(document: &Value, path: &str) -> bool {
    get(document, path).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_nested_path_reads_writes_and_removes_the_same_location() {
        let mut document = json!({"plan": {"backend": "eager"}});
        assert_eq!(get(&document, "plan.backend"), Some(&json!("eager")));
        let displaced = insert(&mut document, "plan.backend", json!("indexed")).expect("inserts");
        assert_eq!(displaced, Some(json!("eager")));
        assert_eq!(
            remove(&mut document, "plan.backend"),
            Some(json!("indexed"))
        );
        assert!(!contains(&document, "plan.backend"));
        assert!(contains(&document, "plan"));
    }

    #[test]
    fn inserting_through_a_scalar_is_an_error_rather_than_an_overwrite() {
        let mut document = json!({"plan": 7});
        let error = insert(&mut document, "plan.backend", json!("eager"))
            .expect_err("a scalar cannot acquire members");
        assert!(error.contains("plan"));
        assert_eq!(document, json!({"plan": 7}));
    }

    #[test]
    fn removing_an_absent_path_reports_absence_rather_than_failing() {
        let mut document = json!({"plan": {}});
        assert_eq!(remove(&mut document, "plan.backend"), None);
        assert_eq!(remove(&mut document, "missing.deeper"), None);
    }
}
