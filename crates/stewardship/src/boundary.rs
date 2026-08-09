//! Governance around the medical and neuroscience boundary, not a second boundary.
//!
//! Implements blueprint 14.14 (Medical and Neuroscience Boundary): its permitted initial scope, its
//! exclusions, its domain-evidence requirement, and its clinical-transition gate.
//!
//! # `bioprism-onco` owns the boundary; this owns what happens around it
//!
//! `bioprism_onco::boundary` is the typed research boundary. It triages a request, splits cohort
//! evidence from person-level action, refuses to read the caller's claimed role or urgency, and
//! raises an `EscalationNotice` whose type makes a recommendation unrepresentable. That is the
//! runtime guard, it is good, and nothing here routes around it or reimplements it. This module
//! does not triage anything: there is no function taking a request and returning a disposition.
//!
//! Three things `onco` deliberately leaves open, which are exactly 14.14's subject:
//!
//! 1. **What makes a pack admissible in the first place.** 14.14: *"Neuroscience pack cards state
//!    data source, modality, population, intended research capability, limitations, and
//!    clinician/scientist review."* [`DomainPackCard::admit`] refuses a card missing any of the
//!    six, and refuses one whose only domain review is by its own author.
//! 2. **What happens to a raised escalation.** `onco` raises the notice; nothing consumes it.
//!    [`EscalationDocket`] is the ledger: a crossing is *open* until a named domain expert records a
//!    decision, and [`CrossingDisposition::Refused`] is terminal — there is no transition out of it
//!    and no override argument anywhere in this module.
//! 3. **What it would take to leave research use.** 14.14 lists six obligations and adds that
//!    *"open-source availability does not waive these obligations"*. [`TransitionDossier`] has no
//!    `waive` method, no force flag, and no constructor for
//!    [`ClinicalTransitionCertificate`] other than discharging all six with a named responsible
//!    party for each.
//!
//! # The brand rule, made structural
//!
//! 14.14's last paragraph: *"Domain benchmark performance is not advertised as clinical
//! competence."* Written as a lint that would be a string search. Written as a type it is this:
//! the only thing that produces a [`ClinicalTransitionCertificate`] is
//! [`TransitionDossier::certify`], and its inputs are six discharged obligations. There is no
//! constructor taking a score, a [`crate::claim::IssuedClaim`], a benchmark result, or a
//! `ClaimTier`. A perfect result on every neuroscience pack in the catalogue moves the dossier not
//! at all, because benchmark evidence is not an input to it — which is the sentence, as an absence
//! of an argument rather than as a warning.
//!
//! # A human refusal is final
//!
//! The one asymmetry worth stating plainly. [`EscalationDocket::permit`] and
//! [`EscalationDocket::refuse`] both close a docket, but only refusal is terminal: a permitted
//! crossing can be revisited and refused later, and a refused one cannot be revisited at all.
//! [`crate::error::BoundaryError::HumanRefusalIsFinal`] is returned for every subsequent attempt,
//! including another refusal. The asymmetry is deliberate and points the safe way: the failure this
//! guards against is a system that keeps asking until it gets a yes.
//!
//! # Not implemented
//!
//! No triage, no request classification, no detection of embedded instructions, no consent model —
//! `bioprism-onco` and `bioprism-policy` own those and each says what it cannot do. No clock: a
//! docket does not time out, and an untouched docket stays open forever, which is the correct
//! behaviour for a question nobody answered. No notification, no queue, no reviewer assignment. No
//! regulatory content: [`ClinicalObligation`] names six obligations and asserts nothing about what
//! discharging one involves in any jurisdiction.

use crate::error::BoundaryError;
use crate::id::{Actor, ActorId, Epoch, Role};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// 14.14's "allowed initial scope", one variant per item in the module's list.
///
/// Closed, because the value of the list is that it is exhaustive: a pack whose purpose is not on
/// it is not admitted pending a judgement call, it is simply not admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermittedResearchScope {
    LiteratureEvidenceAcquisition,
    DatasetAndMetadataHandling,
    CodeAndStatisticalReproducibility,
    ImagingFormatWorkflow,
    Provenance,
    UncertaintyCommunication,
    /// Trial and registry research tasks over public or synthetic data.
    TrialAndRegistryResearch,
}

