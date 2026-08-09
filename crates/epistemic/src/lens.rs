//! Query lenses and indexed views: blueprint 43.49.
//!
//! ## What a lens law is, and why three of them
//!
//! An optic is a focus from a whole onto a part. For a simple lens `(get, put)` the laws are:
//!
//! | Law | Statement | What it rules out |
//! |---|---|---|
//! | get–put | `put(s, get(s)) = s` | a put that changes something the get did not see |
//! | put–get | `get(put(s, v)) = v` | a put that silently rejects or transforms what it was given |
//! | put–put | `put(put(s, v₁), v₂) = put(s, v₂)` | a put that accumulates state across writes |
//!
//! An optic satisfying all three is *very well behaved* and can back an editable projection.
//! Anything less is a read, or a request. 43.49 says so directly — "a non-lawful update optic
//! becomes a one-way request API" — and this module makes that a type-level outcome rather than
//! advice: [`QueryLens::put`] on a [`OpticKind::Getter`] is [`EpistemicError::NoLawfulPut`], not a
//! best-effort write.
//!
//! ## Which laws this implementation actually satisfies
//!
//! Measured, per optic kind, by [`check_laws`] against a sample corpus. The honest summary:
//!
//! | Kind | get–put | put–get | put–put |
//! |---|---|---|---|
//! | [`OpticKind::Lens`] — a total field or index path | holds | holds | holds |
//! | [`OpticKind::AffineTraversal`] — a path that may be absent | holds | **holds only when the focus exists**; on an absent focus a put has nowhere to write and the subsequent get returns nothing | holds |
//! | [`OpticKind::Traversal`] — `Each` | holds | holds | holds |
//! | [`OpticKind::Traversal`] with a `Where` filter | holds | **fails**, demonstrably: put a value that does not satisfy the predicate and the next get does not return it | holds |
//! | [`OpticKind::Getter`] — an aggregation such as `SumOf` | not applicable | not applicable | not applicable |
//!
//! The filtered-traversal put–get failure is not a bug to fix. It is a genuine property of
//! predicate-focused optics — the focus is defined by the data, so writing data moves the focus —
//! and it is in 43.47's counterexample corpus for that reason. What would be a bug is reporting
//! the optic as lawful anyway.
//!
//! ## Indices, and the one invariant that has real teeth
//!
//! 43.49's first non-negotiable invariant: "erasing an index cannot preserve a claim that depends
//! on it". [`IndexedLens`] tags a view with the scope parameters its values are only meaningful
//! under — a coordinate frame, a genome build, an assay and unit — and [`TransformRegistry`]
//! refuses to compose two views whose indices differ on a dimension with no registered transform.
//! That is the module's worked micro-example made executable: the UI cannot drag a viewport
//! coordinate onto a pathology region and silently create an identity.
//!
//! ## What is not implemented
//!
//! Dependent types as such. Indices are runtime-validated tags, which is what 43.49 itself
//! recommends for the public SDK — "generated tagged types and validators rather than requiring
//! users to write dependent type theory". Also absent: profunctor optics, prisms, and any
//! composition other than a sequential focus chain.

use crate::error::EpistemicError;
use crate::theorem::{Applicability, Guarantee};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// One step of a focus chain.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "step")]
pub enum Focus {
    /// Descend into an object key.
    Field { name: String },
    /// Descend into an array position.
    Index { at: usize },
    /// Descend into every element of an array. Makes the optic a traversal.
    Each,
    /// Descend into array elements whose `field` equals `value`. Makes the optic a *filtered*
    /// traversal, and takes put–get with it.
    Where { field: String, value: Value },
    /// Terminal aggregation: sum a numeric field across the current foci. Read-only.
    SumOf { field: String },
}

/// What kind of optic a focus chain is, derived rather than declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpticKind {
    /// Exactly one focus on every document it is applied to.
    Lens,
    /// Zero or one focus. Cardinality is exposed rather than defaulted.
    AffineTraversal,
    /// Any number of foci.
    Traversal,
    /// Read-only. Has no put at all.
    Getter,
}

