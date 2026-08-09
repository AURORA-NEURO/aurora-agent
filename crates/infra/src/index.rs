//! Indexes as rebuildable projections that answer with candidates, never with evidence.
//!
//! Blueprint 12.08's purpose is one sentence and it is the whole design: "support responsive
//! discovery and efficient execution **without allowing derived systems to become canonical
//! truth**". 12.07 adds two obligations: indexes "can be recreated from immutable artifacts and
//! relational metadata", and "every query result reports index revision/freshness". 12.01 puts
//! it plainly — graph and search systems are "rebuildable projections, not canonical truth".
//!
//! # How that is made structural
//!
//! [`IndexAnswer`] contains [`CandidateId`]s and nothing else. There is no method that returns a
//! value, a fact, a payload or a score, and none should be added: the moment an index can hand
//! back content, a caller can answer a question from it, and the projection has quietly become a
//! second source of truth that no invalidation covers. What an answer is *for* is deciding where
//! to look — the canonical read then happens through `bioprism-store`, whose sorted index is the
//! measured path that made point lookups independent of corpus size.
//!
//! Every answer also carries a [`Freshness`], and the third variant is the important one.
//! [`Freshness::Unknown`] is what a projection reports when the caller could not say how current
//! the canonical data is. An implementation that defaulted to `UpToDate` in that case would be
//! asserting currency from ignorance, which is the same mistake
//! [`crate::invalidation::Completeness`] exists to prevent one layer down.
//!
//! # Deliberately not implemented
//!
//! No ranking, no scoring, no tokenization, no stemming, no vectors, no approximate nearest
//! neighbours, no graph traversal — `bioprism-graph` owns traversal and 12.07's embedding section
//! needs a model this crate has no access to. No tenant filtering, which 12.07 requires to happen
//! *before* ranking; its absence is a gap, not a judgement that it is unnecessary. The posting
//! lists are a `BTreeMap` in memory and are rebuilt wholesale; there is no incremental
//! maintenance and no outbox consumer.

use crate::epoch::Epoch;
use crate::error::IndexError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A pointer to somewhere the canonical answer can be read.
///
/// Deliberately opaque. It is a place to look, not a thing that was found.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CandidateId(String);

impl CandidateId {
    pub fn parse(value: impl Into<String>) -> Result<Self, IndexError> {
        let value = value.into();
        if value.trim().is_empty() || value.chars().any(char::is_control) {
            return Err(IndexError::MalformedField {
                field: "candidate id",
                value,
            });
        }
        Ok(CandidateId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CandidateId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CandidateId> for String {
    fn from(value: CandidateId) -> Self {
        value.0
    }
}

impl TryFrom<String> for CandidateId {
    type Error = IndexError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        CandidateId::parse(value)
    }
}

/// How current a projection was when it answered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Freshness {
    /// The projection covers everything the canonical store holds.
    UpToDate { through: Epoch },
    /// The projection lags the canonical store by this many epochs.
    StaleBy { epochs: u64, through: Epoch },
    /// The caller did not say how current the canonical store is, so no claim can be made.
    ///
    /// Distinct from `StaleBy { epochs: 0 }`, which is a measurement. This is the absence of one.
    Unknown { reason: String },
}

impl Freshness {
    pub fn is_up_to_date(&self) -> bool {
        matches!(self, Freshness::UpToDate { .. })
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Freshness::Unknown { .. })
    }

    pub fn name(&self) -> &'static str {
        match self {
            Freshness::UpToDate { .. } => "up-to-date",
            Freshness::StaleBy { .. } => "stale",
            Freshness::Unknown { .. } => "unknown",
        }
    }
}

/// Candidates, plus the revision and freshness of the projection that produced them.
///
/// 12.07: "every query result reports index revision/freshness". Reported on the answer rather
/// than fetched separately, so a caller cannot log the candidates and forget the caveat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexAnswer {
    pub index: String,
    pub revision: u64,
    pub freshness: Freshness,
    candidates: BTreeSet<CandidateId>,
}

impl IndexAnswer {
    /// Where to look. There is no accessor returning content, by design.
    pub fn candidates(&self) -> &BTreeSet<CandidateId> {
        &self.candidates
    }

    pub fn len(&self) -> usize {
        self.candidates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// A rebuildable posting list over terms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    name: String,
    revision: u64,
    built_through: Epoch,
    postings: BTreeMap<String, BTreeSet<CandidateId>>,
}

impl Projection {
    /// An empty projection at revision 0, covering nothing.
    pub fn new(name: impl Into<String>) -> Result<Self, IndexError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(IndexError::MalformedField {
                field: "index name",
                value: name,
            });
        }
        Ok(Projection {
            name,
            revision: 0,
            built_through: Epoch::ZERO,
            postings: BTreeMap::new(),
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn built_through(&self) -> Epoch {
        self.built_through
    }

    /// Replaces the posting lists wholesale and bumps the revision.
    ///
    /// Wholesale because 12.07 requires the projection be reconstructible from canonical data,
    /// and a rebuild that merged into existing postings would preserve entries no longer present
    /// in the source — the classic way an index outlives the deletion it was supposed to reflect.
    ///
    /// Refuses a rebuild covering an earlier epoch than the current one: the revision would go
    /// forwards while the coverage went backwards, and every later freshness claim would be
    /// computed from the wrong bound.
    pub fn rebuild(
        &mut self,
        postings: impl IntoIterator<Item = (String, BTreeSet<CandidateId>)>,
        through: Epoch,
    ) -> Result<u64, IndexError> {
        if through < self.built_through {
            return Err(IndexError::RebuildGoesBackwards {
                index: self.name.clone(),
                through,
                existing: self.built_through,
            });
        }
        self.postings = postings.into_iter().collect();
        self.built_through = through;
        self.revision += 1;
        Ok(self.revision)
    }

    /// Answers a term lookup.
    ///
    /// `canonical_through` is what the caller knows about the canonical store's own coverage.
    /// `None` means the caller does not know, and produces [`Freshness::Unknown`] rather than an
    /// optimistic default.
    pub fn query(&self, term: &str, canonical_through: Option<Epoch>) -> IndexAnswer {
        let freshness = match canonical_through {
            None => Freshness::Unknown {
                reason: "caller did not state the canonical coverage epoch".to_string(),
            },
            Some(canonical) => match canonical.elapsed_since(self.built_through) {
                None | Some(0) => Freshness::UpToDate {
                    through: self.built_through,
                },
                Some(epochs) => Freshness::StaleBy {
                    epochs,
                    through: self.built_through,
                },
            },
        };
        IndexAnswer {
            index: self.name.clone(),
            revision: self.revision,
            freshness,
            candidates: self.postings.get(term).cloned().unwrap_or_default(),
        }
    }

    pub fn terms(&self) -> BTreeSet<&str> {
        self.postings.keys().map(String::as_str).collect()
    }
}