impl PermittedResearchScope {
    pub const ALL: [PermittedResearchScope; 7] = [
        PermittedResearchScope::LiteratureEvidenceAcquisition,
        PermittedResearchScope::DatasetAndMetadataHandling,
        PermittedResearchScope::CodeAndStatisticalReproducibility,
        PermittedResearchScope::ImagingFormatWorkflow,
        PermittedResearchScope::Provenance,
        PermittedResearchScope::UncertaintyCommunication,
        PermittedResearchScope::TrialAndRegistryResearch,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            PermittedResearchScope::LiteratureEvidenceAcquisition => {
                "literature_evidence_acquisition"
            }
            PermittedResearchScope::DatasetAndMetadataHandling => "dataset_and_metadata_handling",
            PermittedResearchScope::CodeAndStatisticalReproducibility => {
                "code_and_statistical_reproducibility"
            }
            PermittedResearchScope::ImagingFormatWorkflow => "imaging_format_workflow",
            PermittedResearchScope::Provenance => "provenance",
            PermittedResearchScope::UncertaintyCommunication => "uncertainty_communication",
            PermittedResearchScope::TrialAndRegistryResearch => "trial_and_registry_research",
        }
    }
}

impl fmt::Display for PermittedResearchScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 14.14's exclusions, one variant per item in the module's list.
///
/// These are not scopes that need stronger evidence. They are outside the platform, and there is no
/// function in this crate that admits one — [`DomainPackCard`] can *declare* an excluded use, and
/// declaring one is how a card gets refused, which is the only reason the declaration exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExcludedUse {
    PatientSpecificDiagnosis,
    Triage,
    Prognosis,
    TreatmentSelection,
    SurgicalGuidance,
    Dosing,
    /// Autonomous communication to patients or clinicians.
    AutonomousCommunication,
}

impl ExcludedUse {
    pub const ALL: [ExcludedUse; 7] = [
        ExcludedUse::PatientSpecificDiagnosis,
        ExcludedUse::Triage,
        ExcludedUse::Prognosis,
        ExcludedUse::TreatmentSelection,
        ExcludedUse::SurgicalGuidance,
        ExcludedUse::Dosing,
        ExcludedUse::AutonomousCommunication,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ExcludedUse::PatientSpecificDiagnosis => "patient_specific_diagnosis",
            ExcludedUse::Triage => "triage",
            ExcludedUse::Prognosis => "prognosis",
            ExcludedUse::TreatmentSelection => "treatment_selection",
            ExcludedUse::SurgicalGuidance => "surgical_guidance",
            ExcludedUse::Dosing => "dosing",
            ExcludedUse::AutonomousCommunication => "autonomous_communication",
        }
    }
}

impl fmt::Display for ExcludedUse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named clinician or domain scientist's review of a pack card.
///
/// The `limitations_noted` field exists because 14.14 asks the card to state limitations and the
/// reviewer to review it: a reviewer who noted none is recorded as having noted none, which is a
/// different statement from a card whose author listed none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainReview {
    pub reviewer: Actor,
    pub at: Epoch,
    #[serde(default)]
    pub limitations_noted: Vec<String>,
}

impl DomainReview {
    pub fn new(reviewer: Actor, at: Epoch) -> Self {
        DomainReview {
            reviewer,
            at,
            limitations_noted: Vec::new(),
        }
    }

    #[must_use]
    pub fn noting(mut self, limitation: impl Into<String>) -> Self {
        self.limitations_noted.push(limitation.into());
        self
    }

    /// Whether this review counts toward admission. A review by anyone other than a domain expert
    /// is recorded and does not count: 14.14 asks for "clinician/scientist review" specifically.
    pub const fn qualifies(&self) -> bool {
        matches!(self.reviewer.role, Role::DomainExpert)
    }
}