impl OpticKind {
    pub fn has_put(self) -> bool {
        self != OpticKind::Getter
    }

    pub fn as_str(self) -> &'static str {
        match self {
            OpticKind::Lens => "lens",
            OpticKind::AffineTraversal => "affine traversal",
            OpticKind::Traversal => "traversal",
            OpticKind::Getter => "getter",
        }
    }
}

/// A path segment in a resolved focus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "segment")]
pub enum Segment {
    Key { name: String },
    At { index: usize },
}

/// A composable focus from a document onto part of it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueryLens {
    pub name: String,
    pub steps: Vec<Focus>,
}

impl QueryLens {
    pub fn new(name: impl Into<String>, steps: Vec<Focus>) -> Self {
        QueryLens {
            name: name.into(),
            steps,
        }
    }

    /// A total field path. `["a", "b"]` focuses `s.a.b`.
    pub fn field_path(name: impl Into<String>, path: &[&str]) -> Self {
        QueryLens::new(
            name,
            path.iter()
                .map(|f| Focus::Field {
                    name: (*f).to_string(),
                })
                .collect(),
        )
    }

    /// The optic kind implied by the steps.
    pub fn kind(&self) -> OpticKind {
        if self
            .steps
            .iter()
            .any(|s| matches!(s, Focus::SumOf { .. }))
        {
            return OpticKind::Getter;
        }
        if self
            .steps
            .iter()
            .any(|s| matches!(s, Focus::Each | Focus::Where { .. }))
        {
            return OpticKind::Traversal;
        }
        OpticKind::AffineTraversal
    }

    /// Whether the focus is defined by the data it points at.
    ///
    /// The property that costs put–get. Separated out so a caller can screen for it without
    /// running the law check.
    pub fn is_predicate_focused(&self) -> bool {
        self.steps.iter().any(|s| matches!(s, Focus::Where { .. }))
    }

    /// Every concrete path this optic focuses in `document`.
    pub fn resolve(&self, document: &Value) -> Result<Vec<Vec<Segment>>, EpistemicError> {
        let mut paths: Vec<Vec<Segment>> = vec![Vec::new()];
        for step in &self.steps {
            let mut next: Vec<Vec<Segment>> = Vec::new();
            for path in &paths {
                let Some(here) = read(document, path) else {
                    continue;
                };
                match step {
                    Focus::Field { name } => {
                        if here.get(name).is_some() {
                            let mut extended = path.clone();
                            extended.push(Segment::Key { name: name.clone() });
                            next.push(extended);
                        }
                    }
                    Focus::Index { at } => {
                        if here.get(at).is_some() {
                            let mut extended = path.clone();
                            extended.push(Segment::At { index: *at });
                            next.push(extended);
                        }
                    }
                    Focus::Each => {
                        if let Some(array) = here.as_array() {
                            for index in 0..array.len() {
                                let mut extended = path.clone();
                                extended.push(Segment::At { index });
                                next.push(extended);
                            }
                        }
                    }
                    Focus::Where { field, value } => {
                        if let Some(array) = here.as_array() {
                            for (index, element) in array.iter().enumerate() {
                                if element.get(field) == Some(value) {
                                    let mut extended = path.clone();
                                    extended.push(Segment::At { index });
                                    next.push(extended);
                                }
                            }
                        }
                    }
                    Focus::SumOf { .. } => {
                        next.push(path.clone());
                    }
                }
            }
            paths = next;
        }
        Ok(paths)
    }

    /// Reads every focus.
    ///
    /// A [`OpticKind::Getter`] returns exactly one value: the aggregate. Everything else returns
    /// one value per focus, and the length *is* the cardinality 43.49 requires be exposed.
    pub fn get(&self, document: &Value) -> Result<Vec<Value>, EpistemicError> {
        if let Some(Focus::SumOf { field }) = self.steps.last() {
            let paths = self.resolve(document)?;
            let mut total = 0.0f64;
            for path in &paths {
                let Some(here) = read(document, path) else {
                    continue;
                };
                total += here.get(field).and_then(Value::as_f64).unwrap_or(0.0);
            }
            return Ok(vec![json!(total)]);
        }
        let paths = self.resolve(document)?;
        Ok(paths
            .iter()
            .filter_map(|path| read(document, path).cloned())
            .collect())
    }

