//! Binding local terminology to versioned external identifiers.
//!
//! Blueprint 28.19 is the source module, and its four named failure modes drive the design:
//! *false precision* (an ambiguous concept forced into one code), *historical erasure* (the
//! original diagnosis lost), *namespace collision*, *version drift*, and *granularity mismatch*
//! (a broad and a narrow label compared as equivalent).
//!
//! Three rules follow, and each is a type rather than a guideline.
//!
//! The local term is never dropped. [`TermBinding`] stores it as a required field and there is
//! no constructor that omits it, because binding "Glioblastoma multiforme, WHO grade IV" to
//! `MONDO:0018177` and keeping only the code destroys the sentence a pathologist wrote. 25.03
//! requires the local terminology to survive; 28.19 requires the same for historical diagnoses.
//!
//! An unmapped term stays unmapped. [`Binding::Unmapped`] carries a reason and there is no path
//! from it to a code. Nothing in this module picks a nearest match, and nothing scores string
//! similarity, because the blueprint's answer to an absent oracle is to report the absence.
//!
//! Every identifier carries a release. An [`OntologyId`] without one cannot be constructed, so a
//! mapping made against Mondo 2024-01 can be told apart from the same CURIE resolved against
//! 2026-06 — that difference is [`crate::Incomparability::OntologyVersionDrift`], not a match.
//!
//! Not implemented, deliberately: no ontology files, no term downloads, no subsumption
//! reasoning. This module cannot tell you that `MONDO:0018177` is a descendant of
//! `MONDO:0005070`, and it does not pretend to: [`MappingPrecision`] records the relation the
//! *curator* asserted. Inferring hierarchy without the ontology loaded would be invention.

use crate::error::{Incomparability, OntologyError, StandardsError};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// A versioned external identifier, such as `MONDO:0018177` at release `2026-06-04`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct OntologyId {
    pub prefix: String,
    pub local_id: String,
    /// The ontology release this mapping was made against. Never empty.
    pub release: String,
}

impl OntologyId {
    /// Parses `PREFIX:LOCAL` and attaches a release.
    ///
    /// The grammar is deliberately narrow: one colon, an alphabetic-initial prefix, a non-empty
    /// local part, and no whitespace anywhere. A permissive parser would accept
    /// `"NCIt: C3058 "` and then fail to match the same concept written without the space.
    pub fn parse(curie: &str, release: &str) -> Result<Self, OntologyError> {
        if release.trim().is_empty() {
            return Err(OntologyError::MissingRelease {
                curie: curie.to_string(),
            });
        }
        if curie.chars().any(char::is_whitespace) {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "contains whitespace".to_string(),
            });
        }
        let Some((prefix, local_id)) = curie.split_once(':') else {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "no ':' separating prefix from local identifier".to_string(),
            });
        };
        if local_id.contains(':') {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "more than one ':'".to_string(),
            });
        }
        if prefix.is_empty() || !prefix.starts_with(|c: char| c.is_ascii_alphabetic()) {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "prefix must begin with an ASCII letter".to_string(),
            });
        }
        if !prefix
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "prefix has characters outside [A-Za-z0-9_.]".to_string(),
            });
        }
        if local_id.is_empty() {
            return Err(OntologyError::MalformedCurie {
                value: curie.to_string(),
                reason: "empty local identifier".to_string(),
            });
        }
        Ok(OntologyId {
            prefix: prefix.to_string(),
            local_id: local_id.to_string(),
            release: release.to_string(),
        })
    }

    pub fn curie(&self) -> String {
        format!("{}:{}", self.prefix, self.local_id)
    }

    /// Same concept, ignoring which release it was resolved against.
    pub fn same_concept(&self, other: &OntologyId) -> bool {
        self.prefix == other.prefix && self.local_id == other.local_id
    }
}

impl fmt::Display for OntologyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.curie(), self.release)
    }
}

/// How closely the identifier matches the local term, as asserted by a curator.
///
/// Only [`MappingPrecision::Exact`] supports treating two bound terms as the same thing.
/// Comparing a broad label with a narrow one is the granularity mismatch 28.19 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MappingPrecision {
    Exact,
    /// The identifier denotes a superset of the local term.
    Broader,
    /// The identifier denotes a subset of the local term.
    Narrower,
    /// Associated but neither a superset nor a subset.
    Related,
}

impl MappingPrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            MappingPrecision::Exact => "exact",
            MappingPrecision::Broader => "broader",
            MappingPrecision::Narrower => "narrower",
            MappingPrecision::Related => "related",
        }
    }

    pub fn is_exact(self) -> bool {
        self == MappingPrecision::Exact
    }
}

