//! Specimen and aliquot lineage as a directed acyclic forest.
//!
//! Implements blueprint 25.04. Material has a history: it is collected from a subject, split,
//! extracted from, depleted and consumed. Every number an assay ever produces is downstream of
//! that history, and two facts about it decide whether a downstream evaluation is honest.
//!
//! The first is **shared ancestry**. Two specimens that trace to one piece of material are not
//! independent observations, whatever their identifiers say. If they land on opposite sides of
//! a train/test split, the split leaks. [`LineageGraph::leakage_risks`] finds those pairs; it
//! is a *precursor* detector, not a verdict — whether shared ancestry is disqualifying depends
//! on the split plan, which is 25.13's business, not this module's.
//!
//! The second is **depletion**. Material is finite and consumption is irreversible. 25.04
//! states it twice: "child material cannot exceed parent quantity" and "consumed material
//! cannot reappear". A derivative recorded as drawn from an exhausted or already-consumed
//! parent is not a modelling nicety — it means the record is wrong about which physical tube
//! the measurement came from, and every claim resting on it inherits that error.
//!
//! # Aliquots are specimens
//!
//! 25.04 lists Specimen and Aliquot as separate primary objects. They are one type here. An
//! aliquot is a specimen whose [`Origin`] is [`Origin::Derived`]; splitting the types would
//! mean ancestry queries return a sum type and callers would pattern-match their way into
//! forgetting one arm. The blueprint gives no field that distinguishes them, so nothing is
//! lost.
//!
//! # Not implemented
//!
//! 25.04 names ChainOfCustody as a primary object and lists no fields for it. Custody transfer
//! is modelled only as far as [`Specimen::scope`] and the collection site record it; a real
//! custody chain needs a holder, a transfer instant and an authority, and inventing that
//! triple here would be this crate's design, not the blueprint's.

use crate::error::LineageError;
use crate::ids::{SpecimenId, SubjectId};
use crate::quantity::Quantity;
use bioprism_scope::{ScopeKey, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// The act of taking material from a source entity. Roots a lineage tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectionEvent {
    pub subject: SubjectId,
    pub collected_at: Timestamp,
    /// Anatomical or environmental site, in the source vocabulary. Ontology binding is 25.03.
    pub site: String,
}

/// What was done to the parent to produce this material.
///
/// The variants that matter downstream are the ones that change what the material *is*:
/// an extraction yields a different analyte, a depletion removes a population, and neither
/// result is interchangeable with the parent even though both trace to it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "process", rename_all = "snake_case")]
pub enum ProcessKind {
    /// Division into portions of the same material.
    Split,
    /// A working portion drawn for a specific use.
    Aliquot,
    /// Isolation of an analyte; the product is not the parent material.
    Extraction { analyte: String },
    /// Removal of a component; the product is the parent minus something.
    Depletion { removed: String },
    /// Expansion in culture. Quantity may exceed the parent, so mass balance does not apply.
    Culture,
    /// Preservation or embedding that changes preservation state but not identity.
    Fixation,
    /// An adapter-specific process this crate does not model.
    Other { label: String },
}

impl ProcessKind {
    /// True when the product's quantity is bounded by what remained in the parent.
    ///
    /// Culture is the exception the mass-balance invariant needs: cells divide, so a cultured
    /// derivative legitimately holds more material than the vial it started from.
    pub fn conserves_mass(&self) -> bool {
        !matches!(self, ProcessKind::Culture)
    }
}

/// Where a specimen came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "origin", rename_all = "snake_case")]
pub enum Origin {
    Collected(CollectionEvent),
    Derived {
        parent: SpecimenId,
        process: ProcessKind,
        drawn_at: Timestamp,
    },
}

impl Origin {
    pub fn parent(&self) -> Option<&SpecimenId> {
        match self {
            Origin::Collected(_) => None,
            Origin::Derived { parent, .. } => Some(parent),
        }
    }