    /// Writes one value per focus, returning a new document.
    ///
    /// Refuses a [`OpticKind::Getter`] outright and refuses a value count that does not match the
    /// focus count. 43.49: "typed delta or acquisition request, never arbitrary graph mutation".
    pub fn put(&self, document: &Value, values: &[Value]) -> Result<Value, EpistemicError> {
        if !self.kind().has_put() {
            return Err(EpistemicError::NoLawfulPut {
                lens: self.name.clone(),
                kind: self.kind().as_str(),
            });
        }
        let paths = self.resolve(document)?;
        if paths.len() != values.len() {
            return Err(EpistemicError::PutArity {
                lens: self.name.clone(),
                foci: paths.len(),
                values: values.len(),
            });
        }
        let mut updated = document.clone();
        for (path, value) in paths.iter().zip(values) {
            write(&mut updated, path, value.clone()).map_err(|detail| {
                EpistemicError::FocusFailed {
                    lens: self.name.clone(),
                    detail,
                }
            })?;
        }
        Ok(updated)
    }
}

fn read<'a>(document: &'a Value, path: &[Segment]) -> Option<&'a Value> {
    let mut here = document;
    for segment in path {
        here = match segment {
            Segment::Key { name } => here.get(name)?,
            Segment::At { index } => here.get(index)?,
        };
    }
    Some(here)
}

fn write(document: &mut Value, path: &[Segment], value: Value) -> Result<(), String> {
    let Some((last, prefix)) = path.split_last() else {
        *document = value;
        return Ok(());
    };
    let mut here = document;
    for segment in prefix {
        here = match segment {
            Segment::Key { name } => here
                .get_mut(name)
                .ok_or_else(|| format!("key {name:?} vanished between resolve and write"))?,
            Segment::At { index } => here
                .get_mut(index)
                .ok_or_else(|| format!("index {index} vanished between resolve and write"))?,
        };
    }
    match last {
        Segment::Key { name } => {
            let object = here
                .as_object_mut()
                .ok_or_else(|| "target of a field write is not an object".to_string())?;
            object.insert(name.clone(), value);
        }
        Segment::At { index } => {
            let array = here
                .as_array_mut()
                .ok_or_else(|| "target of an index write is not an array".to_string())?;
            let slot = array
                .get_mut(*index)
                .ok_or_else(|| format!("index {index} out of range at write time"))?;
            *slot = value;
        }
    }
    Ok(())
}

/// Whether one law held, failed, or does not apply.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum LawStatus {
    Holds,
    /// Failed, with the document and replacement that produced it. A counterexample a reader can
    /// re-run beats a count of failures.
    Fails {
        document: Value,
        replacement: Vec<Value>,
        detail: String,
    },
    NotApplicable {
        why: String,
    },
}

impl LawStatus {
    pub fn holds(&self) -> bool {
        matches!(self, LawStatus::Holds)
    }
}

/// Which laws an optic satisfied, on which corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LawReport {
    pub optic: String,
    pub kind: OpticKind,
    pub samples: usize,
    pub get_put: LawStatus,
    pub put_get: LawStatus,
    pub put_put: LawStatus,
}

impl LawReport {
    /// All three laws held. A `NotApplicable` is not a pass.
    pub fn lawful(&self) -> bool {
        self.get_put.holds() && self.put_get.holds() && self.put_put.holds()
    }