/// The outcome of trying to bind a local term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "binding", rename_all = "snake_case")]
pub enum Binding {
    Mapped {
        id: OntologyId,
        precision: MappingPrecision,
    },
    /// Several identifiers are defensible and no oracle chooses between them.
    ///
    /// Kept as a state rather than resolved by a tiebreak: 28.19 calls forcing an ambiguous
    /// concept into one code "false precision" and asks for the distribution instead.
    Ambiguous { candidates: Vec<OntologyId> },
    /// No identifier applies, or none was found. The reason is required.
    Unmapped { reason: String },
}

/// A local term together with whatever external binding it has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermBinding {
    local_term: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_vocabulary: Option<String>,
    binding: Binding,
}

impl TermBinding {
    /// The general constructor. Rejects an empty local term.
    pub fn new(local_term: impl Into<String>, binding: Binding) -> Result<Self, OntologyError> {
        let local_term = local_term.into();
        if local_term.trim().is_empty() {
            let target = match &binding {
                Binding::Mapped { id, .. } => id.to_string(),
                Binding::Ambiguous { candidates } => format!("{} candidates", candidates.len()),
                Binding::Unmapped { .. } => "nothing".to_string(),
            };
            return Err(OntologyError::EmptyLocalTerm { target });
        }
        Ok(TermBinding {
            local_term,
            source_vocabulary: None,
            binding,
        })
    }

    pub fn mapped(
        local_term: impl Into<String>,
        id: OntologyId,
        precision: MappingPrecision,
    ) -> Result<Self, OntologyError> {
        TermBinding::new(local_term, Binding::Mapped { id, precision })
    }

    pub fn exact(local_term: impl Into<String>, id: OntologyId) -> Result<Self, OntologyError> {
        TermBinding::mapped(local_term, id, MappingPrecision::Exact)
    }

    pub fn ambiguous(
        local_term: impl Into<String>,
        candidates: Vec<OntologyId>,
    ) -> Result<Self, OntologyError> {
        TermBinding::new(local_term, Binding::Ambiguous { candidates })
    }

    pub fn unmapped(
        local_term: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<Self, OntologyError> {
        TermBinding::new(
            local_term,
            Binding::Unmapped {
                reason: reason.into(),
            },
        )
    }

    /// Records which local vocabulary the term came from, such as a site's pathology dictionary.
    pub fn from_vocabulary(mut self, vocabulary: impl Into<String>) -> Self {
        self.source_vocabulary = Some(vocabulary.into());
        self
    }

    /// The original wording. Present for every binding, mapped or not.
    pub fn local_term(&self) -> &str {
        &self.local_term
    }

    pub fn source_vocabulary(&self) -> Option<&str> {
        self.source_vocabulary.as_deref()
    }

    pub fn binding(&self) -> &Binding {
        &self.binding
    }

    pub fn is_unmapped(&self) -> bool {
        matches!(self.binding, Binding::Unmapped { .. })
    }

    pub fn is_ambiguous(&self) -> bool {
        matches!(self.binding, Binding::Ambiguous { .. })
    }

    /// The single identifier this term denotes, when there is exactly one.
    ///
    /// Ambiguous and unmapped bindings return an error. There is no `resolve_or_best_guess`.
    pub fn resolve(&self) -> Result<&OntologyId, OntologyError> {
        match &self.binding {
            Binding::Mapped { id, .. } => Ok(id),
            Binding::Ambiguous { candidates } => Err(OntologyError::NotResolvable {
                local_term: self.local_term.clone(),
                reason: format!("{} candidate identifiers", candidates.len()),
            }),
            Binding::Unmapped { reason } => Err(OntologyError::NotResolvable {
                local_term: self.local_term.clone(),
                reason: reason.clone(),
            }),
        }
    }

    pub fn precision(&self) -> Option<MappingPrecision> {
        match &self.binding {
            Binding::Mapped { precision, .. } => Some(*precision),
            _ => None,
        }
    }

    /// The identifier and asserted precision, or the reason there is not exactly one.
    fn require_mapped(&self) -> Result<(&OntologyId, MappingPrecision), Incomparability> {
        match &self.binding {
            Binding::Mapped { id, precision } => Ok((id, *precision)),
            Binding::Ambiguous { candidates } => Err(Incomparability::AmbiguousTerm {
                local_term: self.local_term.clone(),
                candidates: candidates.len(),
            }),
            Binding::Unmapped { reason } => Err(Incomparability::UnmappedTerm {
                local_term: self.local_term.clone(),
                reason: reason.clone(),
            }),
        }
    }

    /// Whether two bound terms may be treated as denoting the same concept.
    ///
    /// The checks run in the order the failure modes appear in 28.19: unresolvable first, then
    /// namespace, then version drift, then granularity, then identity.
    pub fn comparable_with(&self, other: &TermBinding) -> Result<(), Incomparability> {
        let (mine, my_precision) = self.require_mapped()?;
        let (theirs, their_precision) = other.require_mapped()?;
        if mine.prefix != theirs.prefix {
            return Err(Incomparability::NamespaceMismatch {
                left: mine.prefix.clone(),
                right: theirs.prefix.clone(),
            });
        }
        if mine.same_concept(theirs) && mine.release != theirs.release {
            return Err(Incomparability::OntologyVersionDrift {
                curie: mine.curie(),
                left_release: mine.release.clone(),
                right_release: theirs.release.clone(),
            });
        }
        if !my_precision.is_exact() || !their_precision.is_exact() {
            return Err(Incomparability::GranularityMismatch {
                left: self.local_term.clone(),
                right: other.local_term.clone(),
                left_precision: my_precision.as_str().to_string(),
                right_precision: their_precision.as_str().to_string(),
            });
        }
        if !mine.same_concept(theirs) {
            return Err(Incomparability::TermMismatch {
                left: mine.curie(),
                right: theirs.curie(),
            });
        }
        Ok(())
    }
}

/// A set of bindings for one world or dataset, keyed by local term.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TermCatalog {
    entries: BTreeMap<String, TermBinding>,
}