/// The card 14.14 requires a neuroscience or medical pack to carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainPackCard {
    pub pack: String,
    pub author: Actor,
    pub data_source: String,
    pub modality: String,
    pub population: String,
    pub intended_research_capability: String,
    #[serde(default)]
    pub limitations: Vec<String>,
    #[serde(default)]
    pub scope: BTreeSet<PermittedResearchScope>,
    /// Uses the card declares that the boundary excludes. Normally empty; a non-empty set is how a
    /// card that misunderstood the platform gets refused rather than quietly admitted.
    #[serde(default)]
    pub declared_excluded_uses: BTreeSet<ExcludedUse>,
    #[serde(default)]
    pub reviews: Vec<DomainReview>,
}

impl DomainPackCard {
    pub fn new(
        pack: impl Into<String>,
        author: Actor,
        data_source: impl Into<String>,
        modality: impl Into<String>,
        population: impl Into<String>,
        intended_research_capability: impl Into<String>,
    ) -> Self {
        DomainPackCard {
            pack: pack.into(),
            author,
            data_source: data_source.into(),
            modality: modality.into(),
            population: population.into(),
            intended_research_capability: intended_research_capability.into(),
            limitations: Vec::new(),
            scope: BTreeSet::new(),
            declared_excluded_uses: BTreeSet::new(),
            reviews: Vec::new(),
        }
    }

    #[must_use]
    pub fn limited_by(mut self, limitation: impl Into<String>) -> Self {
        self.limitations.push(limitation.into());
        self
    }

    #[must_use]
    pub fn scoped_to(mut self, scope: PermittedResearchScope) -> Self {
        self.scope.insert(scope);
        self
    }

    #[must_use]
    pub fn declaring(mut self, use_case: ExcludedUse) -> Self {
        self.declared_excluded_uses.insert(use_case);
        self
    }

    #[must_use]
    pub fn reviewed_by(mut self, review: DomainReview) -> Self {
        self.reviews.push(review);
        self
    }

    /// Admit the pack for research use, or say what is missing.
    ///
    /// Checks in the order a reader would want them: the excluded-use declaration first, because a
    /// card asking for triage is not a card with a missing field; then the six required statements;
    /// then the review.
    pub fn admit(&self) -> Result<ResearchStanding, BoundaryError> {
        if let Some(excluded) = self.declared_excluded_uses.iter().next() {
            return Err(BoundaryError::ExcludedUseDeclared {
                use_case: excluded.as_str(),
            });
        }
        for (field, value) in [
            ("data source", &self.data_source),
            ("modality", &self.modality),
            ("population", &self.population),
            (
                "intended research capability",
                &self.intended_research_capability,
            ),
        ] {
            if value.trim().is_empty() {
                return Err(BoundaryError::MissingDomainEvidence { field });
            }
        }
        if self.limitations.iter().all(|l| l.trim().is_empty()) {
            return Err(BoundaryError::MissingDomainEvidence {
                field: "limitations",
            });
        }
        if self.scope.is_empty() {
            return Err(BoundaryError::NoPermittedScope);
        }

        let qualifying: Vec<&DomainReview> =
            self.reviews.iter().filter(|r| r.qualifies()).collect();
        if qualifying.is_empty() {
            return Err(BoundaryError::NoDomainReview);
        }
        if qualifying.iter().all(|r| r.reviewer.id == self.author.id) {
            return Err(BoundaryError::SelfDomainReview {
                party: self.author.id.clone(),
            });
        }

        Ok(ResearchStanding {
            pack: self.pack.clone(),
            scope: self.scope.clone(),
            reviewers: qualifying.iter().map(|r| r.reviewer.id.clone()).collect(),
        })
    }
}

/// What an admitted pack is allowed to be used for.
///
/// `Serialize` without `Deserialize`, and the constructor is [`DomainPackCard::admit`]. The point of
/// the asymmetry is that a standing parsed from JSON is a standing nobody reviewed.
///
/// There is no method on this type taking an [`ExcludedUse`]. Not one that returns `false`, not one
/// that returns an error — none. The exclusions of 14.14 are not permissions this type withholds;
/// they are outside what it can talk about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "a research standing is the admission decision; dropping it discards the review"]
pub struct ResearchStanding {
    pack: String,
    scope: BTreeSet<PermittedResearchScope>,
    reviewers: BTreeSet<ActorId>,
}

impl ResearchStanding {
    pub fn pack(&self) -> &str {
        &self.pack
    }

