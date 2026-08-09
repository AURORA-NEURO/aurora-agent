//! Disease scope, taxonomy and integrated molecular classification.
//!
//! Blueprint 30.01 (disease scope and taxonomy) and 30.10 (integrated molecular
//! classification).
//!
//! # What the blueprint fixes, and what this module invents
//!
//! 30.01 and 30.10 are architecture specifications. They fix the *shape* of classification and
//! say nothing about its content: there is no entity list, no marker list, no ontology version
//! and no threshold anywhere in either module. What they do fix, and what this module
//! implements, is:
//!
//! * the four diagnostic-state qualifiers of 30.01 — *unresolved, provisional, mixed,
//!   not-otherwise-resolved* — as first-class outcomes rather than as an error path;
//! * the three evidence roles of 30.10 — *required, supportive, exclusionary*;
//! * the notion of an **unresolved obligation**: 30.10 states that the correct output under
//!   ambiguity is "a structured unresolved state and a prioritized evidence request, not an
//!   invented certainty";
//! * immutability of the source diagnosis (30.01), with reclassification producing a new
//!   record that cites the original rather than overwriting it.
//!
//! The concrete entities and markers below — the adult-type diffuse glioma set and its defining
//! alterations — are **not from the blueprint**. They are a worked instantiation chosen because
//! they exercise every branch of the machinery, and they are deliberately incomplete: there is
//! no ependymal, embryonal, meningothelial or paediatric criteria table here, and no methylation
//! classifier. A production instantiation would load a versioned criteria table rather than
//! compile one in.
//!
//! # Not implemented
//!
//! Histologic grading below grade 4 (mitotic activity, necrosis, microvascular proliferation),
//! methylation-class calling, tumour purity correction, fusion evidence, and the
//! ontology-version robustness testing 30.01 asks for. Where grade depends on unimplemented
//! histology this module returns [`Observed::Unobserved`] rather than a guess.

use crate::status::{ObservationStatus, Observed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Broad morphologic family, as read from the specimen.
///
/// Histology alone never determines an integrated entity: it selects which criteria tables are
/// in play. That is the whole point of 30.10 — classification is a *conjunction*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Histology {
    /// Diffusely infiltrating glial morphology, adult type.
    DiffuseGlioma,
    /// Any morphology for which this instantiation carries no criteria table.
    OutsideImplementedScope,
}

/// A molecular alteration used as classification evidence.
///
/// Invented for this instantiation; see the module note.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MolecularMarker {
    IdhMutation,
    Codeletion1p19q,
    TertPromoterMutation,
    EgfrAmplification,
    Chromosome7Gain10Loss,
    Cdkn2aCdkn2bHomozygousDeletion,
    H3K27Alteration,
    H3G34Mutation,
}

impl MolecularMarker {
    pub const fn describe(self) -> &'static str {
        match self {
            MolecularMarker::IdhMutation => "IDH1/2 mutation",
            MolecularMarker::Codeletion1p19q => "1p/19q whole-arm codeletion",
            MolecularMarker::TertPromoterMutation => "TERT promoter mutation",
            MolecularMarker::EgfrAmplification => "EGFR amplification",
            MolecularMarker::Chromosome7Gain10Loss => "combined chromosome 7 gain and 10 loss",
            MolecularMarker::Cdkn2aCdkn2bHomozygousDeletion => "CDKN2A/B homozygous deletion",
            MolecularMarker::H3K27Alteration => "H3 K27 alteration",
            MolecularMarker::H3G34Mutation => "H3 G34 mutation",
        }
    }
}

/// The two outcomes an assay can *report*.
///
/// Note what is absent: there is no `Unknown` variant. Not knowing is represented by
/// [`Observed::Unobserved`] carrying an [`ObservationStatus`], which keeps "the assay was never
/// run" structurally distinct from "the assay ran and was negative" (30.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkerCall {
    Present,
    Absent,
}

/// A marker call that may not have been made.
pub type MarkerObservation = Observed<MarkerCall>;

