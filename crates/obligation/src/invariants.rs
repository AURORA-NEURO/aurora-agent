//! The ten non-compressible biological invariants, as a checkable closure constraint.
//!
//! Blueprint 39.05 lists ten protected classes and closes with the sentence this module exists to
//! enforce: *a context compiler may replace verbose representations of these classes with stable
//! codes and graph references, but it may not omit the semantics*.
//!
//! That sentence draws a line that a token counter cannot see. Replacing "genome reference build
//! GRCh38, 1-based, forward orientation" with `ref:grch38@1-based-fwd` is compression. Dropping
//! the coordinate system entirely is a different act, and so is emitting a code that resolves to
//! nothing — a dangling code is an omission wearing a reference. [`Representation`] gives those
//! three outcomes three different shapes, and [`ClosureSelection::check`] treats only the first
//! as legal.
//!
//! Silence is the fourth failure and the most common one: a class that the selection never
//! mentions is reported as [`ClosureViolation::Undeclared`], not passed over. A compiler that
//! genuinely has nothing to say about a class must say so through
//! [`Representation::DeclaredNotApplicable`] and give a reason.
//!
//! Not implemented here: any check that a stable code *resolves to the right thing*. This module
//! verifies that the compiler claims a resolvable reference; verifying the referent belongs to
//! the evidence graph and its resolver, not to a closure check.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The ten protected classes of blueprint 39.05, in the order the specification lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProtectedClass {
    /// Subject, lesion, region, specimen, aliquot, assay, artifact and model-system identity.
    Identity,
    /// Valid, collection, treatment, record, release and decision time.
    Time,
    /// Units, coordinate system, genome/reference build, orientation and scale.
    UnitsAndReference,
    /// Assay platform, protocol, reagent, preprocessing, model, ontology and classifier version.
    ProvenanceAndVersion,
    /// Cohort inclusion/exclusion, duplicate structure, split unit and censoring rule.
    CohortStructure,
    /// Uncertainty, quality, detection limit, missingness class and failure state.
    UncertaintyAndQuality,
    /// Contradictory and negative evidence, preserved as first-class state per 39.09.
    ContradictionAndNegativeEvidence,
    /// Privacy, consent/use restriction, data residency and role visibility.
    AccessAndConsent,
    /// The distinction between observation, interpretation, hypothesis, causal claim and
    /// recommendation.
    EpistemicKind,
    /// The counterfactual support boundary.
    CounterfactualBoundary,
}

impl ProtectedClass {
    pub const ALL: [ProtectedClass; 10] = [
        ProtectedClass::Identity,
        ProtectedClass::Time,
        ProtectedClass::UnitsAndReference,
        ProtectedClass::ProvenanceAndVersion,
        ProtectedClass::CohortStructure,
        ProtectedClass::UncertaintyAndQuality,
        ProtectedClass::ContradictionAndNegativeEvidence,
        ProtectedClass::AccessAndConsent,
        ProtectedClass::EpistemicKind,
        ProtectedClass::CounterfactualBoundary,
    ];

    /// The one-based position in the blueprint's numbered list, so a violation can be cited.
    pub fn blueprint_index(self) -> u8 {
        match self {
            ProtectedClass::Identity => 1,
            ProtectedClass::Time => 2,
            ProtectedClass::UnitsAndReference => 3,
            ProtectedClass::ProvenanceAndVersion => 4,
            ProtectedClass::CohortStructure => 5,
            ProtectedClass::UncertaintyAndQuality => 6,
            ProtectedClass::ContradictionAndNegativeEvidence => 7,
            ProtectedClass::AccessAndConsent => 8,
            ProtectedClass::EpistemicKind => 9,
            ProtectedClass::CounterfactualBoundary => 10,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ProtectedClass::Identity => "identity",
            ProtectedClass::Time => "time",
            ProtectedClass::UnitsAndReference => "units_and_reference",
            ProtectedClass::ProvenanceAndVersion => "provenance_and_version",
            ProtectedClass::CohortStructure => "cohort_structure",
            ProtectedClass::UncertaintyAndQuality => "uncertainty_and_quality",
            ProtectedClass::ContradictionAndNegativeEvidence => {
                "contradiction_and_negative_evidence"
            }
            ProtectedClass::AccessAndConsent => "access_and_consent",
            ProtectedClass::EpistemicKind => "epistemic_kind",
            ProtectedClass::CounterfactualBoundary => "counterfactual_boundary",
        }
    }

    pub fn parse(text: &str) -> Option<ProtectedClass> {
        ProtectedClass::ALL
            .into_iter()
            .find(|class| class.as_str() == text)
    }
}