    pub fn permits(&self, scope: PermittedResearchScope) -> bool {
        self.scope.contains(&scope)
    }

    pub fn scope(&self) -> Vec<PermittedResearchScope> {
        self.scope.iter().copied().collect()
    }

    pub fn reviewers(&self) -> Vec<&ActorId> {
        self.reviewers.iter().collect()
    }
}

impl fmt::Display for ResearchStanding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} admitted for {} research scopes by {} domain reviewer(s)",
            self.pack,
            self.scope.len(),
            self.reviewers.len()
        )
    }
}

/// Why a crossing was raised.
///
/// Deliberately *not* `bioprism_onco::EscalationTrigger`. That enumeration belongs to the crate
/// that does the triage and describes what the guard saw in a request. These four describe what a
/// *governance* process was asked to decide, which is a coarser question asked at a different time,
/// and the two lists would diverge if either crate tried to serve both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrossingTrigger {
    /// A pack or workflow wants a scope the admitted standing does not carry.
    ScopeExtensionRequested,
    /// An output would be read by someone making a care decision.
    OutputEntersCarePathway,
    /// Data with identifiers reached a research workflow.
    IdentifiableDataInResearchPath,
    /// A downstream consumer described the platform in clinical terms.
    ClinicalCharacterisationOfResults,
}

impl CrossingTrigger {
    pub const fn as_str(self) -> &'static str {
        match self {
            CrossingTrigger::ScopeExtensionRequested => "scope_extension_requested",
            CrossingTrigger::OutputEntersCarePathway => "output_enters_care_pathway",
            CrossingTrigger::IdentifiableDataInResearchPath => "identifiable_data_in_research_path",
            CrossingTrigger::ClinicalCharacterisationOfResults => {
                "clinical_characterisation_of_results"
            }
        }
    }
}

impl fmt::Display for CrossingTrigger {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a human decided about a raised crossing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum CrossingDisposition {
    /// Nobody has decided. The default, and the state in which nothing may be released.
    Open,
    /// A named domain expert permitted it, for a stated reason.
    Permitted {
        by: ActorId,
        at: Epoch,
        rationale: String,
    },
    /// A named domain expert refused it. Terminal.
    Refused {
        by: ActorId,
        at: Epoch,
        reason: String,
    },
}

impl CrossingDisposition {
    pub const fn is_open(&self) -> bool {
        matches!(self, CrossingDisposition::Open)
    }

    pub const fn is_refused(&self) -> bool {
        matches!(self, CrossingDisposition::Refused { .. })
    }
}

/// One raised crossing and what happened to it.
///
/// The docket is the whole of this crate's contribution to 14.14's escalation story: `onco` raises,
/// this records, and a human closes. Nothing closes a docket automatically, and there is no epoch
/// after which an open docket becomes permitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EscalationDocket {
    pub id: String,
    pub trigger: CrossingTrigger,
    pub raised_at: Epoch,
    pub subject: String,
    disposition: CrossingDisposition,
}

impl EscalationDocket {
    pub fn raise(
        id: impl Into<String>,
        trigger: CrossingTrigger,
        subject: impl Into<String>,
        at: Epoch,
    ) -> Self {
        EscalationDocket {
            id: id.into(),
            trigger,
            raised_at: at,
            subject: subject.into(),
            disposition: CrossingDisposition::Open,
        }
    }

    pub fn disposition(&self) -> &CrossingDisposition {
        &self.disposition
    }

    pub const fn is_open(&self) -> bool {
        self.disposition.is_open()
    }

    fn check_decidable(&self, by: &Actor) -> Result<(), BoundaryError> {
        match &self.disposition {
            CrossingDisposition::Refused { by: who, at, .. } => {
                return Err(BoundaryError::HumanRefusalIsFinal {
                    party: who.clone(),
                    epoch: *at,
                })
            }
            CrossingDisposition::Permitted { .. } => {}
            CrossingDisposition::Open => {}
        }
        if by.role != Role::DomainExpert {
            return Err(BoundaryError::NotQualifiedToDispose { role: by.role });
        }
        Ok(())
    }