    /// The instant this material came into existence.
    pub fn existed_from(&self) -> Timestamp {
        match self {
            Origin::Collected(event) => event.collected_at,
            Origin::Derived { drawn_at, .. } => *drawn_at,
        }
    }
}

/// Destructive use of material.
///
/// `amount` is `None` when the whole remainder was consumed, which is the common case for a
/// destructive assay run on a working aliquot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumptionEvent {
    pub consumed_at: Timestamp,
    pub amount: Option<Quantity>,
    pub reason: String,
}

/// How confident the record is that this material belongs to the subject it names.
///
/// 25.04 requires identity confidence and conflicts to be *preserved*. `Disputed` therefore
/// carries the competing subjects rather than collapsing to a boolean, and nothing in this
/// module resolves a dispute — [`LineageGraph::validate`] only surfaces it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "confidence", rename_all = "snake_case")]
pub enum IdentityConfidence {
    /// Confirmed by an independent method, e.g. a genotype concordance check.
    Verified { method: String },
    /// Taken from the accompanying paperwork and not independently checked.
    Asserted,
    /// Two or more sources name different subjects for this material.
    Disputed { conflicting: BTreeSet<SubjectId> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityAssertion {
    pub asserted_subject: SubjectId,
    pub confidence: IdentityConfidence,
    /// Free-text provenance for the assertion. 25.11 owns structured evidence.
    pub evidence: Vec<String>,
}

/// One piece of biological material.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Specimen {
    pub id: SpecimenId,
    pub origin: Origin,
    /// Material type in the source vocabulary: "whole blood", "FFPE block", "plasma".
    pub material: String,
    pub preservation: String,
    /// The amount this specimen held when it came into existence.
    pub quantity: Quantity,
    /// Quality attributes keyed by metric name: RIN, A260/280, tumour cellularity.
    pub quality: BTreeMap<String, String>,
    pub consumption: Option<ConsumptionEvent>,
    /// `None` means identity is inherited from the lineage root with no independent claim.
    pub identity: Option<IdentityAssertion>,
    /// Consent and use labels. Splitting material cannot create permission it lacked.
    pub consent_labels: BTreeSet<String>,
    pub scope: ScopeKey,
}

impl Specimen {
    /// Constructs a root specimen with no quality attributes, consent labels or scope bindings.
    pub fn collected(
        id: SpecimenId,
        subject: SubjectId,
        collected_at: Timestamp,
        site: impl Into<String>,
        material: impl Into<String>,
        quantity: Quantity,
    ) -> Self {
        Specimen {
            id,
            origin: Origin::Collected(CollectionEvent {
                subject,
                collected_at,
                site: site.into(),
            }),
            material: material.into(),
            preservation: String::new(),
            quantity,
            quality: BTreeMap::new(),
            consumption: None,
            identity: None,
            consent_labels: BTreeSet::new(),
            scope: ScopeKey::new(),
        }
    }

    /// Constructs a derivative drawn from `parent`.
    pub fn derived(
        id: SpecimenId,
        parent: SpecimenId,
        process: ProcessKind,
        drawn_at: Timestamp,
        material: impl Into<String>,
        quantity: Quantity,
    ) -> Self {
        Specimen {
            id,
            origin: Origin::Derived {
                parent,
                process,
                drawn_at,
            },
            material: material.into(),
            preservation: String::new(),
            quantity,
            quality: BTreeMap::new(),
            consumption: None,
            identity: None,
            consent_labels: BTreeSet::new(),
            scope: ScopeKey::new(),
        }
    }

    pub fn with_consent(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.consent_labels = labels.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_identity(mut self, identity: IdentityAssertion) -> Self {
        self.identity = Some(identity);
        self
    }

    pub fn with_consumption(mut self, consumption: ConsumptionEvent) -> Self {
        self.consumption = Some(consumption);
        self
    }

    /// True when the whole remainder was destroyed at or before `at`.
    ///
    /// A partial consumption returns `false` here: whether material is left depends on the
    /// draws its children took, which only [`LineageGraph`] can see.
    pub fn fully_consumed_by(&self, at: Timestamp) -> bool {
        match &self.consumption {
            Some(event) => event.amount.is_none() && event.consumed_at <= at,
            None => false,
        }
    }
}

/// A diagnostic from [`LineageGraph::validate`].
///
/// Returned in bulk rather than as an `Err` because an adapter fixing a lineage extract needs
/// every violation at once; stopping at the first one turns a single pass into a dozen.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum LineageIssue {
    #[error("specimen {child} declares parent {parent}, which is not in the graph")]
    UnknownParent {
        child: SpecimenId,
        parent: SpecimenId,
    },

