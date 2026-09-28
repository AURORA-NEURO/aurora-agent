//! Shared fixtures and grouped HTTP gateway contract tests.

use super::*;
use crate::http::HttpRequest;
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

fn request(method: &str, target: &str, body: Value) -> HttpRequest {
    HttpRequest {
        method: method.into(),
        target: target.into(),
        version: "HTTP/1.1".into(),
        headers: BTreeMap::from([("content-type".into(), "application/json".into())]),
        body: serde_json::to_vec(&body).unwrap(),
    }
}

fn test_state_path(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT_TEST_STATE: AtomicU64 = AtomicU64::new(1);
    let path = std::env::temp_dir().join(format!(
        "bioprism-api-{label}-{}-{}.json",
        std::process::id(),
        NEXT_TEST_STATE.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_file(&path);
    path
}

mod events;
mod missions;
mod operations;
mod registries;
mod transport;
mod workflows;