/// The role a piece of evidence plays for one entity (30.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    /// Must be observed with the stated call. An unobserved required marker is an obligation.
    Required,
    /// Contributes toward a minimum count; individually neither necessary nor sufficient.
    Supportive,
    /// Observing the stated call rules the entity out.
    Exclusionary,
}

/// A CNS entity in this instantiation's criteria table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CnsEntity {
    AstrocytomaIdhMutant,
    OligodendrogliomaIdhMutant1p19qCodeleted,
    GlioblastomaIdhWildtype,
    DiffuseMidlineGliomaH3K27Altered,
    DiffuseHemisphericGliomaH3G34Mutant,
}

/// CNS grade. Only grade 4 is derivable from the molecular evidence modelled here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CnsGrade {
    Two,
    Three,
    Four,
}

/// The compiled criteria for one entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityCriteria {
    pub entity: CnsEntity,
    pub histology: Histology,
    pub required: &'static [(MolecularMarker, MarkerCall)],
    pub exclusionary: &'static [(MolecularMarker, MarkerCall)],
    pub supportive: &'static [(MolecularMarker, MarkerCall)],
    /// How many supportive markers must be observed with their stated call.
    pub min_supportive: usize,
}

/// The criteria table for this instantiation.
///
/// Compiled in rather than loaded, which is the main reason this module is a worked example and
/// not a product: 30.01 requires ontology-version pinning and 30.10 requires versioned
/// classification rules, and a `const` table can carry neither.
pub const CRITERIA: &[EntityCriteria] = &[
    EntityCriteria {
        entity: CnsEntity::AstrocytomaIdhMutant,
        histology: Histology::DiffuseGlioma,
        required: &[
            (MolecularMarker::IdhMutation, MarkerCall::Present),
            (MolecularMarker::Codeletion1p19q, MarkerCall::Absent),
        ],
        exclusionary: &[
            (MolecularMarker::H3K27Alteration, MarkerCall::Present),
            (MolecularMarker::H3G34Mutation, MarkerCall::Present),
        ],
        supportive: &[],
        min_supportive: 0,
    },
    EntityCriteria {
        entity: CnsEntity::OligodendrogliomaIdhMutant1p19qCodeleted,
        histology: Histology::DiffuseGlioma,
        required: &[
            (MolecularMarker::IdhMutation, MarkerCall::Present),
            (MolecularMarker::Codeletion1p19q, MarkerCall::Present),
        ],
        exclusionary: &[],
        supportive: &[],
        min_supportive: 0,
    },
    EntityCriteria {
        entity: CnsEntity::GlioblastomaIdhWildtype,
        histology: Histology::DiffuseGlioma,
        required: &[(MolecularMarker::IdhMutation, MarkerCall::Absent)],
        exclusionary: &[
            (MolecularMarker::H3K27Alteration, MarkerCall::Present),
            (MolecularMarker::H3G34Mutation, MarkerCall::Present),
        ],
        supportive: &[
            (MolecularMarker::TertPromoterMutation, MarkerCall::Present),
            (MolecularMarker::EgfrAmplification, MarkerCall::Present),
            (MolecularMarker::Chromosome7Gain10Loss, MarkerCall::Present),
        ],
        min_supportive: 1,
    },
    EntityCriteria {
        entity: CnsEntity::DiffuseMidlineGliomaH3K27Altered,
        histology: Histology::DiffuseGlioma,
        required: &[(MolecularMarker::H3K27Alteration, MarkerCall::Present)],
        exclusionary: &[],
        supportive: &[],
        min_supportive: 0,
    },
    EntityCriteria {
        entity: CnsEntity::DiffuseHemisphericGliomaH3G34Mutant,
        histology: Histology::DiffuseGlioma,
        required: &[(MolecularMarker::H3G34Mutation, MarkerCall::Present)],
        exclusionary: &[],
        supportive: &[],
        min_supportive: 0,
    },
];

/// The molecular evidence available for one specimen.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerPanel {
    calls: BTreeMap<MolecularMarker, MarkerObservation>,
}