/// How a selection renders one protected class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Representation {
    /// Rendered in full. Always legal, never cheap.
    Verbose,
    /// Replaced by a stable code or graph reference.
    ///
    /// Legal only while `resolvable` holds. The flag is the compiler's assertion that the code
    /// can be expanded back to the semantics through a handle the consumer actually has.
    StableCode { code: String, resolvable: bool },
    /// The compiler asserts the class has no bearing on this decision, and says why.
    DeclaredNotApplicable { reason: String },
    /// Dropped. Never legal for a protected class; present so the illegal act is representable
    /// and therefore detectable.
    Omitted,
}

impl Representation {
    pub fn code(code: impl Into<String>) -> Self {
        Representation::StableCode {
            code: code.into(),
            resolvable: true,
        }
    }

    pub fn dangling_code(code: impl Into<String>) -> Self {
        Representation::StableCode {
            code: code.into(),
            resolvable: false,
        }
    }

    pub fn not_applicable(reason: impl Into<String>) -> Self {
        Representation::DeclaredNotApplicable {
            reason: reason.into(),
        }
    }

    /// Whether the semantics survive this representation.
    pub fn preserves_semantics(&self) -> bool {
        match self {
            Representation::Verbose => true,
            Representation::StableCode { resolvable, .. } => *resolvable,
            Representation::DeclaredNotApplicable { reason } => !reason.trim().is_empty(),
            Representation::Omitted => false,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Representation::Verbose => "verbose",
            Representation::StableCode { .. } => "stable_code",
            Representation::DeclaredNotApplicable { .. } => "declared_not_applicable",
            Representation::Omitted => "omitted",
        }
    }
}

/// What a context projection did with each protected class.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureSelection {
    entries: BTreeMap<ProtectedClass, Representation>,
}

impl ClosureSelection {
    pub fn new() -> Self {
        ClosureSelection::default()
    }

    /// Declares a representation, returning the one it replaced.
    pub fn declare(
        &mut self,
        class: ProtectedClass,
        representation: Representation,
    ) -> Option<Representation> {
        self.entries.insert(class, representation)
    }

    pub fn get(&self, class: ProtectedClass) -> Option<&Representation> {
        self.entries.get(&class)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProtectedClass, &Representation)> {
        self.entries.iter()
    }

    /// Every class rendered in full. The starting point a compiler compresses away from.
    pub fn all_verbose() -> Self {
        let mut selection = ClosureSelection::new();
        for class in ProtectedClass::ALL {
            selection.declare(class, Representation::Verbose);
        }
        selection
    }

    /// Runs the closure constraint of 39.05 over the selection.
    pub fn check(&self) -> ClosureReport {
        let mut violations = Vec::new();
        for class in ProtectedClass::ALL {
            match self.entries.get(&class) {
                None => violations.push(ClosureViolation::Undeclared { class }),
                Some(Representation::Omitted) => {
                    violations.push(ClosureViolation::SemanticsOmitted { class })
                }
                Some(Representation::StableCode {
                    code,
                    resolvable: false,
                }) => violations.push(ClosureViolation::UnresolvableCode {
                    class,
                    code: code.clone(),
                }),
                Some(Representation::DeclaredNotApplicable { reason })
                    if reason.trim().is_empty() =>
                {
                    violations.push(ClosureViolation::UnreasonedInapplicability { class })
                }
                Some(_) => {}
            }
        }
        ClosureReport { violations }
    }
}

/// One way a projection broke the closure constraint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "violation", rename_all = "snake_case")]
pub enum ClosureViolation {
    /// The projection never mentioned the class. Silence is the default failure mode 39.05 is
    /// written against.
    Undeclared { class: ProtectedClass },
    /// The class was dropped outright.
    SemanticsOmitted { class: ProtectedClass },
    /// Compressed to a code the consumer cannot expand, which omits the semantics by another
    /// route.
    UnresolvableCode { class: ProtectedClass, code: String },
    /// Declared inapplicable without saying why, which is indistinguishable from dropping it.
    UnreasonedInapplicability { class: ProtectedClass },
}

impl ClosureViolation {
    pub fn class(&self) -> ProtectedClass {
        match self {
            ClosureViolation::Undeclared { class }
            | ClosureViolation::SemanticsOmitted { class }
            | ClosureViolation::UnresolvableCode { class, .. }
            | ClosureViolation::UnreasonedInapplicability { class } => *class,
        }
    }
}

/// The outcome of the closure check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureReport {
    pub violations: Vec<ClosureViolation>,
}

impl ClosureReport {
    pub fn is_satisfied(&self) -> bool {
        self.violations.is_empty()
    }

    pub fn violated_classes(&self) -> Vec<ProtectedClass> {
        let mut classes: Vec<ProtectedClass> = self
            .violations
            .iter()
            .map(ClosureViolation::class)
            .collect();
        classes.dedup();
        classes
    }
}