    /// The gated guarantee, so a caller cannot read lawfulness off a bare boolean.
    pub fn applicability(&self) -> Applicability {
        if self.samples == 0 {
            return Applicability::NotChecked {
                guarantee: Guarantee::LensLawful,
                missing_check: "the sample corpus was empty".to_string(),
            };
        }
        if self.lawful() {
            return Applicability::Applies {
                guarantee: Guarantee::LensLawful,
                factor: None,
                evidence: format!(
                    "{}: get-put, put-get and put-put held on {} documents",
                    self.optic, self.samples
                ),
            };
        }
        let mut failed = Vec::new();
        for (name, status) in [
            ("get-put", &self.get_put),
            ("put-get", &self.put_get),
            ("put-put", &self.put_put),
        ] {
            match status {
                LawStatus::Fails { detail, .. } => failed.push(format!("{name} failed: {detail}")),
                LawStatus::NotApplicable { why } => {
                    failed.push(format!("{name} not applicable: {why}"))
                }
                LawStatus::Holds => {}
            }
        }
        Applicability::DoesNotApply {
            guarantee: Guarantee::LensLawful,
            failed_precondition: failed.join("; "),
        }
    }
}

/// Runs the three laws over a corpus of `(document, replacement)` pairs.
///
/// The replacement must have one value per focus in that document; a pair whose arity does not
/// match is reported as a get–put/put–get failure rather than skipped, because skipping it would
/// let a corpus be tuned until every law passes.
pub fn check_laws(
    lens: &QueryLens,
    corpus: &[(Value, Vec<Value>)],
) -> Result<LawReport, EpistemicError> {
    let kind = lens.kind();
    if !kind.has_put() {
        let why = format!("{} is read-only and has no put", kind.as_str());
        return Ok(LawReport {
            optic: lens.name.clone(),
            kind,
            samples: corpus.len(),
            get_put: LawStatus::NotApplicable { why: why.clone() },
            put_get: LawStatus::NotApplicable { why: why.clone() },
            put_put: LawStatus::NotApplicable { why },
        });
    }

    let mut get_put = LawStatus::Holds;
    let mut put_get = LawStatus::Holds;
    let mut put_put = LawStatus::Holds;

    for (document, replacement) in corpus {
        let seen = lens.get(document)?;
        match lens.put(document, &seen) {
            Ok(rebuilt) if &rebuilt == document => {}
            Ok(rebuilt) => {
                if get_put.holds() {
                    get_put = LawStatus::Fails {
                        document: document.clone(),
                        replacement: seen.clone(),
                        detail: format!(
                            "putting back what get returned produced a different document: {rebuilt}"
                        ),
                    };
                }
            }
            Err(error) => {
                if get_put.holds() {
                    get_put = LawStatus::Fails {
                        document: document.clone(),
                        replacement: seen.clone(),
                        detail: error.to_string(),
                    };
                }
            }
        }

        match lens.put(document, replacement) {
            Ok(written) => {
                let back = lens.get(&written)?;
                if back != *replacement && put_get.holds() {
                    put_get = LawStatus::Fails {
                        document: document.clone(),
                        replacement: replacement.clone(),
                        detail: format!("get after put returned {back:?}, not what was put"),
                    };
                }
                match lens.put(&written, replacement) {
                    Ok(twice) if twice == written => {}
                    Ok(twice) => {
                        if put_put.holds() {
                            put_put = LawStatus::Fails {
                                document: document.clone(),
                                replacement: replacement.clone(),
                                detail: format!("a second identical put changed the document: {twice}"),
                            };
                        }
                    }
                    Err(error) => {
                        if put_put.holds() {
                            put_put = LawStatus::Fails {
                                document: document.clone(),
                                replacement: replacement.clone(),
                                detail: error.to_string(),
                            };
                        }
                    }
                }
            }
            Err(error) => {
                if put_get.holds() {
                    put_get = LawStatus::Fails {
                        document: document.clone(),
                        replacement: replacement.clone(),
                        detail: error.to_string(),
                    };
                }
            }
        }
    }

    Ok(LawReport {
        optic: lens.name.clone(),
        kind,
        samples: corpus.len(),
        get_put,
        put_get,
        put_put,
    })
}

/// A view tagged with the scope parameters its values are only meaningful under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexedLens {
    pub lens: QueryLens,
    /// Dimension to value: `{"coordinate_frame": "scan-42", "units": "voxel"}`.
    pub index: BTreeMap<String, String>,
}

impl IndexedLens {
    pub fn new(lens: QueryLens, index: BTreeMap<String, String>) -> Self {
        IndexedLens { lens, index }
    }