    /// Permit the crossing. Revisitable: a permitted docket can later be refused.
    pub fn permit(
        &mut self,
        by: &Actor,
        rationale: impl Into<String>,
        at: Epoch,
    ) -> Result<(), BoundaryError> {
        self.check_decidable(by)?;
        if let CrossingDisposition::Permitted { .. } = self.disposition {
            return Err(BoundaryError::CrossingAlreadyDecided {
                docket: self.id.clone(),
            });
        }
        self.disposition = CrossingDisposition::Permitted {
            by: by.id.clone(),
            at,
            rationale: rationale.into(),
        };
        Ok(())
    }

    /// Refuse the crossing. Terminal: every subsequent call on this docket, including another
    /// refusal, is [`BoundaryError::HumanRefusalIsFinal`].
    pub fn refuse(
        &mut self,
        by: &Actor,
        reason: impl Into<String>,
        at: Epoch,
    ) -> Result<(), BoundaryError> {
        self.check_decidable(by)?;
        self.disposition = CrossingDisposition::Refused {
            by: by.id.clone(),
            at,
            reason: reason.into(),
        };
        Ok(())
    }

    /// Whether a release may proceed under this docket.
    ///
    /// Open is not a yes. That is the whole design: an escalation nobody answered blocks, because
    /// the alternative — proceeding while a question is outstanding — is how a boundary becomes
    /// advisory.
    pub fn release_permitted(&self) -> Result<(), BoundaryError> {
        match &self.disposition {
            CrossingDisposition::Permitted { .. } => Ok(()),
            CrossingDisposition::Open => Err(BoundaryError::CrossingOpen {
                docket: self.id.clone(),
            }),
            CrossingDisposition::Refused { by, at, .. } => {
                Err(BoundaryError::HumanRefusalIsFinal {
                    party: by.clone(),
                    epoch: *at,
                })
            }
        }
    }
}

/// A collection of dockets, so that "is anything outstanding" is one question.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BoundaryLedger {
    dockets: BTreeMap<String, EscalationDocket>,
}

impl BoundaryLedger {
    pub fn new() -> Self {
        BoundaryLedger::default()
    }

    pub fn raise(&mut self, docket: EscalationDocket) -> Result<(), BoundaryError> {
        if self.dockets.contains_key(&docket.id) {
            return Err(BoundaryError::CrossingAlreadyDecided {
                docket: docket.id.clone(),
            });
        }
        self.dockets.insert(docket.id.clone(), docket);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&EscalationDocket> {
        self.dockets.get(id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut EscalationDocket> {
        self.dockets.get_mut(id)
    }

    /// Every docket nobody has decided.
    pub fn open(&self) -> Vec<&EscalationDocket> {
        self.dockets.values().filter(|d| d.is_open()).collect()
    }

    pub fn refused(&self) -> Vec<&EscalationDocket> {
        self.dockets
            .values()
            .filter(|d| d.disposition.is_refused())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.dockets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.dockets.is_empty()
    }
}

/// 14.14's six clinical-transition obligations, verbatim from the module.
///
/// *"Any future clinical-use component requires separate quality management, regulatory strategy,
/// human factors, prospective validation, risk management, and institutional governance;
/// open-source availability does not waive these obligations."*
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClinicalObligation {
    QualityManagement,
    RegulatoryStrategy,
    HumanFactors,
    ProspectiveValidation,
    RiskManagement,
    InstitutionalGovernance,
}

impl ClinicalObligation {
    pub const ALL: [ClinicalObligation; 6] = [
        ClinicalObligation::QualityManagement,
        ClinicalObligation::RegulatoryStrategy,
        ClinicalObligation::HumanFactors,
        ClinicalObligation::ProspectiveValidation,
        ClinicalObligation::RiskManagement,
        ClinicalObligation::InstitutionalGovernance,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ClinicalObligation::QualityManagement => "quality_management",
            ClinicalObligation::RegulatoryStrategy => "regulatory_strategy",
            ClinicalObligation::HumanFactors => "human_factors",
            ClinicalObligation::ProspectiveValidation => "prospective_validation",
            ClinicalObligation::RiskManagement => "risk_management",
            ClinicalObligation::InstitutionalGovernance => "institutional_governance",
        }
    }
}

impl fmt::Display for ClinicalObligation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The record of which transition obligations have been discharged, and by whom.
///
/// There is no `waive`, no `skip`, no `force`, and no argument named `open_source`. That absence is
/// the implementation of 14.14's final clause, and it is worth more than a comment saying the same
/// thing, because a comment can be ignored by a caller in a hurry and a missing method cannot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionDossier {
    discharged: BTreeMap<ClinicalObligation, DischargeRecord>,
}

/// Who discharged an obligation, and the evidence they cited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DischargeRecord {
    pub responsible: Actor,
    pub at: Epoch,
    pub evidence: String,
}

