//! Atomic, provenance-rich evidence units.
//!
//! Implements blueprint 25.11. An evidence object is one addressable thing a claim can rest
//! on: a spreadsheet cell, a region of an image, a span of a report, a row of a database, one
//! reader's interpretation. It is deliberately smaller than a document, because "the paper
//! says" is not something a downstream evaluator can verify and "cell D14 of table 2 in the
//! artifact hashing to `9f2c…`" is.
//!
//! Three of the blueprint's invariants are enforced here.
//!
//! **"Derived evidence preserves ancestors."** [`EvidenceLedger::insert`] rejects a derivation
//! naming an ancestor the ledger does not hold. A dangling ancestor is worse than no
//! provenance at all: it looks like a chain of custody while pointing nowhere.
//!
//! **Access-label propagation.** Not stated as an invariant in 25.11 but listed under its
//! validation section, and 39.05 protects "privacy, consent/use restriction, data residency,
//! and role visibility" as non-compressible. Derivation is the classic laundering path — take
//! controlled evidence, compute a summary, publish the summary — so a derived object must
//! carry every label its ancestors carried. Labels may be *added*; never dropped.
//!
//! **"Support and contradiction are claims with provenance."** [`Relation`] carries who
//! asserted it and when. A contradiction with no asserter is a fact about the world; a
//! contradiction with an asserter is a fact about a disagreement, and only the second is true.
//!
//! # Not implemented
//!
//! "Native coordinates remain resolvable" cannot be fully checked here. Resolving a
//! [`Locator`] means opening the artifact, and no artifact-shape contract exists in the
//! blueprint to check a row index against. [`Locator::check`] verifies internal
//! well-formedness only — that a sequence range is non-empty and ordered, that a bounding box
//! has positive extent, that no coordinate is blank — and cannot tell you whether row 4,000
//! exists in a table with 12 rows.

use crate::error::EvidenceError;
use crate::ids::{EvidenceId, LensId, SpecimenId, SubjectId};
use bioprism_ids::ContentHash;
use bioprism_scope::{Interval, ScopeKey, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// The medium the evidence lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Tabular,
    Image,
    Sequence,
    Text,
    Code,
    Database,
    /// A human reading, which is evidence about an expert rather than about a specimen.
    ExpertInterpretation,
}

/// Where in the artifact the evidence is.
///
/// Native coordinates, not a re-indexed copy: 25.11 requires the original addressing scheme to
/// survive, because a re-indexed coordinate cannot be checked against the source and quietly
/// becomes wrong the first time a row is inserted upstream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "locator", rename_all = "snake_case")]
pub enum Locator {
    TableCell {
        table: String,
        row: String,
        column: String,
    },
    /// A half-open pixel box `[x0, x1) x [y0, y1)` in a named frame.
    ImageRegion {
        series: String,
        frame: String,
        x0: u64,
        y0: u64,
        x1: u64,
        y1: u64,
    },
    /// A half-open interval on a named sequence, with the reference build it is stated against.
    SequenceRange {
        sequence: String,
        reference_build: String,
        start: u64,
        end: u64,
    },
    NotebookCell {
        notebook: String,
        cell: String,
    },
    DatabaseRecord {
        database: String,
        table: String,
        primary_key: String,
    },
    /// A half-open character span of a document.
    DocumentSpan {
        document: String,
        start: u64,
        end: u64,
    },
}

impl Locator {
    /// Internal well-formedness. See the module note on what this cannot check.
    pub fn check(&self, evidence: &EvidenceId) -> Result<(), EvidenceError> {
        let refuse = |reason: &str| {
            Err(EvidenceError::UnresolvableLocator {
                evidence: evidence.to_string(),
                reason: reason.to_string(),
            })
        };
        match self {
            Locator::TableCell { table, row, column } => {
                if table.is_empty() || row.is_empty() || column.is_empty() {
                    return refuse("table, row and column must all be named");
                }
            }
            Locator::ImageRegion {
                series,
                frame,
                x0,
                y0,
                x1,
                y1,
            } => {
                if series.is_empty() || frame.is_empty() {
                    return refuse("series and frame must be named");
                }
                if x1 <= x0 || y1 <= y0 {
                    return refuse("image region has no area");
                }
            }
            Locator::SequenceRange {
                sequence,
                reference_build,
                start,
                end,
            } => {
                if sequence.is_empty() {
                    return refuse("sequence must be named");
                }
                if reference_build.is_empty() {
                    return refuse("a coordinate without a reference build is not resolvable");
                }
                if end <= start {
                    return refuse("sequence range is empty");
                }
            }
            Locator::NotebookCell { notebook, cell } => {
                if notebook.is_empty() || cell.is_empty() {
                    return refuse("notebook and cell must be named");
                }
            }
            Locator::DatabaseRecord {
                database,
                table,
                primary_key,
            } => {
                if database.is_empty() || table.is_empty() || primary_key.is_empty() {
                    return refuse("database, table and primary key must all be named");
                }
            }
            Locator::DocumentSpan {
                document,
                start,
                end,
            } => {
                if document.is_empty() {
                    return refuse("document must be named");
                }
                if end <= start {
                    return refuse("document span is empty");
                }
            }
        }
        Ok(())
    }
}

