//! Information-flow labels and the agent-to-agent disclosure case.
//!
//! Blueprint 23.14 (information-flow labels, the flow rule, declassification, taint propagation)
//! and 23.44's replication privacy rule ("redacted replicas include omission proofs or counts").
//!
//! # What this is not
//!
//! `bioprism-policy` implements 43.33's information-flow *fibers* — labels attached to the data
//! products of a decision pipeline, tracked through transformation. This module is not a second
//! implementation of that and does not want to be. What is left over, and what §23 actually needs,
//! is the **participant-to-participant** case: two agents are separate principals with separate
//! clearances, and the question is not "where may this column go" but *what may this agent learn
//! from that one*, including everything it learns without being told.
//!
//! # Three claims the types enforce
//!
//! **An unlabelled value is not a public value.** This is the whole reason [`Labelling`] exists
//! instead of a bare [`FlowLabel`]. A value that arrived without a label has *unknown*
//! sensitivity, and the only safe reading of unknown is "not cleared for anyone". Every flow out
//! of [`Labelling::Unlabelled`] is [`FlowDecision::Refused`] with
//! [`FlowRefusal::SourceUnlabelled`], and there is no constructor that turns an unlabelled value
//! into a public one — only [`declassify`], which demands a declassifier and emits provenance.
//! The same rule applies to the sub-fields: [`PurposeRestriction::Only`] with an empty set permits
//! *no* purpose, which is why it is a distinct variant from [`PurposeRestriction::Unrestricted`]
//! and why [`FlowLabel`] does not implement `Default`.
//!
//! **Withholding is itself a channel.** A recipient that receives eleven of twelve records and is
//! told nothing learns nothing; a recipient told "one record withheld, reason: compartment" learns
//! that a twelfth record exists and something about its label. Both are information flows and
//! [`Projection`] records which one it performed, per item, as a [`Residual`]. `bioprism-weave`'s
//! capsule doctrine is that withholding is never silent; the cost of that doctrine is a residual
//! channel, and the honest thing is to name it rather than pretend disclosure of an omission is
//! free.
//!
//! **Generation does not declassify.** 23.14: "Model outputs are treated as potentially containing
//! source data". [`taint_join`] is the join of all input labels and there is no path from a set of
//! restricted inputs to a public output except through a [`Declassifier`].
//!
//! # Not implemented
//!
//! No enforcement. Nothing here intercepts a value; a caller must route its disclosures through
//! [`Projection`] for any of this to bind. No cryptography: an "omission proof" in 23.44 is a
//! proof, and [`Residual::Counted`] is a count. No jurisdiction ontology — residency strings are
//! compared for equality, not interpreted. No clock, so retention is compared in declared units
//! and never elapsed.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.14's "sensitivity level". Ordered because the flow rule needs domination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Sensitivity {
    Public,
    Internal,
    Confidential,
    Restricted,
}

/// 23.14's "purpose restrictions".
///
/// Two variants rather than a set, because an empty set of permitted purposes and an absence of
/// purpose restriction are opposite meanings and a single `BTreeSet` cannot hold both.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "purpose", content = "set")]
pub enum PurposeRestriction {
    Unrestricted,
    Only(BTreeSet<String>),
}

impl PurposeRestriction {
    pub fn only(purposes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        PurposeRestriction::Only(purposes.into_iter().map(Into::into).collect())
    }

    /// Whether every purpose `inner` permits is permitted here.
    pub fn permits_all_of(&self, inner: &PurposeRestriction) -> bool {
        match (self, inner) {
            (PurposeRestriction::Unrestricted, _) => true,
            (PurposeRestriction::Only(_), PurposeRestriction::Unrestricted) => false,
            (PurposeRestriction::Only(outer), PurposeRestriction::Only(inner)) => {
                inner.is_subset(outer)
            }
        }
    }
}

/// 23.14's "retention policy", in declared days.
///
/// A `None` maximum is *no declared retention limit*, which under this module's reading is the
/// permissive case only because 23.14 lists retention as an attribute a label carries rather than
/// a restriction a label imposes when absent. Retention is the one field where absence is treated
/// as unconstrained, and it is called out here so the asymmetry with purpose is deliberate rather
/// than accidental.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Retention {
    pub max_days: Option<u32>,
}

impl Retention {
    pub fn days(days: u32) -> Self {
        Retention {
            max_days: Some(days),
        }
    }

    pub fn unbounded() -> Self {
        Retention { max_days: None }
    }