    #[error("lineage through specimen {specimen} is cyclic")]
    Cycle { specimen: SpecimenId },

    #[error("specimen {child} was drawn at {drawn_at} from {parent}, which was consumed at {consumed_at}")]
    DrawnFromConsumedParent {
        child: SpecimenId,
        parent: SpecimenId,
        drawn_at: Timestamp,
        consumed_at: Timestamp,
    },

    #[error("specimen {child} drew {requested} from {parent}, which had {remaining} left")]
    DrawnFromExhaustedParent {
        child: SpecimenId,
        parent: SpecimenId,
        requested: Quantity,
        remaining: Quantity,
    },

    #[error("draws from specimen {parent} total {drawn} but it held {capacity}")]
    MassBalanceExceeded {
        parent: SpecimenId,
        capacity: Quantity,
        drawn: Quantity,
    },

    #[error("specimen {child} was drawn at {drawn_at}, before parent {parent} existed at {parent_existed_from}")]
    DrawnBeforeParentExisted {
        child: SpecimenId,
        parent: SpecimenId,
        drawn_at: Timestamp,
        parent_existed_from: Timestamp,
    },

    #[error("specimen {child} is measured in {child_unit:?} but parent {parent} in {parent_unit:?}, so mass balance cannot be checked")]
    UnitMismatch {
        child: SpecimenId,
        parent: SpecimenId,
        child_unit: String,
        parent_unit: String,
    },

    #[error(
        "specimen {specimen} asserts subject {asserted} but its lineage root collected {inherited}"
    )]
    IdentityConflict {
        specimen: SpecimenId,
        asserted: SubjectId,
        inherited: SubjectId,
    },

    #[error("specimen {specimen} has a disputed identity between {count} subjects")]
    DisputedIdentity { specimen: SpecimenId, count: usize },

    #[error("specimen {child} carries consent label {label:?} that parent {parent} does not")]
    ConsentExpanded {
        child: SpecimenId,
        parent: SpecimenId,
        label: String,
    },
}

/// A reason two specimens are not independent observations.
///
/// This is a precursor to identity leakage, not leakage itself. Whether it matters depends on
/// how the cohort is split, which [`crate::cohort::SplitPlan`] decides.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LeakageRisk {
    #[error("specimens {left} and {right} both descend from material {ancestor}")]
    SharedMaterialAncestor {
        left: SpecimenId,
        right: SpecimenId,
        ancestor: SpecimenId,
    },

    #[error("specimens {left} and {right} were collected from subject {subject}")]
    SharedSourceSubject {
        left: SpecimenId,
        right: SpecimenId,
        subject: SubjectId,
    },
}

/// A forest of specimens keyed by identifier.
///
/// Insertion order does not affect any query: everything iterates a `BTreeMap`, so validation
/// output is stable across runs and comparable between implementations.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LineageGraph {
    specimens: BTreeMap<SpecimenId, Specimen>,
}