/// Who produced this evidence object from the artifact, and with what.
///
/// Adapter and parser versions are recorded because 25.11's security note says the resolver
/// records them, and because an extraction bug is a version, not a mood: without the version
/// you cannot tell which extracted rows to re-do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub adapter: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub extracted_at: Timestamp,
    /// The upstream source in its own terms: an accession, a DOI, a file path.
    pub source: String,
}

/// What this evidence is about.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeasurementContext {
    pub subject: Option<SubjectId>,
    pub specimen: Option<SpecimenId>,
    pub lens: Option<LensId>,
    pub observed_at: Option<Timestamp>,
    /// Where the evidence is valid, in the shared scope vocabulary.
    pub scope: ScopeKey,
}

/// A statement about how good the evidence is, and who says so.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAssertion {
    pub grade: String,
    pub asserted_by: String,
    pub caveats: BTreeSet<String>,
}

/// How this evidence was produced from other evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Derivation {
    pub ancestors: Vec<EvidenceId>,
    pub transform: String,
    pub transform_version: String,
}

/// Who may see this, and whether it may travel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessPolicy {
    pub labels: BTreeSet<String>,
    /// False when the bytes may only be referenced, never embedded in a package.
    ///
    /// 25.11's security note is explicit that controlled data may be referenced without being
    /// embedded, which is why the artifact hash and the [`Locator`] are separate from content.
    pub embeddable: bool,
}

impl AccessPolicy {
    pub fn labelled(labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        AccessPolicy {
            labels: labels.into_iter().map(Into::into).collect(),
            embeddable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObject {
    pub id: EvidenceId,
    /// Hash of the artifact the locator points into, not of this object.
    pub artifact_hash: ContentHash,
    pub locator: Locator,
    pub modality: Modality,
    pub content_type: String,
    /// Entity bindings by role: `{"gene": "HGNC:1097", "disease": "MONDO:0005070"}`.
    /// Terms are opaque here; 25.03 owns ontology binding and version pinning.
    pub bindings: BTreeMap<String, String>,
    pub context: MeasurementContext,
    pub quality: QualityAssertion,
    pub provenance: Provenance,
    /// When this evidence is true of the world, not when it was recorded.
    pub validity: Interval,
    pub access: AccessPolicy,
    pub derivation: Option<Derivation>,
}

impl EvidenceObject {
    /// Whether the evidence is outside its validity interval at `at`.
    ///
    /// Staleness is a property of the claim, not of the file: a tumour measurement from 2019 is
    /// a perfectly good record and a stale statement about today's tumour.
    pub fn is_stale_at(&self, at: Timestamp) -> bool {
        !self.validity.contains(at)
    }

    /// Confirms the artifact these coordinates point into is the one that was hashed.
    pub fn verify_artifact(&self, bytes: &[u8]) -> Result<(), EvidenceError> {
        let actual = ContentHash::of_bytes(bytes);
        if actual == self.artifact_hash {
            Ok(())
        } else {
            Err(EvidenceError::ArtifactHashMismatch {
                evidence: self.id.to_string(),
                declared: self.artifact_hash.to_string(),
                actual: actual.to_string(),
            })
        }
    }

    /// A content hash over the evidence object itself, for citation in a result bundle.
    pub fn content_hash(&self) -> Result<ContentHash, EvidenceError> {
        let value = serde_json::to_value(self).map_err(|error| EvidenceError::Canonical {
            evidence: self.id.to_string(),
            message: error.to_string(),
        })?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceError::canonical(self.id.as_str(), error))
    }
}

/// Whether one evidence object backs another or cuts against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stance {
    Supports,
    Contradicts,
}

/// An asserted relation between two evidence objects.
///
/// Support and contradiction are not derived from the evidence; somebody claims them. The
/// asserter and the instant are mandatory for that reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    pub subject: EvidenceId,
    pub object: EvidenceId,
    pub stance: Stance,
    pub asserted_by: String,
    pub asserted_at: Timestamp,
    pub rationale: String,
}

/// A set of evidence objects and the relations asserted between them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceLedger {
    objects: BTreeMap<EvidenceId, EvidenceObject>,
    relations: Vec<Relation>,
}

impl EvidenceLedger {
    pub fn new() -> Self {
        EvidenceLedger {
            objects: BTreeMap::new(),
            relations: Vec::new(),
        }
    }

    /// Admits an evidence object after checking everything that can be checked without the
    /// artifact bytes: locator well-formedness, validity, ancestry and access-label carriage.
    pub fn insert(&mut self, object: EvidenceObject) -> Result<(), EvidenceError> {
        if self.objects.contains_key(&object.id) {
            return Err(EvidenceError::DuplicateEvidence {
                evidence: object.id.to_string(),
            });
        }
        object.locator.check(&object.id)?;
        if object.validity.is_empty() {
            return Err(EvidenceError::EmptyValidityInterval {
                evidence: object.id.to_string(),
            });
        }
        if let Some(derivation) = &object.derivation {
            self.check_derivation(&object, derivation)?;
        }
        self.objects.insert(object.id.clone(), object);
        Ok(())
    }