    /// Whether a destination retaining for `inner` respects a source permitting `self`.
    pub fn permits(&self, inner: &Retention) -> bool {
        match (self.max_days, inner.max_days) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(outer), Some(inner)) => inner <= outer,
        }
    }

    fn meet(&self, other: &Retention) -> Retention {
        Retention {
            max_days: match (self.max_days, other.max_days) {
                (None, other) => other,
                (mine, None) => mine,
                (Some(a), Some(b)) => Some(a.min(b)),
            },
        }
    }
}

/// 23.14's label: sensitivity, compartments, purpose, residency, retention, legal basis.
///
/// No `Default`, because the default of every field is the permissive reading and a label a caller
/// forgot to fill in must not be the label that lets everything through.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct FlowLabel {
    pub sensitivity: Sensitivity,
    pub compartments: BTreeSet<String>,
    pub purpose: PurposeRestriction,
    pub residency: BTreeSet<String>,
    pub retention: Retention,
    pub legal_basis: Option<String>,
}

impl FlowLabel {
    /// A label at one sensitivity with no compartments, unrestricted purpose, no residency
    /// constraint and unbounded retention. Named `open_at` rather than `new` because it is the
    /// weakest label at its level and a reader should see that at the call site.
    pub fn open_at(sensitivity: Sensitivity) -> Self {
        FlowLabel {
            sensitivity,
            compartments: BTreeSet::new(),
            purpose: PurposeRestriction::Unrestricted,
            residency: BTreeSet::new(),
            retention: Retention::unbounded(),
            legal_basis: None,
        }
    }

    pub fn in_compartment(mut self, compartment: impl Into<String>) -> Self {
        self.compartments.insert(compartment.into());
        self
    }

    pub fn for_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = match self.purpose {
            PurposeRestriction::Unrestricted => {
                PurposeRestriction::Only([purpose.into()].into_iter().collect())
            }
            PurposeRestriction::Only(mut set) => {
                set.insert(purpose.into());
                PurposeRestriction::Only(set)
            }
        };
        self
    }

    pub fn resident_in(mut self, jurisdiction: impl Into<String>) -> Self {
        self.residency.insert(jurisdiction.into());
        self
    }

    pub fn retained_for(mut self, days: u32) -> Self {
        self.retention = Retention::days(days);
        self
    }

    /// The join of two labels: the least label that dominates both.
    ///
    /// Compartments union (more restrictive), purposes intersect, residency intersects, retention
    /// takes the shorter window. This is 23.14's taint rule for a value derived from two sources.
    pub fn join(&self, other: &FlowLabel) -> FlowLabel {
        let residency = if self.residency.is_empty() {
            other.residency.clone()
        } else if other.residency.is_empty() {
            self.residency.clone()
        } else {
            self.residency.intersection(&other.residency).cloned().collect()
        };
        FlowLabel {
            sensitivity: self.sensitivity.max(other.sensitivity),
            compartments: self.compartments.union(&other.compartments).cloned().collect(),
            purpose: match (&self.purpose, &other.purpose) {
                (PurposeRestriction::Unrestricted, p) | (p, PurposeRestriction::Unrestricted) => {
                    p.clone()
                }
                (PurposeRestriction::Only(a), PurposeRestriction::Only(b)) => {
                    PurposeRestriction::Only(a.intersection(b).cloned().collect())
                }
            },
            residency,
            retention: self.retention.meet(&other.retention),
            legal_basis: self.legal_basis.clone().or_else(|| other.legal_basis.clone()),
        }
    }

    /// 23.14's flow rule: information may flow from `self` to `destination` only when the
    /// destination policy dominates all restrictions.
    pub fn flows_to(&self, destination: &FlowLabel) -> FlowDecision {
        let mut refusals = Vec::new();
        if destination.sensitivity < self.sensitivity {
            refusals.push(FlowRefusal::SensitivityNotDominated {
                source: self.sensitivity,
                destination: destination.sensitivity,
            });
        }
        let missing: BTreeSet<String> = self
            .compartments
            .difference(&destination.compartments)
            .cloned()
            .collect();
        if !missing.is_empty() {
            refusals.push(FlowRefusal::CompartmentsMissing { missing });
        }
        if !self.purpose.permits_all_of(&destination.purpose) {
            refusals.push(FlowRefusal::PurposeNotPermitted {
                source: self.purpose.clone(),
                destination: destination.purpose.clone(),
            });
        }
        if !self.residency.is_empty() {
            let outside: BTreeSet<String> = destination
                .residency
                .difference(&self.residency)
                .cloned()
                .collect();
            if destination.residency.is_empty() || !outside.is_empty() {
                refusals.push(FlowRefusal::ResidencyNotPermitted {
                    permitted: self.residency.clone(),
                    destination: destination.residency.clone(),
                });
            }
        }
        if !self.retention.permits(&destination.retention) {
            refusals.push(FlowRefusal::RetentionExceeded {
                permitted: self.retention,
                destination: destination.retention,
            });
        }
        if refusals.is_empty() {
            FlowDecision::Permitted
        } else {
            FlowDecision::Refused { refusals }
        }
    }
}