impl MarkerPanel {
    /// A panel in which nothing has been collected.
    ///
    /// The name is chosen over `new` or `empty` because an empty panel is not neutral: it is a
    /// positive statement that no molecular evidence exists, which is what makes classification
    /// from histology alone unresolved rather than merely uncertain.
    pub fn nothing_collected() -> Self {
        MarkerPanel::default()
    }

    #[must_use]
    pub fn observed(mut self, marker: MolecularMarker, call: MarkerCall) -> Self {
        self.calls.insert(marker, Observed::Value(call));
        self
    }

    #[must_use]
    pub fn unobserved(mut self, marker: MolecularMarker, status: ObservationStatus) -> Self {
        self.calls.insert(marker, Observed::Unobserved(status));
        self
    }

    /// The state of one marker.
    ///
    /// A marker absent from the panel reports [`ObservationStatus::NotCollected`], never
    /// [`MarkerCall::Absent`]. This is the single most consequential line in the module: the
    /// opposite default silently converts every unrun assay into a wild-type call.
    pub fn state(&self, marker: MolecularMarker) -> MarkerObservation {
        self.calls
            .get(&marker)
            .copied()
            .unwrap_or(Observed::Unobserved(ObservationStatus::NotCollected))
    }

    pub fn iter(&self) -> impl Iterator<Item = (MolecularMarker, MarkerObservation)> + '_ {
        self.calls.iter().map(|(marker, state)| (*marker, *state))
    }
}

/// An unmet evidence requirement carried by an unresolved classification (30.10).
///
/// This is the "prioritized evidence request" the blueprint asks for: it names the assay to run,
/// why it matters, and how many candidate entities it would discriminate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObligation {
    pub marker: MolecularMarker,
    pub role: EvidenceRole,
    /// Current state of the marker, so the caller can tell "never run" from "assay failed" and
    /// route the request to the right process.
    pub state: MarkerObservation,
    /// How many candidate entities this marker currently discriminates. Higher is more urgent.
    pub discriminates: usize,
}

/// Evidence that was satisfied for the resolved entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SatisfiedEvidence {
    pub marker: MolecularMarker,
    pub role: EvidenceRole,
    pub call: MarkerCall,
}

/// The outcome of integrated classification.
///
/// The four non-integrated variants are the diagnostic-state qualifiers 30.01 names verbatim:
/// unresolved, provisional, mixed and not-otherwise-resolved. They are outcomes, not errors, and
/// there is deliberately no method that turns any of them into an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "resolution")]
pub enum DiagnosticResolution {
    /// Exactly one entity's required criteria are met with observed evidence.
    Integrated {
        entity: CnsEntity,
        grade: Observed<CnsGrade>,
        evidence: Vec<SatisfiedEvidence>,
    },
    /// One candidate survives, but its criteria are not fully observed.
    ///
    /// A provisional state names the candidate. It is not a diagnosis, and the obligations say
    /// exactly what would settle it.
    Provisional {
        candidate: CnsEntity,
        obligations: Vec<EvidenceObligation>,
    },
    /// Several candidates survive and none is fully observed.
    Unresolved {
        candidates: BTreeSet<CnsEntity>,
        obligations: Vec<EvidenceObligation>,
    },
    /// Several candidates are *fully satisfied*, which means the criteria table is inconsistent
    /// with the evidence rather than that the tumour is ambiguous.
    ///
    /// 30.10 requires discordance detection; surfacing it as a distinct state is what makes it
    /// detectable instead of being resolved by whichever entity the iteration order reached
    /// first.
    Mixed { candidates: BTreeSet<CnsEntity> },
    /// Evidence excludes every candidate the histology admits.
    NotOtherwiseResolved {
        histology: Histology,
        excluded: BTreeSet<CnsEntity>,
    },
}

impl DiagnosticResolution {
    /// The entity, when and only when one was integrated.
    ///
    /// There is no fallback, no "best guess" and no `Option::unwrap_or` target, because 30.10
    /// names forcing a definitive entity under incomplete criteria as a characteristic failure.
    pub const fn entity(&self) -> Option<CnsEntity> {
        match self {
            DiagnosticResolution::Integrated { entity, .. } => Some(*entity),
            _ => None,
        }
    }