    fn check_derivation(
        &self,
        object: &EvidenceObject,
        derivation: &Derivation,
    ) -> Result<(), EvidenceError> {
        for ancestor_id in &derivation.ancestors {
            if ancestor_id == &object.id {
                return Err(EvidenceError::SelfDerivation {
                    evidence: object.id.to_string(),
                });
            }
            let ancestor =
                self.objects
                    .get(ancestor_id)
                    .ok_or_else(|| EvidenceError::UnknownAncestor {
                        evidence: object.id.to_string(),
                        ancestor: ancestor_id.to_string(),
                    })?;
            for label in &ancestor.access.labels {
                if !object.access.labels.contains(label) {
                    return Err(EvidenceError::AccessLabelDropped {
                        evidence: object.id.to_string(),
                        ancestor: ancestor_id.to_string(),
                        label: label.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// Records an asserted relation between two objects already in the ledger.
    pub fn assert_relation(&mut self, relation: Relation) -> Result<(), EvidenceError> {
        if relation.asserted_by.trim().is_empty() {
            return Err(EvidenceError::UnattributedRelation {
                subject: relation.subject.to_string(),
                object: relation.object.to_string(),
            });
        }
        for id in [&relation.subject, &relation.object] {
            if !self.objects.contains_key(id) {
                return Err(EvidenceError::UnknownEvidence {
                    evidence: id.to_string(),
                });
            }
        }
        self.relations.push(relation);
        Ok(())
    }

    pub fn get(&self, id: &EvidenceId) -> Result<&EvidenceObject, EvidenceError> {
        self.objects
            .get(id)
            .ok_or_else(|| EvidenceError::UnknownEvidence {
                evidence: id.to_string(),
            })
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EvidenceObject> {
        self.objects.values()
    }

    /// Every relation naming `id` on either side.
    ///
    /// Both directions, because a contradiction asserted *against* a piece of evidence is
    /// exactly what a consumer of that evidence needs to see, and a subject-only lookup would
    /// hide it.
    pub fn relations_for(&self, id: &EvidenceId) -> Vec<&Relation> {
        self.relations
            .iter()
            .filter(|relation| &relation.subject == id || &relation.object == id)
            .collect()
    }

    pub fn contradictions(&self) -> Vec<&Relation> {
        self.relations
            .iter()
            .filter(|relation| relation.stance == Stance::Contradicts)
            .collect()
    }

    /// Objects whose validity interval does not cover `at`.
    pub fn stale_at(&self, at: Timestamp) -> Vec<&EvidenceObject> {
        self.objects
            .values()
            .filter(|object| object.is_stale_at(at))
            .collect()
    }

    /// Everything wrong with the ledger as a whole at decision time `at`.
    ///
    /// Both issue kinds are about *use*, not about structure — an object can be perfectly well
    /// formed and still be the wrong thing to reason from today.
    pub fn audit(&self, at: Timestamp) -> Vec<EvidenceIssue> {
        let mut issues = Vec::new();
        for object in self.objects.values() {
            if object.is_stale_at(at) {
                issues.push(EvidenceIssue::Stale {
                    evidence: object.id.clone(),
                    at,
                });
            }
        }
        for relation in self.contradictions() {
            issues.push(EvidenceIssue::UnadjudicatedContradiction {
                evidence: relation.object.clone(),
                contradicted_by: relation.subject.clone(),
            });
        }
        issues
    }

    /// Transitive ancestors of `id`, nearest first.
    pub fn ancestry(&self, id: &EvidenceId) -> Result<Vec<EvidenceId>, EvidenceError> {
        let mut found = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![id.clone()];
        while let Some(current) = frontier.pop() {
            let object = self.get(&current)?;
            let Some(derivation) = &object.derivation else {
                continue;
            };
            for ancestor in &derivation.ancestors {
                if seen.insert(ancestor.clone()) {
                    found.push(ancestor.clone());
                    frontier.push(ancestor.clone());
                }
            }
        }
        Ok(found)
    }

    /// The union of access labels this object inherits from its whole ancestry.
    ///
    /// A caller that wants to know what a derived summary is allowed to reveal asks this, not
    /// the object's own label set — although [`EvidenceLedger::insert`] guarantees they agree.
    pub fn effective_access_labels(
        &self,
        id: &EvidenceId,
    ) -> Result<BTreeSet<String>, EvidenceError> {
        let mut labels = self.get(id)?.access.labels.clone();
        for ancestor in self.ancestry(id)? {
            labels.extend(self.get(&ancestor)?.access.labels.iter().cloned());
        }
        Ok(labels)
    }
}

/// A diagnostic about an evidence set as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvidenceIssue {
    #[error("evidence {evidence} is contradicted by {contradicted_by} and no adjudication is recorded")]
    UnadjudicatedContradiction {
        evidence: EvidenceId,
        contradicted_by: EvidenceId,
    },

    #[error("evidence {evidence} is outside its validity interval at {at}")]
    Stale { evidence: EvidenceId, at: Timestamp },
}