impl TransitionDossier {
    pub fn new() -> Self {
        TransitionDossier::default()
    }

    /// Record an obligation as discharged. Refuses an empty evidence sentence and an unnamed
    /// responsible party — an obligation discharged by nobody is not discharged.
    pub fn discharge(
        &mut self,
        obligation: ClinicalObligation,
        responsible: Actor,
        evidence: impl Into<String>,
        at: Epoch,
    ) -> Result<(), BoundaryError> {
        let evidence = evidence.into();
        if responsible.id.as_str().trim().is_empty() || evidence.trim().is_empty() {
            return Err(BoundaryError::ObligationUnattributed {
                obligation: obligation.as_str(),
            });
        }
        self.discharged.insert(
            obligation,
            DischargeRecord {
                responsible,
                at,
                evidence,
            },
        );
        Ok(())
    }

    /// The obligations still outstanding, in the module's order.
    pub fn outstanding(&self) -> Vec<ClinicalObligation> {
        ClinicalObligation::ALL
            .into_iter()
            .filter(|o| !self.discharged.contains_key(o))
            .collect()
    }

    pub fn record(&self, obligation: ClinicalObligation) -> Option<&DischargeRecord> {
        self.discharged.get(&obligation)
    }

    /// Issue the certificate, or name the first outstanding obligation.
    ///
    /// The only constructor of [`ClinicalTransitionCertificate`], and its inputs are six discharge
    /// records. No benchmark result, claim, tier or score is an input, which is 14.14's brand rule
    /// as an absence rather than as a warning.
    pub fn certify(
        &self,
        component: impl Into<String>,
        at: Epoch,
    ) -> Result<ClinicalTransitionCertificate, BoundaryError> {
        if let Some(outstanding) = self.outstanding().first() {
            return Err(BoundaryError::ObligationOutstanding {
                obligation: outstanding.as_str(),
            });
        }
        Ok(ClinicalTransitionCertificate {
            component: component.into(),
            discharged: self.discharged.clone(),
            at,
        })
    }
}

/// Evidence that all six obligations of 14.14 were discharged for a named component.
///
/// `Serialize` without `Deserialize`, one constructor, private fields. A certificate that could be
/// parsed from JSON is a certificate anybody can write, and this is the one object in the crate
/// whose forgery would matter outside the repository.
///
/// Holding one asserts nothing about whether the component is safe, effective, or lawful anywhere.
/// It asserts that six obligations were recorded as discharged with a named responsible party for
/// each — which is what a governance layer can know, and the boundary of what it should claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "a transition certificate is the gate's output; dropping it discards the dossier"]
pub struct ClinicalTransitionCertificate {
    component: String,
    discharged: BTreeMap<ClinicalObligation, DischargeRecord>,
    at: Epoch,
}

impl ClinicalTransitionCertificate {
    pub fn component(&self) -> &str {
        &self.component
    }

    pub fn at(&self) -> Epoch {
        self.at
    }

    pub fn responsible_for(&self, obligation: ClinicalObligation) -> Option<&Actor> {
        self.discharged.get(&obligation).map(|r| &r.responsible)
    }
}

