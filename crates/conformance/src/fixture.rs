//! Golden fixtures: loading, digest verification and drift detection.
//!
//! Implements blueprint 40.33. Its execution path ends in "freeze hashes", and the reason is the
//! quiet failure that follows when you do not: a fixture is edited to make a red test go green,
//! the test now asserts against the edited input, and the suite keeps reporting success while
//! having stopped constraining anything. Nothing in a green CI run reveals this. So a fixture
//! whose digest no longer matches its card is an *error*, not a failing case — see
//! [`crate::ConformanceError`] for why that distinction is deliberate.
//!
//! ## Digests are taken over canonical values, not file bytes
//!
//! A fixture's declared `sha256` is [`bioprism_ids::ContentHash`] of the parsed document, i.e.
//! the same canonical encoding certificates are hashed under. Reindenting a fixture, or changing
//! its key order, is therefore not drift — which is correct, because neither changes what any
//! expectation observes. Adding a fact is drift. This costs one JSON parse per fixture and buys
//! a signal with no false positives from formatting churn.
//!
//! ## Cards carry a shape, so drift can say what moved
//!
//! A digest mismatch on a 332 KB world tells a reviewer that *something* changed. That is not
//! enough to act on, and 40.33 asks for the failure to be legible. Each card therefore also
//! declares a [`FixtureShape`] — top-level keys, canonical byte length, and the size of each
//! top-level collection — which is cheap to store and lets the drift report say "facts: 761 →
//! 760" instead of two hex strings. It is a summary, not a diff: it will tell you a fact was
//! removed, not which one.
//!
//! Not implemented here: signature verification, license scanning and the PII canary scan that
//! 40.33's verification plan also asks for. Provenance and license are recorded on the card as
//! declarations, and nothing in this crate checks that they are true.

use crate::error::ConformanceError;
use bioprism_ids::{to_canonical_bytes, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

pub const FIXTURE_MANIFEST_SCHEMA_VERSION: &str = "bioprism-fixture-manifest/0.1";

/// What a fixture is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureRole {
    /// An input world document.
    World,
    /// An input query document.
    Query,
    /// A frozen expected output, compared byte-for-byte.
    GoldenOutput,
}

impl FixtureRole {
    pub fn as_str(self) -> &'static str {
        match self {
            FixtureRole::World => "world",
            FixtureRole::Query => "query",
            FixtureRole::GoldenOutput => "golden_output",
        }
    }
}

/// A cheap structural summary of a fixture document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureShape {
    pub top_level_keys: Vec<String>,
    pub canonical_bytes: usize,
    /// Length of every top-level value that is an array or object.
    pub collection_sizes: BTreeMap<String, usize>,
}

impl FixtureShape {
    pub fn of(value: &Value) -> Result<Self, ConformanceError> {
        let canonical_bytes = to_canonical_bytes(value)
            .map_err(|e| ConformanceError::Canonical(e.to_string()))?
            .len();
        let (top_level_keys, collection_sizes) = match value.as_object() {
            Some(map) => {
                let keys = map.keys().cloned().collect();
                let sizes = map
                    .iter()
                    .filter_map(|(key, child)| match child {
                        Value::Array(items) => Some((key.clone(), items.len())),
                        Value::Object(fields) => Some((key.clone(), fields.len())),
                        _ => None,
                    })
                    .collect();
                (keys, sizes)
            }
            None => (Vec::new(), BTreeMap::new()),
        };
        Ok(FixtureShape {
            top_level_keys,
            canonical_bytes,
            collection_sizes,
        })
    }

    /// What moved between the declared shape (`self`) and an observed one.
    pub fn changes_to(&self, observed: &FixtureShape) -> Vec<ShapeChange> {
        let mut changes = Vec::new();
        for key in &observed.top_level_keys {
            if !self.top_level_keys.contains(key) {
                changes.push(ShapeChange::KeyAdded(key.clone()));
            }
        }
        for key in &self.top_level_keys {
            if !observed.top_level_keys.contains(key) {
                changes.push(ShapeChange::KeyRemoved(key.clone()));
            }
        }
        for (key, declared) in &self.collection_sizes {
            if let Some(actual) = observed.collection_sizes.get(key) {
                if actual != declared {
                    changes.push(ShapeChange::CollectionResized {
                        key: key.clone(),
                        declared: *declared,
                        observed: *actual,
                    });
                }
            }
        }
        if self.canonical_bytes != observed.canonical_bytes {
            changes.push(ShapeChange::SizeChanged {
                declared: self.canonical_bytes,
                observed: observed.canonical_bytes,
            });
        }
        changes
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ShapeChange {
    KeyAdded(String),
    KeyRemoved(String),
    CollectionResized {
        key: String,
        declared: usize,
        observed: usize,
    },
    SizeChanged {
        declared: usize,
        observed: usize,
    },
}

impl fmt::Display for ShapeChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeChange::KeyAdded(key) => write!(f, "top-level key {key:?} added"),
            ShapeChange::KeyRemoved(key) => write!(f, "top-level key {key:?} removed"),
            ShapeChange::CollectionResized {
                key,
                declared,
                observed,
            } => write!(f, "{key}: {declared} -> {observed}"),
            ShapeChange::SizeChanged { declared, observed } => {
                write!(f, "canonical bytes: {declared} -> {observed}")
            }
        }
    }
}

