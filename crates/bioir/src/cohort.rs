//! Cohorts, eligibility rules and split plans.
//!
//! Implements blueprint 25.13. Three of the blueprint's stated invariants drive everything
//! here: "a participant cannot cross protected splits", "repeated measures remain grouped when
//! required", and "exclusions are executable and counted".
//!
//! # The split unit is not the subject
//!
//! The common assumption — group by patient and you are safe — is wrong in both directions.
//! It is too weak for a site-held-out design, where two patients at the same scanner share a
//! batch effect that patient-level grouping happily splits. It is too strong for a cell-line
//! panel, where the unit really is the specimen. So [`SplitUnit`] is declared, not assumed.
//!
//! But the repeated-measures check is *not* conditional on the split unit. A subject who
//! contributed three biopsies contributes three correlated rows no matter which facet the
//! split keys on, and a site-held-out split will cheerfully separate them if that subject was
//! treated at two hospitals. [`SplitPlan::validate`] therefore runs the repeated-measures
//! check independently of the declared unit. That is the one case the "just group by patient"
//! reflex and the "we split by site" reflex both miss.
//!
//! # Undecidable eligibility
//!
//! 25.13 requires a `missingness` field and says nothing about how a rule behaves when the
//! attribute it tests is absent. Guessing would silently move records between arms, so
//! [`Predicate::evaluate`] is three-valued ([`Truth`]) and any record whose eligibility is
//! [`Truth::Unknown`] under any rule lands in [`CohortAssembly::undecidable`] rather than in
//! the cohort. Refusing to classify is recoverable; a wrong classification that reconciles is
//! not.
//!
//! # Not implemented
//!
//! 25.13 lists Estimand as a primary object and gives it one table row. [`Estimand`] here
//! records the target, unit, population and contrast as text and checks exactly one thing —
//! that its unit of analysis matches the cohort's. Estimand algebra (identification,
//! transportability, censoring semantics) is section 43's causal machinery, not this IR's.

use crate::error::CohortError;
use crate::ids::{CohortId, ObservationId, SpecimenId, SubjectId};
use crate::lineage::LineageGraph;
use bioprism_scope::{Interval, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use thiserror::Error;

/// A named arm of a split: "train", "test", "fold-3", "site-held-out".
///
/// Fold names are opaque. Nothing here assumes "train" and "test" are the only two, because
/// cross-validation, nested tuning splits and prospective holdouts all use more.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Fold(pub String);

impl Fold {
    pub fn new(name: impl Into<String>) -> Self {
        Fold(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Fold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// What one row of the cohort *is*.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "unit", rename_all = "snake_case")]
pub enum UnitOfAnalysis {
    Subject,
    Specimen,
    Lesion,
    /// One measurement occasion; the unit under which repeated measures are rows, not columns.
    Observation,
    Custom { label: String },
}

impl fmt::Display for UnitOfAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnitOfAnalysis::Subject => f.write_str("subject"),
            UnitOfAnalysis::Specimen => f.write_str("specimen"),
            UnitOfAnalysis::Lesion => f.write_str("lesion"),
            UnitOfAnalysis::Observation => f.write_str("observation"),
            UnitOfAnalysis::Custom { label } => write!(f, "custom({label})"),
        }
    }
}

/// The facet a split keys on, or that a grouping requires to stay intact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "facet", rename_all = "snake_case")]
pub enum SplitUnit {
    Subject,
    Specimen,
    Site,
    Observation,
    /// Any declared record attribute: a batch id, a scanner, a collection year.
    Attribute { key: String },
}

impl SplitUnit {
    /// The value that identifies which group a record belongs to under this facet.
    ///
    /// `None` means the record cannot be keyed at all — an observation with no specimen under
    /// a specimen split, or a missing attribute. That is reported rather than skipped: a
    /// record that cannot be keyed cannot be shown not to leak.
    pub fn key_of(&self, observation: &Observation) -> Option<String> {
        match self {
            SplitUnit::Subject => Some(observation.subject.to_string()),
            SplitUnit::Specimen => observation.specimen.as_ref().map(SpecimenId::to_string),
            SplitUnit::Site => Some(observation.site.clone()),
            SplitUnit::Observation => Some(observation.id.to_string()),
            SplitUnit::Attribute { key } => observation.attributes.get(key).map(scalar_string),
        }
    }

    /// Position in the only coarseness chain the blueprint implies: observation, specimen,
    /// subject. `None` for facets outside it.
    ///
    /// Site is deliberately unranked. A site is not coarser or finer than a subject — subjects
    /// move between sites and sites hold many subjects — so no static comparison is sound and
    /// the data-level checks in [`SplitPlan::validate`] must do that work.
    fn coarseness(&self) -> Option<u8> {
        match self {
            SplitUnit::Observation => Some(0),
            SplitUnit::Specimen => Some(1),
            SplitUnit::Subject => Some(2),
            SplitUnit::Site | SplitUnit::Attribute { .. } => None,
        }
    }
}

impl fmt::Display for SplitUnit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SplitUnit::Subject => f.write_str("subject"),
            SplitUnit::Specimen => f.write_str("specimen"),
            SplitUnit::Site => f.write_str("site"),
            SplitUnit::Observation => f.write_str("observation"),
            SplitUnit::Attribute { key } => write!(f, "attribute({key})"),
        }
    }
}