/// A value's labelling state.
///
/// The reason this is not `Option<FlowLabel>`: an `Option` invites `unwrap_or(public)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "labelling", content = "label")]
pub enum Labelling {
    Labelled(FlowLabel),
    /// No label was recorded. Not public, not restricted: unknown, and therefore refused.
    Unlabelled,
}

impl Labelling {
    pub fn label(&self) -> Option<&FlowLabel> {
        match self {
            Labelling::Labelled(label) => Some(label),
            Labelling::Unlabelled => None,
        }
    }

    /// The join of two labellings. Unlabelled is absorbing: anything derived from an unlabelled
    /// input is itself unlabelled, because the derivation cannot establish what it may contain.
    pub fn join(&self, other: &Labelling) -> Labelling {
        match (self, other) {
            (Labelling::Labelled(a), Labelling::Labelled(b)) => Labelling::Labelled(a.join(b)),
            _ => Labelling::Unlabelled,
        }
    }

    pub fn flows_to(&self, destination: &Labelling) -> FlowDecision {
        match (self, destination) {
            (Labelling::Unlabelled, _) => FlowDecision::Refused {
                refusals: vec![FlowRefusal::SourceUnlabelled],
            },
            (_, Labelling::Unlabelled) => FlowDecision::Refused {
                refusals: vec![FlowRefusal::DestinationUnlabelled],
            },
            (Labelling::Labelled(source), Labelling::Labelled(destination)) => {
                source.flows_to(destination)
            }
        }
    }
}