impl fmt::Display for ClinicalTransitionCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: all {} clinical transition obligations recorded as discharged at {}",
            self.component,
            ClinicalObligation::ALL.len(),
            self.at
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card() -> DomainPackCard {
        DomainPackCard::new(
            "neuro-imaging-workflow",
            Actor::author("pack-author"),
            "public OpenNeuro derivatives",
            "structural MRI, BIDS",
            "adult research volunteers",
            "imaging-format workflow reproducibility",
        )
        .limited_by("no clinical population; no scanner-vendor diversity")
        .scoped_to(PermittedResearchScope::ImagingFormatWorkflow)
        .scoped_to(PermittedResearchScope::CodeAndStatisticalReproducibility)
        .reviewed_by(DomainReview::new(
            Actor::domain_expert("neuroradiologist-1"),
            Epoch(2),
        ))
    }

    #[test]
    fn a_domain_pack_card_missing_any_required_statement_is_not_admitted() {
        assert!(card().admit().is_ok());
        let mut blank = card();
        blank.modality = "  ".into();
        assert!(matches!(
            blank.admit(),
            Err(BoundaryError::MissingDomainEvidence { field: "modality" })
        ));
        let mut unlimited = card();
        unlimited.limitations.clear();
        assert!(matches!(
            unlimited.admit(),
            Err(BoundaryError::MissingDomainEvidence {
                field: "limitations"
            })
        ));
    }

    #[test]
    fn a_pack_whose_only_domain_review_is_its_own_author_is_not_admitted() {
        let card = DomainPackCard::new("p", Actor::domain_expert("same"), "s", "m", "pop", "cap")
            .limited_by("l")
            .scoped_to(PermittedResearchScope::Provenance)
            .reviewed_by(DomainReview::new(Actor::domain_expert("same"), Epoch(1)));
        assert!(matches!(
            card.admit(),
            Err(BoundaryError::SelfDomainReview { .. })
        ));
    }

    #[test]
    fn a_review_by_someone_who_is_not_a_domain_expert_does_not_count_as_domain_review() {
        let mut card = card();
        card.reviews = vec![DomainReview::new(
            Actor::independent_reviewer("engineer-1"),
            Epoch(2),
        )];
        assert!(matches!(card.admit(), Err(BoundaryError::NoDomainReview)));
    }

    #[test]
    fn a_card_declaring_an_excluded_use_is_refused_rather_than_scoped_down() {
        let card = card().declaring(ExcludedUse::Triage);
        assert!(matches!(
            card.admit(),
            Err(BoundaryError::ExcludedUseDeclared { use_case: "triage" })
        ));
    }

    #[test]
    fn a_card_with_no_permitted_scope_declares_nothing_it_may_do() {
        let mut card = card();
        card.scope.clear();
        assert!(matches!(card.admit(), Err(BoundaryError::NoPermittedScope)));
    }

    #[test]
    fn a_research_standing_carries_only_the_scopes_the_card_declared() {
        let standing = card().admit().unwrap();
        assert!(standing.permits(PermittedResearchScope::ImagingFormatWorkflow));
        assert!(!standing.permits(PermittedResearchScope::TrialAndRegistryResearch));
        assert_eq!(standing.scope().len(), 2);
    }

    #[test]
    fn an_open_crossing_blocks_release() {
        let docket = EscalationDocket::raise(
            "d-1",
            CrossingTrigger::OutputEntersCarePathway,
            "cohort summary requested by a care team",
            Epoch(3),
        );
        assert!(docket.is_open());
        assert!(matches!(
            docket.release_permitted(),
            Err(BoundaryError::CrossingOpen { .. })
        ));
    }

    #[test]
    fn a_human_refusal_of_a_boundary_crossing_is_final() {
        let mut docket = EscalationDocket::raise(
            "d-2",
            CrossingTrigger::ScopeExtensionRequested,
            "extend to prognosis",
            Epoch(3),
        );
        let expert = Actor::domain_expert("clinical-lead");
        docket
            .refuse(&expert, "the request is outside research scope", Epoch(4))
            .unwrap();
        for outcome in [
            docket.permit(&expert, "reconsidered", Epoch(5)),
            docket.refuse(&expert, "again", Epoch(5)),
        ] {
            assert!(matches!(
                outcome,
                Err(BoundaryError::HumanRefusalIsFinal { .. })
            ));
        }
        assert!(matches!(
            docket.release_permitted(),
            Err(BoundaryError::HumanRefusalIsFinal { .. })
        ));
    }

    #[test]
    fn a_permitted_crossing_can_still_be_refused_later() {
        let mut docket = EscalationDocket::raise(
            "d-3",
            CrossingTrigger::IdentifiableDataInResearchPath,
            "identifiers found in a derived table",
            Epoch(3),
        );
        let expert = Actor::domain_expert("steward-expert");
        docket
            .permit(&expert, "identifiers removed", Epoch(4))
            .unwrap();
        assert!(docket.release_permitted().is_ok());
        docket
            .refuse(&expert, "removal was incomplete", Epoch(5))
            .unwrap();
        assert!(docket.disposition().is_refused());
    }

    #[test]
    fn only_a_domain_expert_disposes_of_a_boundary_crossing() {
        let mut docket = EscalationDocket::raise(
            "d-4",
            CrossingTrigger::ClinicalCharacterisationOfResults,
            "a press release called the result diagnostic",
            Epoch(3),
        );
        for actor in [
            Actor::author("author-1"),
            Actor::independent_reviewer("reviewer-1"),
            Actor::steward("steward-1"),
        ] {
            assert!(matches!(
                docket.permit(&actor, "fine", Epoch(4)),
                Err(BoundaryError::NotQualifiedToDispose { .. })
            ));
        }
        assert!(docket
            .permit(&Actor::domain_expert("x"), "corrected", Epoch(4))
            .is_ok());
    }

    #[test]
    fn a_ledger_reports_every_undecided_crossing() {
        let mut ledger = BoundaryLedger::new();
        ledger
            .raise(EscalationDocket::raise(
                "a",
                CrossingTrigger::ScopeExtensionRequested,
                "s",
                Epoch(1),
            ))
            .unwrap();
        ledger
            .raise(EscalationDocket::raise(
                "b",
                CrossingTrigger::OutputEntersCarePathway,
                "s",
                Epoch(1),
            ))
            .unwrap();
        assert_eq!(ledger.open().len(), 2);
        ledger
            .get_mut("a")
            .unwrap()
            .refuse(&Actor::domain_expert("e"), "no", Epoch(2))
            .unwrap();
        assert_eq!(ledger.open().len(), 1);
        assert_eq!(ledger.refused().len(), 1);
    }

    #[test]
    fn a_clinical_transition_certificate_needs_all_six_obligations() {
        let mut dossier = TransitionDossier::new();
        assert_eq!(dossier.outstanding().len(), 6);
        for obligation in ClinicalObligation::ALL.into_iter().take(5) {
            dossier
                .discharge(
                    obligation,
                    Actor::domain_expert("qms-lead"),
                    "dossier section",
                    Epoch(4),
                )
                .unwrap();
        }
        assert!(matches!(
            dossier.certify("planner", Epoch(5)),
            Err(BoundaryError::ObligationOutstanding {
                obligation: "institutional_governance"
            })
        ));
        dossier
            .discharge(
                ClinicalObligation::InstitutionalGovernance,
                Actor::domain_expert("governance-office"),
                "board minute 14",
                Epoch(5),
            )
            .unwrap();
        let certificate = dossier.certify("planner", Epoch(6)).unwrap();
        assert_eq!(certificate.component(), "planner");
        assert!(certificate
            .responsible_for(ClinicalObligation::RiskManagement)
            .is_some());
    }

    #[test]
    fn benchmark_evidence_is_not_an_input_to_the_transition_gate() {
        let standing = card().admit().unwrap();
        assert!(standing.permits(PermittedResearchScope::ImagingFormatWorkflow));
        let dossier = TransitionDossier::new();
        assert!(matches!(
            dossier.certify("planner", Epoch(9)),
            Err(BoundaryError::ObligationOutstanding { .. })
        ));
        assert_eq!(dossier.outstanding().len(), 6);
    }

    #[test]
    fn an_obligation_discharged_with_no_evidence_is_not_discharged() {
        let mut dossier = TransitionDossier::new();
        assert!(matches!(
            dossier.discharge(
                ClinicalObligation::HumanFactors,
                Actor::domain_expert("x"),
                "   ",
                Epoch(1)
            ),
            Err(BoundaryError::ObligationUnattributed { .. })
        ));
        assert_eq!(dossier.outstanding().len(), 6);
    }
}