impl TermCatalog {
    pub fn new() -> Self {
        TermCatalog::default()
    }

    /// Adds a binding.
    ///
    /// Re-adding an identical binding is accepted; binding the same local term to something
    /// different is [`OntologyError::ConflictingBinding`] rather than a last-writer-wins
    /// overwrite, because an overwrite is version drift performed in memory.
    pub fn bind(&mut self, binding: TermBinding) -> Result<(), OntologyError> {
        if let Some(existing) = self.entries.get(binding.local_term()) {
            if existing != &binding {
                return Err(OntologyError::ConflictingBinding {
                    local_term: binding.local_term().to_string(),
                });
            }
            return Ok(());
        }
        self.entries
            .insert(binding.local_term().to_string(), binding);
        Ok(())
    }

    pub fn get(&self, local_term: &str) -> Option<&TermBinding> {
        self.entries.get(local_term)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &TermBinding> {
        self.entries.values()
    }

    /// Terms that carry no external identifier.
    ///
    /// These are the ones a release gate should surface. They are not errors and not dropped;
    /// they are the honest remainder of a mapping pass.
    pub fn unmapped(&self) -> impl Iterator<Item = &TermBinding> {
        self.entries.values().filter(|b| b.is_unmapped())
    }

    pub fn ambiguous(&self) -> impl Iterator<Item = &TermBinding> {
        self.entries.values().filter(|b| b.is_ambiguous())
    }

    /// Identifiers that more than one local term maps onto, with those terms.
    ///
    /// A merge is not automatically wrong — synonyms exist — but 28.19 treats an unexamined
    /// merge as how granularity mismatch enters a dataset, so the catalog can list them.
    pub fn merged_targets(&self) -> Vec<(String, Vec<String>)> {
        let mut by_target: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for binding in self.entries.values() {
            if let Binding::Mapped { id, .. } = binding.binding() {
                by_target
                    .entry(id.curie())
                    .or_default()
                    .push(binding.local_term().to_string());
            }
        }
        by_target
            .into_iter()
            .filter(|(_, terms)| terms.len() > 1)
            .collect()
    }

    /// A content hash of the whole catalog, for pinning a mapping version in a receipt.
    ///
    /// 28.19's release gate requires ontology versions to be pinned; a digest over the catalog
    /// is how a downstream artefact states which mapping it used without copying it.
    pub fn digest(&self) -> Result<ContentHash, StandardsError> {
        let value =
            serde_json::to_value(self).map_err(|error| StandardsError::Encoding {
                subject: "term catalog".to_string(),
                detail: error.to_string(),
            })?;
        ContentHash::of_value(&value).map_err(|error| StandardsError::Encoding {
            subject: "term catalog".to_string(),
            detail: error.to_string(),
        })
    }
}