/// The taint rule of 23.14: a derived artifact carries the join of its inputs.
///
/// An empty input list yields [`Labelling::Unlabelled`] rather than a public label, because a value
/// with no recorded provenance is exactly the case this module refuses to treat as safe.
pub fn taint_join(inputs: &[Labelling]) -> Labelling {
    let mut iter = inputs.iter();
    match iter.next() {
        None => Labelling::Unlabelled,
        Some(first) => iter.fold(first.clone(), |acc, next| acc.join(next)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum FlowDecision {
    Permitted,
    Refused { refusals: Vec<FlowRefusal> },
}

impl FlowDecision {
    pub fn permitted(&self) -> bool {
        matches!(self, FlowDecision::Permitted)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum FlowRefusal {
    /// The source carries no label. Absence of a label is not a public label.
    SourceUnlabelled,
    /// The destination declares no clearance, so nothing can be shown to dominate.
    DestinationUnlabelled,
    SensitivityNotDominated {
        source: Sensitivity,
        destination: Sensitivity,
    },
    CompartmentsMissing {
        missing: BTreeSet<String>,
    },
    PurposeNotPermitted {
        source: PurposeRestriction,
        destination: PurposeRestriction,
    },
    ResidencyNotPermitted {
        permitted: BTreeSet<String>,
        destination: BTreeSet<String>,
    },
    RetentionExceeded {
        permitted: Retention,
        destination: Retention,
    },
}

/// 23.14's declassification interface. Every variant is an explicit callable transformation, and
/// every application emits provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Declassifier {
    /// Refuses below a minimum cohort, because an aggregate over one row is that row.
    Aggregate { min_cohort: u32 },
    RedactDirectIdentifiers { fields: BTreeSet<String> },
    SummariseWithoutSensitiveFields { excluded: BTreeSet<String> },
    Tokenize,
    SyntheticExample,
    HumanReviewedRelease { reviewer: String },
}

impl Declassifier {
    /// The sensitivity a successful application lands at.
    ///
    /// Every declassifier lowers by exactly one level except human review, which is the only one
    /// 23.14 describes as a judgement rather than a transformation and therefore the only one
    /// permitted to reach [`Sensitivity::Public`] in one step.
    fn target(&self, source: Sensitivity) -> Sensitivity {
        match self {
            Declassifier::HumanReviewedRelease { .. } => Sensitivity::Public,
            _ => match source {
                Sensitivity::Restricted => Sensitivity::Confidential,
                Sensitivity::Confidential => Sensitivity::Internal,
                Sensitivity::Internal | Sensitivity::Public => Sensitivity::Public,
            },
        }
    }
}

/// The record a declassification emits. 23.14: "Every declassification emits provenance and a
/// verifier result."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declassification {
    pub declassifier: Declassifier,
    pub source: FlowLabel,
    pub result: FlowLabel,
    pub verifier: VerifierResult,
    /// Compartments the transformation did *not* clear. Declassification lowers sensitivity; it
    /// does not dissolve a compartment, and a caller expecting otherwise gets to see the leftovers.
    pub retained_compartments: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verifier")]
pub enum VerifierResult {
    Passed { method: String },
    /// A declassifier that ran but whose output nobody checked. Recorded rather than treated as a
    /// pass, so a bundle can be audited for declassifications nothing verified.
    Unverified { reason: String },
}

/// Apply a declassifier, refusing an unlabelled source outright.
pub fn declassify(
    value: &Labelling,
    declassifier: &Declassifier,
    cohort_size: u32,
    verifier: VerifierResult,
) -> Result<(Labelling, Declassification), FlowError> {
    let source = match value {
        Labelling::Unlabelled => return Err(FlowError::DeclassifyUnlabelled),
        Labelling::Labelled(label) => label.clone(),
    };
    if let Declassifier::Aggregate { min_cohort } = declassifier {
        if cohort_size < *min_cohort {
            return Err(FlowError::CohortTooSmall {
                required: *min_cohort,
                actual: cohort_size,
            });
        }
    }
    let mut result = source.clone();
    result.sensitivity = declassifier.target(source.sensitivity);
    let record = Declassification {
        declassifier: declassifier.clone(),
        source,
        result: result.clone(),
        verifier,
        retained_compartments: result.compartments.clone(),
    };
    Ok((Labelling::Labelled(result), record))
}

/// A participant as an information-flow principal: an identifier and the ceiling it is cleared to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Principal {
    pub id: String,
    pub clearance: Labelling,
}

impl Principal {
    pub fn new(id: impl Into<String>, clearance: FlowLabel) -> Self {
        Principal {
            id: id.into(),
            clearance: Labelling::Labelled(clearance),
        }
    }

    /// A participant whose clearance was never established. It receives nothing, which is the
    /// point: an unclearance is not a public clearance either.
    pub fn uncleared(id: impl Into<String>) -> Self {
        Principal {
            id: id.into(),
            clearance: Labelling::Unlabelled,
        }
    }

    /// Bridge to the kernel's clearance vocabulary. `bioprism_weave::Recipient` holds a flat set of
    /// string labels; this renders the parts of a [`FlowLabel`] that survive that shape, and the
    /// parts that do not survive are what [`Principal::capsule_clearance_loss`] reports.
    pub fn capsule_clearance(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        if let Labelling::Labelled(label) = &self.clearance {
            out.insert(format!("sensitivity:{:?}", label.sensitivity).to_lowercase());
            for compartment in &label.compartments {
                out.insert(format!("compartment:{compartment}"));
            }
        }
        out
    }

    /// What a flat string clearance set cannot express. Named because 23.01's adapter principle
    /// applies inside one codebase too: a projection that loses purpose, residency and retention
    /// must say so rather than report compatibility.
    pub fn capsule_clearance_loss(&self) -> Vec<&'static str> {
        match &self.clearance {
            Labelling::Unlabelled => vec!["clearance-absent"],
            Labelling::Labelled(label) => {
                let mut loss = Vec::new();
                if label.purpose != PurposeRestriction::Unrestricted {
                    loss.push("purpose-restriction");
                }
                if !label.residency.is_empty() {
                    loss.push("residency");
                }
                if label.retention.max_days.is_some() {
                    loss.push("retention");
                }
                if label.legal_basis.is_some() {
                    loss.push("legal-basis");
                }
                loss
            }
        }
    }
}

/// One item a projection may or may not release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    pub labelling: Labelling,
    /// A schema name. Released even when content is not, when the residual policy permits shape.
    pub shape: String,
}

/// What a recipient learns about an item it was not given.
///
/// This is the agent-to-agent case that has no analogue in a single-process label checker: the
/// *decision to withhold* is observable, and how observable is a policy choice with consequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Residual {
    /// The recipient is told nothing. It cannot distinguish "withheld" from "does not exist".
    Silent,
    /// The recipient is told how many items were withheld, and nothing else.
    Counted,
    /// The recipient is told the count and each withheld item's schema.
    ShapeDisclosed,
    /// The recipient is told the count, the shapes, and why each was withheld. Maximum
    /// accountability, maximum residual channel.
    ReasonDisclosed,
}

/// How much a projection reveals about what it withheld.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResidualPolicy {
    pub residual: Residual,
}

impl ResidualPolicy {
    pub fn new(residual: Residual) -> Self {
        ResidualPolicy { residual }
    }
}