impl LineageGraph {
    pub fn new() -> Self {
        LineageGraph {
            specimens: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, specimen: Specimen) -> Result<(), LineageError> {
        if self.specimens.contains_key(&specimen.id) {
            return Err(LineageError::DuplicateSpecimen {
                specimen: specimen.id.to_string(),
            });
        }
        self.specimens.insert(specimen.id.clone(), specimen);
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.specimens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.specimens.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Specimen> {
        self.specimens.values()
    }

    pub fn get(&self, id: &SpecimenId) -> Result<&Specimen, LineageError> {
        self.specimens
            .get(id)
            .ok_or_else(|| LineageError::UnknownSpecimen {
                specimen: id.to_string(),
            })
    }

    /// Ancestors nearest first, excluding `id` itself.
    ///
    /// Fails rather than looping on a cycle. [`LineageGraph::validate`] catches that failure
    /// and reports it as [`LineageIssue::Cycle`] so a malformed extract still produces a full
    /// diagnostic list.
    pub fn ancestors(&self, id: &SpecimenId) -> Result<Vec<SpecimenId>, LineageError> {
        let mut chain = Vec::new();
        let mut seen = BTreeSet::new();
        seen.insert(id.clone());
        let mut cursor = self.get(id)?.origin.parent().cloned();
        while let Some(current) = cursor {
            if !seen.insert(current.clone()) {
                return Err(LineageError::Cycle {
                    specimen: current.to_string(),
                });
            }
            let parent = match self.specimens.get(&current) {
                Some(specimen) => specimen.origin.parent().cloned(),
                None => {
                    return Err(LineageError::UnknownParent {
                        child: chain.last().unwrap_or(id).to_string(),
                        parent: current.to_string(),
                    })
                }
            };
            chain.push(current);
            cursor = parent;
        }
        Ok(chain)
    }

    /// `id` followed by its ancestors. The set used for shared-ancestry tests.
    fn self_and_ancestors(&self, id: &SpecimenId) -> Result<Vec<SpecimenId>, LineageError> {
        let mut chain = vec![id.clone()];
        chain.extend(self.ancestors(id)?);
        Ok(chain)
    }

    pub fn lineage_root(&self, id: &SpecimenId) -> Result<&Specimen, LineageError> {
        match self.ancestors(id)?.last() {
            Some(root) => self.get(root),
            None => self.get(id),
        }
    }

    /// The collection event at the root of this material's lineage.
    pub fn collection(&self, id: &SpecimenId) -> Result<&CollectionEvent, LineageError> {
        match &self.lineage_root(id)?.origin {
            Origin::Collected(event) => Ok(event),
            Origin::Derived { parent, .. } => Err(LineageError::UnknownParent {
                child: id.to_string(),
                parent: parent.to_string(),
            }),
        }
    }

    /// The subject this material belongs to.
    ///
    /// A local [`IdentityAssertion`] wins over the inherited root subject, because an
    /// independent genotype check on an aliquot is exactly the evidence that would overturn
    /// the paperwork. When the two disagree the disagreement is *not* silently resolved:
    /// [`LineageGraph::validate`] reports [`LineageIssue::IdentityConflict`] for the same pair.
    pub fn source_subject(&self, id: &SpecimenId) -> Result<SubjectId, LineageError> {
        if let Some(identity) = &self.get(id)?.identity {
            return Ok(identity.asserted_subject.clone());
        }
        Ok(self.collection(id)?.subject.clone())
    }

    pub fn children_of(&self, id: &SpecimenId) -> Vec<&Specimen> {
        self.specimens
            .values()
            .filter(|specimen| specimen.origin.parent() == Some(id))
            .collect()
    }

    /// All descendants of `id` in breadth-first order.
    pub fn descendants(&self, id: &SpecimenId) -> Result<Vec<SpecimenId>, LineageError> {
        self.get(id)?;
        let mut found = Vec::new();
        let mut seen = BTreeSet::new();
        let mut frontier = vec![id.clone()];
        while let Some(current) = frontier.pop() {
            for child in self.children_of(&current) {
                if seen.insert(child.id.clone()) {
                    found.push(child.id.clone());
                    frontier.push(child.id.clone());
                }
            }
        }
        Ok(found)
    }

    /// The nearest piece of material both specimens descend from, if any.
    ///
    /// A specimen is its own ancestor for this purpose: an aliquot and the block it was cut
    /// from share material, and a split that separates them still leaks.
    pub fn nearest_shared_ancestor(
        &self,
        left: &SpecimenId,
        right: &SpecimenId,
    ) -> Result<Option<SpecimenId>, LineageError> {
        let right_chain: BTreeSet<SpecimenId> =
            self.self_and_ancestors(right)?.into_iter().collect();
        Ok(self
            .self_and_ancestors(left)?
            .into_iter()
            .find(|candidate| right_chain.contains(candidate)))
    }

    /// Total material drawn out of `id` by its direct derivatives.
    ///
    /// Cultured derivatives are excluded: their quantity is not drawn from the parent, it is
    /// grown. Counting it would report a mass-balance violation on a perfectly sound record.
    pub fn drawn_from(&self, id: &SpecimenId) -> Result<Quantity, LineageError> {
        let parent = self.get(id)?;
        let mut total = parent.quantity.zero_like();
        for child in self.children_of(id) {
            let conserving = match &child.origin {
                Origin::Derived { process, .. } => process.conserves_mass(),
                Origin::Collected(_) => false,
            };
            if conserving {
                total = total.add(&child.quantity, child.id.as_str())?;
            }
        }
        Ok(total)
    }

    /// What is left in `id` after every draw and any consumption.
    ///
    /// May be negative, which is precisely the mass-balance violation 25.04 forbids; the
    /// signed value is kept so the caller can see by how much.
    pub fn remaining(&self, id: &SpecimenId) -> Result<Quantity, LineageError> {
        let specimen = self.get(id)?;
        let drawn = self.drawn_from(id)?;
        let mut left = specimen.quantity.subtract(&drawn, id.as_str())?;
        if let Some(event) = &specimen.consumption {
            left = match &event.amount {
                Some(amount) => left.subtract(amount, id.as_str())?,
                None => left.zero_like(),
            };
        }
        Ok(left)
    }

    /// Pairs among `specimens` that are not independent observations.
    ///
    /// Shared material ancestry is reported in preference to a shared subject: it is the
    /// stronger relation and reporting both for the same pair would double-count one risk.
    pub fn leakage_risks(
        &self,
        specimens: &[SpecimenId],
    ) -> Result<Vec<LeakageRisk>, LineageError> {
        let mut risks = Vec::new();
        for (index, left) in specimens.iter().enumerate() {
            for right in specimens.iter().skip(index + 1) {
                if left == right {
                    continue;
                }
                if let Some(ancestor) = self.nearest_shared_ancestor(left, right)? {
                    risks.push(LeakageRisk::SharedMaterialAncestor {
                        left: left.clone(),
                        right: right.clone(),
                        ancestor,
                    });
                    continue;
                }
                let left_subject = self.source_subject(left)?;
                if left_subject == self.source_subject(right)? {
                    risks.push(LeakageRisk::SharedSourceSubject {
                        left: left.clone(),
                        right: right.clone(),
                        subject: left_subject,
                    });
                }
            }
        }
        Ok(risks)
    }

    /// Every structural, temporal, material, identity and consent violation in the graph.
    pub fn validate(&self) -> Vec<LineageIssue> {
        let mut issues = Vec::new();
        for specimen in self.specimens.values() {
            self.check_structure(specimen, &mut issues);
            self.check_identity(specimen, &mut issues);
            self.check_consent(specimen, &mut issues);
        }
        for specimen in self.specimens.values() {
            self.check_depletion(specimen, &mut issues);
        }
        issues
    }

    fn check_structure(&self, specimen: &Specimen, issues: &mut Vec<LineageIssue>) {
        let Origin::Derived {
            parent, drawn_at, ..
        } = &specimen.origin
        else {
            return;
        };
        let Some(parent_specimen) = self.specimens.get(parent) else {
            issues.push(LineageIssue::UnknownParent {
                child: specimen.id.clone(),
                parent: parent.clone(),
            });
            return;
        };
        if let Err(LineageError::Cycle { .. }) = self.ancestors(&specimen.id) {
            issues.push(LineageIssue::Cycle {
                specimen: specimen.id.clone(),
            });
            return;
        }
        let parent_existed_from = parent_specimen.origin.existed_from();
        if *drawn_at < parent_existed_from {
            issues.push(LineageIssue::DrawnBeforeParentExisted {
                child: specimen.id.clone(),
                parent: parent.clone(),
                drawn_at: *drawn_at,
                parent_existed_from,
            });
        }
        if let Some(event) = &parent_specimen.consumption {
            if event.amount.is_none() && *drawn_at >= event.consumed_at {
                issues.push(LineageIssue::DrawnFromConsumedParent {
                    child: specimen.id.clone(),
                    parent: parent.clone(),
                    drawn_at: *drawn_at,
                    consumed_at: event.consumed_at,
                });
            }
        }
        if specimen.quantity.unit != parent_specimen.quantity.unit {
            issues.push(LineageIssue::UnitMismatch {
                child: specimen.id.clone(),
                parent: parent.clone(),
                child_unit: specimen.quantity.unit.clone(),
                parent_unit: parent_specimen.quantity.unit.clone(),
            });
        }
    }

    /// Walks a parent's draws in time order and reports the first child that overdrew.
    ///
    /// Aggregate mass balance answers "was this record impossible?"; the ordered walk answers
    /// "which draw made it impossible?", which is the question an adapter author has.
    fn check_depletion(&self, parent: &Specimen, issues: &mut Vec<LineageIssue>) {
        let mut draws: Vec<(Timestamp, &Specimen)> = Vec::new();
        for child in self.children_of(&parent.id) {
            let Origin::Derived {
                process, drawn_at, ..
            } = &child.origin
            else {
                continue;
            };
            if process.conserves_mass() {
                draws.push((*drawn_at, child));
            }
        }
        if draws.is_empty() {
            return;
        }
        draws.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.id.cmp(&b.1.id)));

        let mut remaining = parent.quantity.clone();
        let mut drawn_total = parent.quantity.zero_like();
        for (_, child) in &draws {
            if child.quantity.unit != parent.quantity.unit {
                continue;
            }
            if child.quantity.amount > remaining.amount {
                issues.push(LineageIssue::DrawnFromExhaustedParent {
                    child: child.id.clone(),
                    parent: parent.id.clone(),
                    requested: child.quantity.clone(),
                    remaining: remaining.clone(),
                });
            }
            remaining.amount -= child.quantity.amount;
            drawn_total.amount += child.quantity.amount;
        }
        if drawn_total.amount > parent.quantity.amount {
            issues.push(LineageIssue::MassBalanceExceeded {
                parent: parent.id.clone(),
                capacity: parent.quantity.clone(),
                drawn: drawn_total,
            });
        }
    }