/// The fixture card of 40.33: identity, provenance, license and checksum.
///
/// `synthetic` is not decoration. 40.33 invariant 2 — "synthetic does not imply realistic" — is
/// the one that gets forgotten first, because a synthetic world that reproduces a real failure
/// mode starts to feel like evidence about the world. Carrying the flag into every report keeps
/// the qualification attached to the result rather than to the reviewer's memory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureCard {
    pub id: String,
    /// Path relative to the fixture root.
    pub path: String,
    pub role: FixtureRole,
    /// Canonical-value digest, frozen.
    pub sha256: String,
    pub provenance: String,
    pub license: String,
    pub synthetic: bool,
    pub shape: FixtureShape,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureManifest {
    pub schema_version: String,
    pub manifest_id: String,
    pub fixtures: Vec<FixtureCard>,
}

impl FixtureManifest {
    pub fn new(manifest_id: impl Into<String>, fixtures: Vec<FixtureCard>) -> Self {
        FixtureManifest {
            schema_version: FIXTURE_MANIFEST_SCHEMA_VERSION.to_string(),
            manifest_id: manifest_id.into(),
            fixtures,
        }
    }

    pub fn card(&self, id: &str) -> Option<&FixtureCard> {
        self.fixtures.iter().find(|card| card.id == id)
    }

    pub fn len(&self) -> usize {
        self.fixtures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.fixtures.is_empty()
    }
}

/// One fixture that no longer matches its card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixtureDrift {
    pub id: String,
    pub path: String,
    pub declared_sha256: String,
    pub observed_sha256: String,
    pub changes: Vec<ShapeChange>,
}

impl fmt::Display for FixtureDrift {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {} declares {} but hashes to {}",
            self.id, self.path, self.declared_sha256, self.observed_sha256
        )?;
        if self.changes.is_empty() {
            // Same shape, different bytes: a value was edited in place. Worth calling out,
            // because it is the drift a shape summary cannot localise at all.
            f.write_str("; structure unchanged, so a value was edited in place")
        } else {
            let rendered: Vec<String> = self.changes.iter().map(ShapeChange::to_string).collect();
            write!(f, "; {}", rendered.join(", "))
        }
    }
}

/// Fixtures loaded from disk and checked against their cards.
#[derive(Debug, Clone)]
pub struct FixtureStore {
    root: PathBuf,
    manifest_id: String,
    documents: BTreeMap<String, Value>,
    drift: Vec<FixtureDrift>,
    cards: BTreeMap<String, FixtureCard>,
}

impl FixtureStore {
    /// Reads every fixture in the manifest and records any that drifted.
    ///
    /// Unreadable or unparseable files are errors — there is nothing to compare. Digest
    /// mismatches are collected rather than raised, so that a caller can present *all* of them at
    /// once; [`FixtureStore::verify`] is the strict form used by the release gate.
    pub fn load(
        root: impl Into<PathBuf>,
        manifest: &FixtureManifest,
    ) -> Result<Self, ConformanceError> {
        let root = root.into();
        let mut documents = BTreeMap::new();
        let mut cards = BTreeMap::new();
        let mut drift = Vec::new();

        for card in &manifest.fixtures {
            let path = resolve(&root, &card.path);
            let text = std::fs::read_to_string(&path).map_err(|e| {
                ConformanceError::FixtureUnreadable {
                    id: card.id.clone(),
                    path: path.display().to_string(),
                    message: e.to_string(),
                }
            })?;
            let value: Value =
                serde_json::from_str(&text).map_err(|e| ConformanceError::FixtureNotJson {
                    id: card.id.clone(),
                    path: path.display().to_string(),
                    message: e.to_string(),
                })?;

            let observed_sha256 = ContentHash::of_value(&value)
                .map_err(|e| ConformanceError::Canonical(e.to_string()))?
                .as_str()
                .to_string();
            if observed_sha256 != card.sha256 {
                let observed_shape = FixtureShape::of(&value)?;
                drift.push(FixtureDrift {
                    id: card.id.clone(),
                    path: card.path.clone(),
                    declared_sha256: card.sha256.clone(),
                    observed_sha256,
                    changes: card.shape.changes_to(&observed_shape),
                });
            }

            documents.insert(card.id.clone(), value);
            cards.insert(card.id.clone(), card.clone());
        }

        Ok(FixtureStore {
            root,
            manifest_id: manifest.manifest_id.clone(),
            documents,
            drift,
            cards,
        })
    }

    /// Fails loudly on the first drifted fixture, naming what changed.
    pub fn verify(&self) -> Result<(), ConformanceError> {
        match self.drift.first() {
            None => Ok(()),
            Some(drift) => Err(ConformanceError::FixtureDrifted {
                id: drift.id.clone(),
                detail: drift.to_string(),
            }),
        }
    }

    pub fn drift(&self) -> &[FixtureDrift] {
        &self.drift
    }

    pub fn get(&self, id: &str) -> Result<&Value, ConformanceError> {
        self.documents
            .get(id)
            .ok_or_else(|| ConformanceError::UnknownFixture { id: id.to_string() })
    }

    pub fn card(&self, id: &str) -> Option<&FixtureCard> {
        self.cards.get(id)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest_id(&self) -> &str {
        &self.manifest_id
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Cards for fixtures declared synthetic, so a report can restate the qualification of
    /// 40.33 invariant 2 wherever the result is read.
    pub fn synthetic_ids(&self) -> Vec<&str> {
        self.cards
            .values()
            .filter(|card| card.synthetic)
            .map(|card| card.id.as_str())
            .collect()
    }
}

fn resolve(root: &Path, relative: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in relative.split('/') {
        path.push(segment);
    }
    path
}