    pub const fn is_integrated(&self) -> bool {
        matches!(self, DiagnosticResolution::Integrated { .. })
    }

    /// The outstanding evidence request, ordered most discriminating first.
    pub fn obligations(&self) -> &[EvidenceObligation] {
        match self {
            DiagnosticResolution::Provisional { obligations, .. }
            | DiagnosticResolution::Unresolved { obligations, .. } => obligations,
            _ => &[],
        }
    }
}

/// Classify a specimen from histology plus molecular evidence.
///
/// Blueprint 30.10. The conjunction is enforced structurally: `histology` only selects the
/// candidate set, and every transition out of that set is driven by an *observed* marker call.
/// An unobserved marker never advances or retreats a candidate; it becomes an obligation.
pub fn classify(histology: Histology, panel: &MarkerPanel) -> DiagnosticResolution {
    let table: Vec<&EntityCriteria> = CRITERIA
        .iter()
        .filter(|criteria| criteria.histology == histology)
        .collect();

    if table.is_empty() {
        return DiagnosticResolution::NotOtherwiseResolved {
            histology,
            excluded: BTreeSet::new(),
        };
    }

    let mut satisfied = BTreeSet::new();
    let mut pending: Vec<&EntityCriteria> = Vec::new();
    let mut excluded = BTreeSet::new();

    for criteria in &table {
        match evaluate(criteria, panel) {
            Verdict::Satisfied => {
                satisfied.insert(criteria.entity);
            }
            Verdict::Excluded => {
                excluded.insert(criteria.entity);
            }
            Verdict::Pending => pending.push(criteria),
        }
    }

    match (satisfied.len(), pending.len()) {
        (1, _) => {
            let entity = *satisfied.iter().next().expect("length checked to be one");
            let criteria = table
                .iter()
                .find(|c| c.entity == entity)
                .expect("satisfied entities come from the filtered table");
            DiagnosticResolution::Integrated {
                entity,
                grade: grade_for(entity, panel),
                evidence: satisfied_evidence(criteria, panel),
            }
        }
        (n, _) if n > 1 => DiagnosticResolution::Mixed {
            candidates: satisfied,
        },
        (0, 0) => DiagnosticResolution::NotOtherwiseResolved { histology, excluded },
        (0, 1) => DiagnosticResolution::Provisional {
            candidate: pending[0].entity,
            obligations: obligations_for(&pending, panel),
        },
        (0, _) => DiagnosticResolution::Unresolved {
            candidates: pending.iter().map(|c| c.entity).collect(),
            obligations: obligations_for(&pending, panel),
        },
        _ => unreachable!("satisfied.len() is exhaustively covered above"),
    }
}

enum Verdict {
    Satisfied,
    Pending,
    Excluded,
}

fn evaluate(criteria: &EntityCriteria, panel: &MarkerPanel) -> Verdict {
    for (marker, call) in criteria.exclusionary {
        if panel.state(*marker) == Observed::Value(*call) {
            return Verdict::Excluded;
        }
    }

    let mut complete = true;
    for (marker, call) in criteria.required {
        match panel.state(*marker) {
            Observed::Value(observed) if observed == *call => {}
            Observed::Value(_) => return Verdict::Excluded,
            Observed::Unobserved(_) => complete = false,
        }
    }

    let supportive_hits = criteria
        .supportive
        .iter()
        .filter(|(marker, call)| panel.state(*marker) == Observed::Value(*call))
        .count();
    if supportive_hits < criteria.min_supportive {
        complete = false;
    }

    if complete {
        Verdict::Satisfied
    } else {
        Verdict::Pending
    }
}

fn satisfied_evidence(criteria: &EntityCriteria, panel: &MarkerPanel) -> Vec<SatisfiedEvidence> {
    let mut evidence: Vec<SatisfiedEvidence> = criteria
        .required
        .iter()
        .map(|(marker, call)| SatisfiedEvidence {
            marker: *marker,
            role: EvidenceRole::Required,
            call: *call,
        })
        .collect();
    for (marker, call) in criteria.supportive {
        if panel.state(*marker) == Observed::Value(*call) {
            evidence.push(SatisfiedEvidence {
                marker: *marker,
                role: EvidenceRole::Supportive,
                call: *call,
            });
        }
    }
    evidence
}