    /// Dimensions on which two indices disagree.
    pub fn conflicts_with(&self, other: &IndexedLens) -> Vec<String> {
        self.index
            .iter()
            .filter(|(dimension, value)| {
                other
                    .index
                    .get(*dimension)
                    .is_some_and(|theirs| theirs != *value)
            })
            .map(|(dimension, _)| dimension.clone())
            .collect()
    }
}

/// Transforms declared to exist between two values of one scope dimension.
///
/// Registration is explicit and directionless: declaring a transform is a claim that a mapping
/// exists and has been characterised, which is 43.49's "composed lens exists only through a
/// registered transform and uncertainty object". This registry records that the claim was made; it
/// does not carry the mapping, because a mapping between coordinate frames belongs to
/// `bioprism-standards` and duplicating it here would create a second one to keep in parity.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TransformRegistry {
    edges: BTreeSet<(String, String, String)>,
}

impl TransformRegistry {
    pub fn new() -> Self {
        TransformRegistry::default()
    }

    pub fn register(
        &mut self,
        dimension: impl Into<String>,
        left: impl Into<String>,
        right: impl Into<String>,
    ) {
        let (dimension, left, right) = (dimension.into(), left.into(), right.into());
        let (a, b) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.edges.insert((dimension, a, b));
    }

    pub fn has(&self, dimension: &str, left: &str, right: &str) -> bool {
        let (a, b) = if left <= right {
            (left, right)
        } else {
            (right, left)
        };
        self.edges
            .contains(&(dimension.to_string(), a.to_string(), b.to_string()))
    }

    /// Composes two indexed views, refusing when an index differs with no registered transform.
    ///
    /// The refusal is the invariant: "erasing an index cannot preserve a claim that depends on
    /// it". Composition succeeds only when the indices agree or a transform was declared, and the
    /// composed index is the union — so the result still says what it is indexed by.
    pub fn compose(
        &self,
        left: &IndexedLens,
        right: &IndexedLens,
    ) -> Result<IndexedLens, EpistemicError> {
        for dimension in left.conflicts_with(right) {
            let ours = &left.index[&dimension];
            let theirs = &right.index[&dimension];
            if !self.has(&dimension, ours, theirs) {
                return Err(EpistemicError::UnregisteredIndexTransform {
                    left: format!("{dimension}={ours}"),
                    right: format!("{dimension}={theirs}"),
                });
            }
        }
        let mut steps = left.lens.steps.clone();
        steps.extend(right.lens.steps.iter().cloned());
        let mut index = left.index.clone();
        for (dimension, value) in &right.index {
            index.entry(dimension.clone()).or_insert(value.clone());
        }
        Ok(IndexedLens {
            lens: QueryLens::new(
                format!("{}∘{}", left.lens.name, right.lens.name),
                steps,
            ),
            index,
        })
    }
}

/// What a view disclosed about where it came from.
///
/// 43.49: "every view discloses source and filter". A receipt with a source digest is the
/// difference between a view a reader can trace and a table with a title.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewReceipt {
    pub optic: String,
    pub kind: OpticKind,
    /// Number of foci. Cardinality, exposed.
    pub foci: usize,
    pub steps: Vec<Focus>,
    pub index: BTreeMap<String, String>,
    /// Content address of the document the view was taken from.
    pub source_digest: String,
}

/// Builds the receipt for one application of an indexed view.
pub fn view_receipt(
    view: &IndexedLens,
    document: &Value,
) -> Result<ViewReceipt, EpistemicError> {
    let digest = bioprism_ids::sha256_hex_of_value(document).map_err(|e| {
        EpistemicError::FocusFailed {
            lens: view.lens.name.clone(),
            detail: e.to_string(),
        }
    })?;
    Ok(ViewReceipt {
        optic: view.lens.name.clone(),
        kind: view.lens.kind(),
        foci: view.lens.get(document)?.len(),
        steps: view.lens.steps.clone(),
        index: view.index.clone(),
        source_digest: digest,
    })
}