/// Whether one subject may contribute several rows, and what that obliges a split to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "repeated_measures", rename_all = "snake_case")]
pub enum RepeatedMeasures {
    /// The frame is asserted to hold at most one row per subject. Violations are reported.
    AtMostOnePerSubject,
    /// Repeated measures exist and must never be separated by a split.
    GroupedBySubject,
    /// The author asserts repeated rows of one subject are independent.
    ///
    /// This is honoured, because 25.13 says repeated measures stay grouped "when required" and
    /// therefore admits designs where they are not. It is not hidden: a split that separates
    /// them still yields [`LeakageFinding::RepeatedMeasuresDeclaredIndependent`], because
    /// 39.05 protects "duplicate structure" as a class that may be re-encoded but not omitted.
    Independent,
}

/// The facets a split must not break, beyond its own unit.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupingKey {
    pub facets: BTreeSet<SplitUnit>,
    pub repeated_measures_of: Option<RepeatedMeasures>,
}

impl GroupingKey {
    pub fn by(facets: impl IntoIterator<Item = SplitUnit>) -> Self {
        GroupingKey {
            facets: facets.into_iter().collect(),
            repeated_measures_of: None,
        }
    }

    pub fn with_repeated_measures(mut self, policy: RepeatedMeasures) -> Self {
        self.repeated_measures_of = Some(policy);
        self
    }

    fn repeated_measures(&self) -> &RepeatedMeasures {
        self.repeated_measures_of
            .as_ref()
            .unwrap_or(&RepeatedMeasures::GroupedBySubject)
    }
}

/// What the index date on each row anchors to, and how long outcomes are followed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeAnchor {
    /// The event the index date marks: "diagnosis", "first dose", "resection".
    pub event: String,
    pub horizon_days: Option<u64>,
    /// How incomplete follow-up is handled, in the source protocol's words.
    pub censoring_rule: String,
}

/// One row of the candidate frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub subject: SubjectId,
    /// Present when the row is backed by physical material, which is what makes the
    /// lineage-aware leakage check in [`SplitPlan::validate`] possible.
    pub specimen: Option<SpecimenId>,
    pub site: String,
    pub index_date: Timestamp,
    pub attributes: BTreeMap<String, Value>,
}

impl Observation {
    pub fn new(
        id: ObservationId,
        subject: SubjectId,
        site: impl Into<String>,
        index_date: Timestamp,
    ) -> Self {
        Observation {
            id,
            subject,
            specimen: None,
            site: site.into(),
            index_date,
            attributes: BTreeMap::new(),
        }
    }