/// Build the prioritized evidence request across all surviving candidates.
///
/// Priority is the number of candidates a marker currently discriminates, which makes the first
/// obligation the assay that removes the most ambiguity per unit of scarce tissue — the
/// "minimal-evidence acquisition" metric of 30.10.
fn obligations_for(pending: &[&EntityCriteria], panel: &MarkerPanel) -> Vec<EvidenceObligation> {
    let mut counts: BTreeMap<MolecularMarker, usize> = BTreeMap::new();
    for criteria in pending {
        for (marker, _) in criteria.required.iter().chain(criteria.supportive.iter()) {
            if !panel.state(*marker).is_observed() {
                *counts.entry(*marker).or_insert(0) += 1;
            }
        }
    }

    let mut obligations: Vec<EvidenceObligation> = counts
        .into_iter()
        .map(|(marker, discriminates)| EvidenceObligation {
            marker,
            role: role_of(pending, marker),
            state: panel.state(marker),
            discriminates,
        })
        .collect();
    obligations.sort_by(|a, b| {
        b.discriminates
            .cmp(&a.discriminates)
            .then(a.marker.cmp(&b.marker))
    });
    obligations
}

fn role_of(pending: &[&EntityCriteria], marker: MolecularMarker) -> EvidenceRole {
    if pending
        .iter()
        .any(|c| c.required.iter().any(|(m, _)| *m == marker))
    {
        EvidenceRole::Required
    } else {
        EvidenceRole::Supportive
    }
}

/// Grade, where the modelled evidence determines it.
///
/// Grades 2 and 3 depend on mitotic activity, necrosis and microvascular proliferation, none of
/// which this module models. Where those would be needed the result is
/// [`ObservationStatus::NotCollected`] rather than a default to the lower grade, which would
/// systematically understate disease.
fn grade_for(entity: CnsEntity, panel: &MarkerPanel) -> Observed<CnsGrade> {
    match entity {
        CnsEntity::GlioblastomaIdhWildtype
        | CnsEntity::DiffuseMidlineGliomaH3K27Altered
        | CnsEntity::DiffuseHemisphericGliomaH3G34Mutant => Observed::Value(CnsGrade::Four),
        CnsEntity::AstrocytomaIdhMutant => {
            match panel.state(MolecularMarker::Cdkn2aCdkn2bHomozygousDeletion) {
                Observed::Value(MarkerCall::Present) => Observed::Value(CnsGrade::Four),
                _ => Observed::Unobserved(ObservationStatus::NotCollected),
            }
        }
        CnsEntity::OligodendrogliomaIdhMutant1p19qCodeleted => {
            Observed::Unobserved(ObservationStatus::NotCollected)
        }
    }
}

/// The diagnosis exactly as the source system wrote it (30.01).
///
/// Immutable by construction: there are accessors and no mutators, and reclassification does not
/// consume it. 30.01 requires that historical labels be treated as time-valid observations
/// rather than as errors to be corrected, so overwriting this text would destroy the record of
/// what was believed when a decision was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceDiagnosis {
    text: String,
    ontology_version: String,
}

impl SourceDiagnosis {
    pub fn new(text: impl Into<String>, ontology_version: impl Into<String>) -> Self {
        SourceDiagnosis {
            text: text.into(),
            ontology_version: ontology_version.into(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn ontology_version(&self) -> &str {
        &self.ontology_version
    }
}

/// A later classification that cites, rather than replaces, the source diagnosis (30.01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reclassification {
    pub source: SourceDiagnosis,
    pub resolution: DiagnosticResolution,
    /// Version of the criteria table applied. Without it, 30.01's prohibition on retroactively
    /// applying future classification rules is unenforceable.
    pub rule_version: String,
}

impl Reclassification {
    pub fn new(
        source: SourceDiagnosis,
        resolution: DiagnosticResolution,
        rule_version: impl Into<String>,
    ) -> Self {
        Reclassification {
            source,
            resolution,
            rule_version: rule_version.into(),
        }
    }
}