/// A withheld item, rendered at the policy's residual level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Withheld {
    /// Present only at [`Residual::ShapeDisclosed`] and above.
    pub shape: Option<String>,
    /// Present only at [`Residual::ReasonDisclosed`].
    pub refusals: Option<Vec<FlowRefusal>>,
}

/// The result of projecting a set of items for one recipient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub recipient: String,
    pub released: Vec<Item>,
    /// Empty at [`Residual::Silent`] even when items were withheld; [`Projection::withheld_count`]
    /// is the field that is always accurate and is deliberately not serialised to the recipient.
    pub withheld: Vec<Withheld>,
    pub residual: Residual,
    withheld_count: usize,
}

impl Projection {
    /// The true number withheld, whatever the residual policy discloses. A caller auditing a
    /// projection sees this; the recipient sees [`Projection::withheld`].
    pub fn withheld_count(&self) -> usize {
        self.withheld_count
    }

    /// Whether the recipient can tell that anything was withheld at all.
    pub fn recipient_can_detect_omission(&self) -> bool {
        self.withheld_count > 0 && self.residual != Residual::Silent
    }
}

/// Project items for a recipient under a residual policy.
///
/// Order is preserved from the input, so two runs over the same inputs produce byte-identical
/// projections.
pub fn project_for(
    recipient: &Principal,
    items: &[Item],
    policy: ResidualPolicy,
) -> Projection {
    let mut released = Vec::new();
    let mut withheld = Vec::new();
    let mut withheld_count = 0usize;
    for item in items {
        match item.labelling.flows_to(&recipient.clearance) {
            FlowDecision::Permitted => released.push(item.clone()),
            FlowDecision::Refused { refusals } => {
                withheld_count += 1;
                match policy.residual {
                    Residual::Silent => {}
                    Residual::Counted => withheld.push(Withheld {
                        shape: None,
                        refusals: None,
                    }),
                    Residual::ShapeDisclosed => withheld.push(Withheld {
                        shape: Some(item.shape.clone()),
                        refusals: None,
                    }),
                    Residual::ReasonDisclosed => withheld.push(Withheld {
                        shape: Some(item.shape.clone()),
                        refusals: Some(refusals),
                    }),
                }
            }
        }
    }
    Projection {
        recipient: recipient.id.clone(),
        released,
        withheld,
        residual: policy.residual,
        withheld_count,
    }
}

/// What one participant may learn from another, item by item.
///
/// Two things have to be separate for this to be more than a clearance comparison: what a
/// participant is *cleared for* and what it actually *holds*. A participant cleared for a
/// compartment it has no data from can disclose nothing about it, and a participant holding
/// something it is not itself cleared for is a different problem. The matrix is therefore built
/// from (principal, holdings) pairs, and `may_learn(a, b)` and `may_learn(b, a)` are computed from
/// different holdings.
///
/// This is the asymmetry a fabric storing one "trust level" per pair cannot represent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisclosureMatrix {
    entries: BTreeMap<(String, String), Vec<String>>,
}

impl DisclosureMatrix {
    /// For each ordered pair, which of the holder's items may flow to the receiver.
    ///
    /// An item a holder is not itself cleared for is excluded from what it may pass on, because a
    /// participant cannot disclose what it may not read.
    pub fn compute(holdings: &[(Principal, Vec<Item>)]) -> Self {
        let mut entries: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for (holder, held) in holdings {
            for (receiver, _) in holdings {
                if holder.id == receiver.id {
                    continue;
                }
                let permitted: Vec<String> = held
                    .iter()
                    .filter(|item| {
                        item.labelling.flows_to(&holder.clearance).permitted()
                            && item.labelling.flows_to(&receiver.clearance).permitted()
                    })
                    .map(|item| item.id.clone())
                    .collect();
                entries.insert((holder.id.clone(), receiver.id.clone()), permitted);
            }
        }
        DisclosureMatrix { entries }
    }

    pub fn may_learn(&self, from: &str, to: &str) -> &[String] {
        self.entries
            .get(&(from.to_string(), to.to_string()))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Whether the relation happens to be symmetric for this pair. Rarely true, and the cases where
    /// it is are the cases where a single trust scalar would have been adequate.
    pub fn is_symmetric_for(&self, a: &str, b: &str) -> bool {
        self.may_learn(a, b) == self.may_learn(b, a)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FlowError {
    #[error("cannot declassify an unlabelled value: its content is unknown, so no transformation can be shown to have removed anything")]
    DeclassifyUnlabelled,

    #[error("aggregate declassifier requires a cohort of at least {required}, got {actual}")]
    CohortTooSmall { required: u32, actual: u32 },
}