    pub fn with_specimen(mut self, specimen: SpecimenId) -> Self {
        self.specimen = Some(specimen);
        self
    }

    pub fn with_attribute(mut self, key: impl Into<String>, value: Value) -> Self {
        self.attributes.insert(key.into(), value);
        self
    }
}

/// Three-valued truth, because "the attribute is absent" is not "the attribute is false".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Truth {
    True,
    False,
    Unknown,
}

impl Truth {
    pub fn negate(self) -> Truth {
        match self {
            Truth::True => Truth::False,
            Truth::False => Truth::True,
            Truth::Unknown => Truth::Unknown,
        }
    }

    fn of(value: bool) -> Truth {
        if value {
            Truth::True
        } else {
            Truth::False
        }
    }
}

/// An executable eligibility test over one row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "predicate", rename_all = "snake_case")]
pub enum Predicate {
    AttributeEquals { key: String, value: Value },
    AttributeIn { key: String, values: BTreeSet<String> },
    AttributeAtLeast { key: String, threshold: f64 },
    /// The only predicate that decides on missingness itself rather than inheriting it.
    AttributePresent { key: String },
    IndexDateWithin { window: Interval },
    Not { inner: Box<Predicate> },
    All { of: Vec<Predicate> },
    Any { of: Vec<Predicate> },
}