    fn check_identity(&self, specimen: &Specimen, issues: &mut Vec<LineageIssue>) {
        let Some(identity) = &specimen.identity else {
            return;
        };
        if let IdentityConfidence::Disputed { conflicting } = &identity.confidence {
            issues.push(LineageIssue::DisputedIdentity {
                specimen: specimen.id.clone(),
                count: conflicting.len(),
            });
        }
        if let Ok(collection) = self.collection(&specimen.id) {
            if collection.subject != identity.asserted_subject {
                issues.push(LineageIssue::IdentityConflict {
                    specimen: specimen.id.clone(),
                    asserted: identity.asserted_subject.clone(),
                    inherited: collection.subject.clone(),
                });
            }
        }
    }

    /// Consent narrows down a lineage and never widens.
    ///
    /// A derivative that carries a use label its parent lacks means someone re-consented the
    /// tube rather than the person, which is not a thing that can happen.
    fn check_consent(&self, specimen: &Specimen, issues: &mut Vec<LineageIssue>) {
        let Some(parent) = specimen.origin.parent() else {
            return;
        };
        let Some(parent_specimen) = self.specimens.get(parent) else {
            return;
        };
        for label in specimen
            .consent_labels
            .difference(&parent_specimen.consent_labels)
        {
            issues.push(LineageIssue::ConsentExpanded {
                child: specimen.id.clone(),
                parent: parent.clone(),
                label: label.clone(),
            });
        }
    }
}