impl Predicate {
    /// Kleene evaluation: unknown propagates unless a definite operand settles the result.
    pub fn evaluate(&self, observation: &Observation) -> Truth {
        match self {
            Predicate::AttributePresent { key } => {
                Truth::of(observation.attributes.contains_key(key))
            }
            Predicate::AttributeEquals { key, value } => match observation.attributes.get(key) {
                Some(found) => Truth::of(found == value),
                None => Truth::Unknown,
            },
            Predicate::AttributeIn { key, values } => match observation.attributes.get(key) {
                Some(found) => Truth::of(values.contains(&scalar_string(found))),
                None => Truth::Unknown,
            },
            Predicate::AttributeAtLeast { key, threshold } => {
                match observation.attributes.get(key).and_then(Value::as_f64) {
                    Some(found) => Truth::of(found >= *threshold),
                    None => Truth::Unknown,
                }
            }
            Predicate::IndexDateWithin { window } => {
                Truth::of(window.contains(observation.index_date))
            }
            Predicate::Not { inner } => inner.evaluate(observation).negate(),
            Predicate::All { of } => {
                let mut result = Truth::True;
                for predicate in of {
                    match predicate.evaluate(observation) {
                        Truth::False => return Truth::False,
                        Truth::Unknown => result = Truth::Unknown,
                        Truth::True => {}
                    }
                }
                result
            }
            Predicate::Any { of } => {
                let mut result = Truth::False;
                for predicate in of {
                    match predicate.evaluate(observation) {
                        Truth::True => return Truth::True,
                        Truth::Unknown => result = Truth::Unknown,
                        Truth::False => {}
                    }
                }
                result
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleEffect {
    Include,
    Exclude,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EligibilityRule {
    pub id: String,
    pub effect: RuleEffect,
    pub predicate: Predicate,
    /// Why the rule exists, in the protocol's words. Carried so a count can be defended.
    pub rationale: String,
}

impl EligibilityRule {
    pub fn include(id: impl Into<String>, predicate: Predicate) -> Self {
        EligibilityRule {
            id: id.into(),
            effect: RuleEffect::Include,
            predicate,
            rationale: String::new(),
        }
    }

    pub fn exclude(id: impl Into<String>, predicate: Predicate) -> Self {
        EligibilityRule {
            id: id.into(),
            effect: RuleEffect::Exclude,
            predicate,
            rationale: String::new(),
        }
    }

    pub fn because(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }
}

/// The quantity the cohort exists to estimate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Estimand {
    pub target: String,
    pub unit: UnitOfAnalysis,
    pub population: String,
    pub contrast: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CohortDefinition {
    pub id: CohortId,
    pub population: String,
    pub source_datasets: Vec<String>,
    pub rules: Vec<EligibilityRule>,
    pub time_anchor: TimeAnchor,
    pub unit: UnitOfAnalysis,
    pub grouping: GroupingKey,
    pub estimand: Estimand,
}

impl CohortDefinition {
    /// Declaration-level checks that need no data.
    pub fn validate(&self) -> Result<(), CohortError> {
        if self.rules.is_empty() {
            return Err(CohortError::NoRules {
                cohort: self.id.to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for rule in &self.rules {
            if !seen.insert(rule.id.clone()) {
                return Err(CohortError::DuplicateRule {
                    cohort: self.id.to_string(),
                    rule: rule.id.clone(),
                });
            }
        }
        if self.estimand.unit != self.unit {
            return Err(CohortError::EstimandUnitMismatch {
                cohort: self.id.to_string(),
                cohort_unit: self.unit.to_string(),
                estimand_unit: self.estimand.unit.to_string(),
            });
        }
        Ok(())
    }

    /// Runs the rules over a candidate frame.
    ///
    /// Attribution of an excluded record is to the *first* exclusion rule in declaration order
    /// that fires. 25.13 requires "count reconciliation" without saying whether a record
    /// excluded twice is counted twice; first-match is chosen so the counts partition the
    /// frame and [`CohortAssembly::reconciles`] can be a real check rather than an inequality.
    pub fn assemble(&self, frame: &[Observation]) -> Result<CohortAssembly, CohortError> {
        self.validate()?;
        let mut seen = BTreeSet::new();
        for observation in frame {
            if !seen.insert(observation.id.clone()) {
                return Err(CohortError::DuplicateObservation {
                    observation: observation.id.to_string(),
                });
            }
        }

        let mut assembly = CohortAssembly {
            cohort: self.id.clone(),
            screened: frame.len(),
            included: Vec::new(),
            excluded: BTreeMap::new(),
            undecidable: BTreeMap::new(),
        };

        'record: for observation in frame {
            for rule in &self.rules {
                let verdict = rule.predicate.evaluate(observation);
                match (rule.effect, verdict) {
                    (_, Truth::Unknown) => {
                        assembly
                            .undecidable
                            .insert(observation.id.clone(), rule.id.clone());
                        continue 'record;
                    }
                    (RuleEffect::Exclude, Truth::True) | (RuleEffect::Include, Truth::False) => {
                        assembly
                            .excluded
                            .entry(rule.id.clone())
                            .or_default()
                            .push(observation.id.clone());
                        continue 'record;
                    }
                    _ => {}
                }
            }
            assembly.included.push(observation.id.clone());
        }
        Ok(assembly)
    }
}

/// The executed result of running eligibility rules over a frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CohortAssembly {
    pub cohort: CohortId,
    pub screened: usize,
    pub included: Vec<ObservationId>,
    /// Rule id to the records it removed, first-match attributed.
    pub excluded: BTreeMap<String, Vec<ObservationId>>,
    /// Records whose eligibility could not be decided, and the rule that went unknown.
    pub undecidable: BTreeMap<ObservationId, String>,
}

impl CohortAssembly {
    pub fn exclusion_counts(&self) -> BTreeMap<String, usize> {
        self.excluded
            .iter()
            .map(|(rule, records)| (rule.clone(), records.len()))
            .collect()
    }

    pub fn excluded_total(&self) -> usize {
        self.excluded.values().map(Vec::len).sum()
    }

    /// Every screened record is accounted for exactly once.
    pub fn reconciles(&self) -> bool {
        self.included.len() + self.excluded_total() + self.undecidable.len() == self.screened
    }

    pub fn contains(&self, observation: &ObservationId) -> bool {
        self.included.contains(observation)
    }
}

/// An assignment of cohort members to folds, plus any chronological boundary it claims.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SplitPlan {
    pub unit: SplitUnit,
    pub assignments: BTreeMap<ObservationId, Fold>,
    /// Optional claim that every record in `earlier` predates every record in `later`.
    ///
    /// 25.13 lists "time-split tests" under validation. A temporal split is the one design
    /// where fold membership is not arbitrary, so the boundary is declared and checked rather
    /// than inferred from the assignment.
    pub chronological_boundary: Option<ChronologicalBoundary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChronologicalBoundary {
    pub earlier: Fold,
    pub later: Fold,
    /// Records in `earlier` must have index dates strictly before this instant, and records in
    /// `later` at or after it.
    pub at: Timestamp,
}

impl SplitPlan {
    pub fn new(unit: SplitUnit) -> Self {
        SplitPlan {
            unit,
            assignments: BTreeMap::new(),
            chronological_boundary: None,
        }
    }

    pub fn assign(mut self, observation: ObservationId, fold: impl Into<String>) -> Self {
        self.assignments.insert(observation, Fold::new(fold));
        self
    }

    pub fn with_boundary(mut self, boundary: ChronologicalBoundary) -> Self {
        self.chronological_boundary = Some(boundary);
        self
    }

    /// Declaration-level check that the split unit can honour the declared grouping.
    ///
    /// Only the observation/specimen/subject chain is comparable. A split by site with a
    /// subject grouping is not rejected here because it is not statically wrong — it is wrong
    /// only if some subject actually appears at two sites, which [`SplitPlan::validate`]
    /// discovers from the data.
    pub fn validate_declaration(&self, cohort: &CohortDefinition) -> Result<(), CohortError> {
        let Some(split_rank) = self.unit.coarseness() else {
            return Ok(());
        };
        for facet in &cohort.grouping.facets {
            let Some(facet_rank) = facet.coarseness() else {
                continue;
            };
            if split_rank < facet_rank {
                return Err(CohortError::SplitUnitFinerThanGrouping {
                    cohort: cohort.id.to_string(),
                    split_unit: self.unit.to_string(),
                    grouping: facet.to_string(),
                });
            }
        }
        Ok(())
    }

    /// Every way this assignment lets information cross a fold boundary.
    ///
    /// `lineage` is optional. Without it the shared-material check cannot run, and its absence
    /// is reported as [`LeakageFinding::LineageUnavailable`] rather than passed over, because a
    /// silent skip reads exactly like a clean result.
    pub fn validate(
        &self,
        cohort: &CohortDefinition,
        assembly: &CohortAssembly,
        frame: &[Observation],
        lineage: Option<&LineageGraph>,
    ) -> Vec<LeakageFinding> {
        let mut findings = Vec::new();
        let by_id: BTreeMap<&ObservationId, &Observation> =
            frame.iter().map(|record| (&record.id, record)).collect();
        let members: Vec<&Observation> = assembly
            .included
            .iter()
            .filter_map(|id| by_id.get(id).copied())
            .collect();

        self.check_coverage(assembly, &members, &mut findings);
        self.check_facet(&self.unit, &members, &mut findings);
        for facet in &cohort.grouping.facets {
            if facet != &self.unit {
                self.check_facet(facet, &members, &mut findings);
            }
        }
        self.check_repeated_measures(cohort, &members, &mut findings);
        self.check_chronology(&members, &mut findings);
        self.check_material(&members, lineage, &mut findings);
        findings
    }

    fn fold_of(&self, observation: &ObservationId) -> Option<&Fold> {
        self.assignments.get(observation)
    }

    /// The fold an observation was assigned to, or a typed failure naming it.
    pub fn fold_for(&self, observation: &ObservationId) -> Result<&Fold, CohortError> {
        self.assignments
            .get(observation)
            .ok_or_else(|| CohortError::UnknownObservation {
                observation: observation.to_string(),
            })
    }

    fn check_coverage(
        &self,
        assembly: &CohortAssembly,
        members: &[&Observation],
        findings: &mut Vec<LeakageFinding>,
    ) {
        for record in members {
            if self.fold_of(&record.id).is_none() {
                findings.push(LeakageFinding::UnassignedObservation {
                    observation: record.id.clone(),
                });
            }
        }
        for observation in self.assignments.keys() {
            if !assembly.contains(observation) {
                findings.push(LeakageFinding::AssignmentOutsideCohort {
                    observation: observation.clone(),
                });
            }
        }
    }

    /// All records sharing a facet key must land in one fold.
    fn check_facet(
        &self,
        facet: &SplitUnit,
        members: &[&Observation],
        findings: &mut Vec<LeakageFinding>,
    ) {
        let mut folds_by_key: BTreeMap<String, BTreeSet<Fold>> = BTreeMap::new();
        for record in members {
            let Some(key) = facet.key_of(record) else {
                findings.push(LeakageFinding::UnkeyableObservation {
                    observation: record.id.clone(),
                    facet: facet.clone(),
                });
                continue;
            };
            if let Some(fold) = self.fold_of(&record.id) {
                folds_by_key.entry(key).or_default().insert(fold.clone());
            }
        }
        for (key, folds) in folds_by_key {
            if folds.len() > 1 {
                findings.push(LeakageFinding::GroupSeparated {
                    facet: facet.clone(),
                    key,
                    folds,
                });
            }
        }
    }

    /// Repeated measures of one subject, checked whatever the split unit is.
    fn check_repeated_measures(
        &self,
        cohort: &CohortDefinition,
        members: &[&Observation],
        findings: &mut Vec<LeakageFinding>,
    ) {
        let mut by_subject: BTreeMap<SubjectId, BTreeMap<ObservationId, Option<Fold>>> =
            BTreeMap::new();
        for record in members {
            by_subject.entry(record.subject.clone()).or_default().insert(
                record.id.clone(),
                self.fold_of(&record.id).cloned(),
            );
        }
        let policy = cohort.grouping.repeated_measures();
        for (subject, records) in by_subject {
            if records.len() < 2 {
                continue;
            }
            if policy == &RepeatedMeasures::AtMostOnePerSubject {
                findings.push(LeakageFinding::UndeclaredRepeatedMeasures {
                    subject: subject.clone(),
                    observations: records.len(),
                });
            }
            let folds: BTreeSet<Fold> = records.values().flatten().cloned().collect();
            if folds.len() < 2 {
                continue;
            }
            findings.push(match policy {
                RepeatedMeasures::Independent => {
                    LeakageFinding::RepeatedMeasuresDeclaredIndependent {
                        subject,
                        observations: records.len(),
                        folds,
                    }
                }
                _ => LeakageFinding::RepeatedMeasuresSeparated {
                    subject,
                    observations: records.len(),
                    folds,
                },
            });
        }
    }

    fn check_chronology(&self, members: &[&Observation], findings: &mut Vec<LeakageFinding>) {
        let Some(boundary) = &self.chronological_boundary else {
            return;
        };
        for record in members {
            let Some(fold) = self.fold_of(&record.id) else {
                continue;
            };
            let violates = (fold == &boundary.earlier && record.index_date >= boundary.at)
                || (fold == &boundary.later && record.index_date < boundary.at);
            if violates {
                findings.push(LeakageFinding::ChronologicalBoundaryViolated {
                    observation: record.id.clone(),
                    fold: fold.clone(),
                    index_date: record.index_date,
                    boundary: boundary.at,
                });
            }
        }
    }

    /// Records in different folds whose material traces to a common ancestor.
    ///
    /// This catches what subject-level grouping cannot: two rows recorded under different
    /// subject identifiers whose specimens are aliquots of the same block.
    fn check_material(
        &self,
        members: &[&Observation],
        lineage: Option<&LineageGraph>,
        findings: &mut Vec<LeakageFinding>,
    ) {
        let backed: Vec<(&ObservationId, &SpecimenId)> = members
            .iter()
            .filter_map(|record| record.specimen.as_ref().map(|s| (&record.id, s)))
            .collect();
        let Some(graph) = lineage else {
            if !backed.is_empty() {
                findings.push(LeakageFinding::LineageUnavailable {
                    material_backed_observations: backed.len(),
                });
            }
            return;
        };
        for (index, (left_id, left_specimen)) in backed.iter().enumerate() {
            for (right_id, right_specimen) in backed.iter().skip(index + 1) {
                let (Some(left_fold), Some(right_fold)) =
                    (self.fold_of(left_id), self.fold_of(right_id))
                else {
                    continue;
                };
                if left_fold == right_fold {
                    continue;
                }
                let Ok(Some(ancestor)) =
                    graph.nearest_shared_ancestor(left_specimen, right_specimen)
                else {
                    continue;
                };
                findings.push(LeakageFinding::SharedMaterialAcrossFolds {
                    left: (*left_id).clone(),
                    right: (*right_id).clone(),
                    ancestor,
                    folds: [left_fold.clone(), right_fold.clone()].into_iter().collect(),
                });
            }
        }
    }
}

/// A way information crosses a fold boundary, or a reason that cannot be ruled out.
///
/// Returned in bulk: a split plan with three problems should report three, and a caller that
/// fixed only the first would resubmit a plan that still leaks.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LeakageFinding {
    #[error("cohort member {observation} is assigned to no fold")]
    UnassignedObservation { observation: ObservationId },

    #[error("observation {observation} is assigned to a fold but is not in the cohort")]
    AssignmentOutsideCohort { observation: ObservationId },

    #[error("observation {observation} has no value for facet {facet}, so its fold cannot be checked")]
    UnkeyableObservation {
        observation: ObservationId,
        facet: SplitUnit,
    },

    #[error("facet {facet} value {key:?} spans {} folds", folds.len())]
    GroupSeparated {
        facet: SplitUnit,
        key: String,
        folds: BTreeSet<Fold>,
    },

    #[error("subject {subject} contributes {observations} rows split across {} folds", folds.len())]
    RepeatedMeasuresSeparated {
        subject: SubjectId,
        observations: usize,
        folds: BTreeSet<Fold>,
    },

    #[error("subject {subject} contributes {observations} rows split across {} folds under a declared-independent policy", folds.len())]
    RepeatedMeasuresDeclaredIndependent {
        subject: SubjectId,
        observations: usize,
        folds: BTreeSet<Fold>,
    },

    #[error("subject {subject} contributes {observations} rows but the cohort declares at most one per subject")]
    UndeclaredRepeatedMeasures {
        subject: SubjectId,
        observations: usize,
    },

    #[error("observation {observation} in fold {fold} has index date {index_date}, on the wrong side of boundary {boundary}")]
    ChronologicalBoundaryViolated {
        observation: ObservationId,
        fold: Fold,
        index_date: Timestamp,
        boundary: Timestamp,
    },

    #[error("observations {left} and {right} are in different folds but their material shares ancestor {ancestor}")]
    SharedMaterialAcrossFolds {
        left: ObservationId,
        right: ObservationId,
        ancestor: SpecimenId,
        folds: BTreeSet<Fold>,
    },

    #[error("{material_backed_observations} cohort rows are backed by material but no lineage graph was supplied, so shared-material leakage was not checked")]
    LineageUnavailable { material_backed_observations: usize },
}

/// Renders a JSON scalar the way a split key or a set membership test needs it.
///
/// Non-scalars fall back to their JSON text: a record keyed on a nested object is unusual, and
/// a stable string is more useful than dropping the record silently.
fn scalar_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}
