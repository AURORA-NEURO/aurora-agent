//! Trust zones, the threat-to-control relation, output trust dimensions, and release gates.
//!
//! Blueprint 23.29.
//!
//! # The one thing this module is for
//!
//! 23.29: "An output has **dimensions rather than one trust score**." [`OutputTrust`] has nine
//! dimensions, no `Ord`, no `overall()`, and no arithmetic that would produce a single number from
//! them. A dimension nobody assessed is [`Assessment::Unassessed`], which is not a low score — the
//! same distinction `bioprism-atlas` draws between unmeasured and measured-poor, and the reason
//! [`OutputTrust::relied_upon_for`] refuses rather than degrades when a required dimension is
//! unassessed.
//!
//! # The relation §23 does not state
//!
//! 23.29 lists eleven principal threats and fifteen controls in two adjacent sections and never
//! relates them. A control list that no threat points at is a checklist; a threat with no control
//! is an accepted risk nobody wrote down. [`Threat::controls`] is that relation, and it is **this
//! crate's reading, not the blueprint's**. Its value is that it is falsifiable: the tests assert
//! that every threat has at least one control and every control answers at least one threat, so a
//! future edit that adds a control nothing needs, or a threat nothing covers, fails.
//!
//! # Trust zones bound who may discharge a control
//!
//! 23.29's three zones exist so that "trusting self-reported capability" is a describable error.
//! [`Control::minimum_zone`] — also this crate's reading — says how much trust an actor must have
//! before its performance of a control counts, and [`discharged_by`] refuses an untrusted actor
//! signing off on the controls that exist to constrain it. A benchmark whose integrity check was
//! run by the participant being benchmarked has not been checked.
//!
//! # Not implemented
//!
//! No cryptography. No signature verification, no key rotation, no identity attestation: 23.29's
//! first control is "signed identities and artifact hashes" and this crate can represent the claim
//! and not verify it, exactly as `bioprism-fabric` records for 23.46. No isolation, no rate
//! limiting, no quarantine, no incident workflow. 23.29's red-team program is a *programme* —
//! twelve exercises against a running system — and is recorded in [`RED_TEAM_EXERCISES`] with
//! nothing executing them; this module supplies the vocabulary a red-team report would be written
//! in, not the red team.

use crate::adapter::Grade;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 23.29's three trust zones, ordered least trusted first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustZone {
    /// "agents · models · prompts · adapters · benchmark code · remote tools · generated
    /// components · external artifacts · messages from other organizations".
    Untrusted,
    /// "local runtime · artifact store · approved deterministic evaluators".
    ConditionallyTrusted,
    /// "compiler · policy engine · signature verifier · ledger append rules".
    TrustedCore,
}

impl TrustZone {
    pub const ALL: [TrustZone; 3] = [
        TrustZone::Untrusted,
        TrustZone::ConditionallyTrusted,
        TrustZone::TrustedCore,
    ];

    /// The members 23.29 places in this zone, verbatim.
    pub fn members(self) -> &'static [&'static str] {
        match self {
            TrustZone::TrustedCore => &[
                "compiler",
                "policy engine",
                "signature verifier",
                "ledger append rules",
            ],
            TrustZone::ConditionallyTrusted => &[
                "local runtime",
                "artifact store",
                "approved deterministic evaluators",
            ],
            TrustZone::Untrusted => &[
                "agents",
                "models",
                "prompts",
                "adapters",
                "benchmark code",
                "remote tools",
                "generated components",
                "external artifacts",
                "messages from other organizations",
            ],
        }
    }
}

/// 23.29's eleven principal threats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Threat {
    PromptAndContextInjection,
    CapabilityEscalation,
    ConfusedDeputy,
    SybilAndCollusion,
    BenchmarkPoisoning,
    ArtifactSubstitution,
    MemoryPoisoning,
    DataExfiltration,
    DenialOfWallet,
    ProtocolDeadlockOrFlooding,
    SupplyChainCompromise,
}

/// 23.29's fifteen controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Control {
    SignedIdentitiesAndArtifactHashes,
    LeastPrivilegeCapabilityGrants,
    TransitiveRevocation,
    EffectAndInformationFlowChecks,
    IsolatedExecution,
    NoAmbientCredentials,
    BudgetQuotasAndRateLimits,
    IndependentEvaluatorProcess,
    ContentAndPromptInjectionLabels,
    DeterministicChecksBeforeModelJudges,
    ParticipantDiversityAndAliasDetection,
    AppendOnlyAuditHistory,
    ReproducibleResultBundles,
    QuarantineAndRollback,
    SecureDefaultsForPublicMolecules,
}

impl Control {
    pub const ALL: [Control; 15] = [
        Control::SignedIdentitiesAndArtifactHashes,
        Control::LeastPrivilegeCapabilityGrants,
        Control::TransitiveRevocation,
        Control::EffectAndInformationFlowChecks,
        Control::IsolatedExecution,
        Control::NoAmbientCredentials,
        Control::BudgetQuotasAndRateLimits,
        Control::IndependentEvaluatorProcess,
        Control::ContentAndPromptInjectionLabels,
        Control::DeterministicChecksBeforeModelJudges,
        Control::ParticipantDiversityAndAliasDetection,
        Control::AppendOnlyAuditHistory,
        Control::ReproducibleResultBundles,
        Control::QuarantineAndRollback,
        Control::SecureDefaultsForPublicMolecules,
    ];

    /// The least trusted zone an actor may sit in and still discharge this control.
    ///
    /// **This crate's reading.** 23.29 gives zones and controls and never says which actor may
    /// perform which. The organising idea below is that a control constraining untrusted
    /// participants cannot be discharged by one: signature verification, ledger append rules and
    /// policy enforcement are trusted-core work by 23.29's own zone membership, and evaluator
    /// independence is meaningless if the evaluated party runs it.
    pub fn minimum_zone(self) -> TrustZone {
        match self {
            Control::SignedIdentitiesAndArtifactHashes
            | Control::LeastPrivilegeCapabilityGrants
            | Control::TransitiveRevocation
            | Control::EffectAndInformationFlowChecks
            | Control::NoAmbientCredentials
            | Control::AppendOnlyAuditHistory
            | Control::IndependentEvaluatorProcess => TrustZone::TrustedCore,
            Control::IsolatedExecution
            | Control::BudgetQuotasAndRateLimits
            | Control::ContentAndPromptInjectionLabels
            | Control::DeterministicChecksBeforeModelJudges
            | Control::ParticipantDiversityAndAliasDetection
            | Control::ReproducibleResultBundles
            | Control::QuarantineAndRollback
            | Control::SecureDefaultsForPublicMolecules => TrustZone::ConditionallyTrusted,
        }
    }

    /// The threats this control answers, derived from [`Threat::controls`].
    pub fn threats(self) -> BTreeSet<Threat> {
        Threat::ALL
            .into_iter()
            .filter(|threat| threat.controls().contains(&self))
            .collect()
    }
}

impl Threat {
    pub const ALL: [Threat; 11] = [
        Threat::PromptAndContextInjection,
        Threat::CapabilityEscalation,
        Threat::ConfusedDeputy,
        Threat::SybilAndCollusion,
        Threat::BenchmarkPoisoning,
        Threat::ArtifactSubstitution,
        Threat::MemoryPoisoning,
        Threat::DataExfiltration,
        Threat::DenialOfWallet,
        Threat::ProtocolDeadlockOrFlooding,
        Threat::SupplyChainCompromise,
    ];

    /// 23.29's own one-line description of the threat.
    pub fn description(self) -> &'static str {
        match self {
            Threat::PromptAndContextInjection => {
                "malicious artifacts attempt to change goals, exfiltrate data, or invoke tools"
            }
            Threat::CapabilityEscalation => {
                "a participant delegates or invokes authority it does not own"
            }
            Threat::ConfusedDeputy => {
                "an authorized agent is manipulated into acting for an unauthorized participant"
            }
            Threat::SybilAndCollusion => {
                "multiple apparent agents share one operator or coordinate to inflate consensus"
            }
            Threat::BenchmarkPoisoning => {
                "generated tests or public results are manipulated to favor one architecture"
            }
            Threat::ArtifactSubstitution => {
                "a result references a different file, model, schema, or world snapshot than the one evaluated"
            }
            Threat::MemoryPoisoning => {
                "false state is inserted into shared or organizational memory"
            }
            Threat::DataExfiltration => {
                "sensitive content leaves through messages, traces, model providers, artifacts, or side channels"
            }
            Threat::DenialOfWallet => {
                "agents trigger expensive models, branches, tools, or human reviews"
            }
            Threat::ProtocolDeadlockOrFlooding => {
                "participants exploit retries, subscriptions, or message fan-out"
            }
            Threat::SupplyChainCompromise => {
                "a component, adapter, model endpoint, or dependency is malicious"
            }
        }
    }

    /// The controls that answer this threat.
    ///
    /// **Not in the blueprint.** 23.29 prints the two lists adjacently and relates them nowhere,
    /// which leaves a policy saying "this threat is covered" with nothing to evaluate. The relation
    /// below is this crate's reading; the pair of coverage properties it must satisfy is asserted
    /// in the tests rather than assumed here.
    pub fn controls(self) -> &'static [Control] {
        match self {
            Threat::PromptAndContextInjection => &[
                Control::ContentAndPromptInjectionLabels,
                Control::EffectAndInformationFlowChecks,
                Control::IsolatedExecution,
                Control::DeterministicChecksBeforeModelJudges,
            ],
            Threat::CapabilityEscalation => &[
                Control::LeastPrivilegeCapabilityGrants,
                Control::TransitiveRevocation,
                Control::EffectAndInformationFlowChecks,
                Control::NoAmbientCredentials,
            ],
            Threat::ConfusedDeputy => &[
                Control::LeastPrivilegeCapabilityGrants,
                Control::NoAmbientCredentials,
                Control::EffectAndInformationFlowChecks,
                Control::AppendOnlyAuditHistory,
            ],
            Threat::SybilAndCollusion => &[
                Control::ParticipantDiversityAndAliasDetection,
                Control::SignedIdentitiesAndArtifactHashes,
                Control::IndependentEvaluatorProcess,
            ],
            Threat::BenchmarkPoisoning => &[
                Control::IndependentEvaluatorProcess,
                Control::DeterministicChecksBeforeModelJudges,
                Control::ReproducibleResultBundles,
                Control::AppendOnlyAuditHistory,
            ],
            Threat::ArtifactSubstitution => &[
                Control::SignedIdentitiesAndArtifactHashes,
                Control::ReproducibleResultBundles,
                Control::AppendOnlyAuditHistory,
            ],
            Threat::MemoryPoisoning => &[
                Control::AppendOnlyAuditHistory,
                Control::SignedIdentitiesAndArtifactHashes,
                Control::QuarantineAndRollback,
                Control::EffectAndInformationFlowChecks,
            ],
            Threat::DataExfiltration => &[
                Control::EffectAndInformationFlowChecks,
                Control::ContentAndPromptInjectionLabels,
                Control::IsolatedExecution,
                Control::SecureDefaultsForPublicMolecules,
            ],
            Threat::DenialOfWallet => &[
                Control::BudgetQuotasAndRateLimits,
                Control::LeastPrivilegeCapabilityGrants,
                Control::SecureDefaultsForPublicMolecules,
            ],
            Threat::ProtocolDeadlockOrFlooding => &[
                Control::BudgetQuotasAndRateLimits,
                Control::QuarantineAndRollback,
            ],
            Threat::SupplyChainCompromise => &[
                Control::SignedIdentitiesAndArtifactHashes,
                Control::IsolatedExecution,
                Control::QuarantineAndRollback,
                Control::SecureDefaultsForPublicMolecules,
            ],
        }
    }
}

/// An actor claiming to have discharged a control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Actor {
    pub name: String,
    pub zone: TrustZone,
}

/// Why a discharge claim was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum DischargeRefusal {
    #[error("{actor} is {actual:?} and {control:?} requires at least {required:?}")]
    ZoneTooLow {
        actor: String,
        control: Control,
        actual: TrustZone,
        required: TrustZone,
    },

    #[error("{actor} cannot discharge {control:?} on its own output")]
    SelfEvaluation { actor: String, control: Control },
}

/// Whether an actor's claim to have discharged a control counts.
///
/// The self-evaluation clause covers [`Control::IndependentEvaluatorProcess`] specifically, which
/// is the one control whose whole content is "somebody else did this".
pub fn discharged_by(
    actor: &Actor,
    control: Control,
    subject: &str,
) -> Result<(), DischargeRefusal> {
    if control == Control::IndependentEvaluatorProcess && actor.name == subject {
        return Err(DischargeRefusal::SelfEvaluation {
            actor: actor.name.clone(),
            control,
        });
    }
    let required = control.minimum_zone();
    if actor.zone < required {
        return Err(DischargeRefusal::ZoneTooLow {
            actor: actor.name.clone(),
            control,
            actual: actor.zone,
            required,
        });
    }
    Ok(())
}

/// 23.29's nine dimensions of output trust.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustDimension {
    IdentityAuthenticity,
    ArtifactIntegrity,
    ExecutionReproducibility,
    EvidenceStrength,
    EvaluatorStrength,
    PolicyCompliance,
    ParticipantIndependence,
    Freshness,
    ConfidentialityHandling,
}

impl TrustDimension {
    pub const ALL: [TrustDimension; 9] = [
        TrustDimension::IdentityAuthenticity,
        TrustDimension::ArtifactIntegrity,
        TrustDimension::ExecutionReproducibility,
        TrustDimension::EvidenceStrength,
        TrustDimension::EvaluatorStrength,
        TrustDimension::PolicyCompliance,
        TrustDimension::ParticipantIndependence,
        TrustDimension::Freshness,
        TrustDimension::ConfidentialityHandling,
    ];
}

/// One dimension's state.
///
/// Three states, not two, and not a number. `Adequate` and `Inadequate` are judgements a reviewer
/// records; [`Assessment::Unassessed`] is the absence of one. Collapsing the third into the second
/// would let an unexamined output look merely weak instead of unexamined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "assessment")]
pub enum Assessment {
    Adequate { basis: String },
    Inadequate { finding: String },
    Unassessed,
}

/// 23.29's dimensional trust, with no scalar anywhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputTrust {
    pub output: String,
    dimensions: BTreeMap<TrustDimension, Assessment>,
}

/// Why an output may not be relied upon for a purpose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum RelianceRefusal {
    #[error("{output} is inadequate on {dimension:?}: {finding}")]
    Inadequate {
        output: String,
        dimension: TrustDimension,
        finding: String,
    },

    #[error("{output} was never assessed on {dimension:?}")]
    Unassessed {
        output: String,
        dimension: TrustDimension,
    },
}

impl OutputTrust {
    /// A new record with every dimension unassessed, which is the truthful starting state.
    pub fn new(output: impl Into<String>) -> Self {
        OutputTrust {
            output: output.into(),
            dimensions: TrustDimension::ALL
                .into_iter()
                .map(|d| (d, Assessment::Unassessed))
                .collect(),
        }
    }

    pub fn adequate(mut self, dimension: TrustDimension, basis: impl Into<String>) -> Self {
        self.dimensions.insert(
            dimension,
            Assessment::Adequate {
                basis: basis.into(),
            },
        );
        self
    }

    pub fn inadequate(mut self, dimension: TrustDimension, finding: impl Into<String>) -> Self {
        self.dimensions.insert(
            dimension,
            Assessment::Inadequate {
                finding: finding.into(),
            },
        );
        self
    }

    pub fn assessment(&self, dimension: TrustDimension) -> &Assessment {
        self.dimensions
            .get(&dimension)
            .unwrap_or(&Assessment::Unassessed)
    }

    pub fn unassessed(&self) -> BTreeSet<TrustDimension> {
        TrustDimension::ALL
            .into_iter()
            .filter(|d| matches!(self.assessment(*d), Assessment::Unassessed))
            .collect()
    }

    /// Whether this output may be relied upon by a caller that needs specific dimensions.
    ///
    /// The caller names what it needs. There is no default set and no aggregate threshold, because
    /// a threshold over nine incommensurable dimensions is the single trust score 23.29 rejects.
    /// Unassessed refuses just as inadequate does, and the two refusals are distinguishable.
    pub fn relied_upon_for(
        &self,
        required: &BTreeSet<TrustDimension>,
    ) -> Result<(), Vec<RelianceRefusal>> {
        let refusals: Vec<RelianceRefusal> = required
            .iter()
            .filter_map(|dimension| match self.assessment(*dimension) {
                Assessment::Adequate { .. } => None,
                Assessment::Inadequate { finding } => Some(RelianceRefusal::Inadequate {
                    output: self.output.clone(),
                    dimension: *dimension,
                    finding: finding.clone(),
                }),
                Assessment::Unassessed => Some(RelianceRefusal::Unassessed {
                    output: self.output.clone(),
                    dimension: *dimension,
                }),
            })
            .collect();
        if refusals.is_empty() {
            Ok(())
        } else {
            Err(refusals)
        }
    }
}

/// 23.29's eight cross-organization agreement clauses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgreementClause {
    IdentityAndKeyRotation,
    DataHandling,
    AllowedDelegates,
    AuditExchange,
    IncidentNotification,
    Revocation,
    ResultPublication,
    DisputeResolution,
}

impl AgreementClause {
    pub const ALL: [AgreementClause; 8] = [
        AgreementClause::IdentityAndKeyRotation,
        AgreementClause::DataHandling,
        AgreementClause::AllowedDelegates,
        AgreementClause::AuditExchange,
        AgreementClause::IncidentNotification,
        AgreementClause::Revocation,
        AgreementClause::ResultPublication,
        AgreementClause::DisputeResolution,
    ];
}

/// A federation agreement between two organizations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossOrgAgreement {
    pub parties: (String, String),
    pub clauses: BTreeSet<AgreementClause>,
}

impl CrossOrgAgreement {
    /// 23.29: "Federated collaboration **requires** explicit agreements for: ..." — so a missing
    /// clause is a gap in the agreement, not a default.
    pub fn gaps(&self) -> BTreeSet<AgreementClause> {
        AgreementClause::ALL
            .into_iter()
            .filter(|c| !self.clauses.contains(c))
            .collect()
    }

    pub fn complete(&self) -> bool {
        self.gaps().is_empty()
    }
}

/// 23.29's six security release gates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseGate {
    CoreAuthorizationAndEffectTestsPass,
    UntrustedCodeHasNoAmbientHostAccess,
    ReplayCannotExecuteProductionEffects,
    PublicBundlesRedactSensitiveByDefault,
    AdapterLossAndTrustGradesVisible,
    IncidentResponseAndRevocationTested,
}

impl ReleaseGate {
    pub const ALL: [ReleaseGate; 6] = [
        ReleaseGate::CoreAuthorizationAndEffectTestsPass,
        ReleaseGate::UntrustedCodeHasNoAmbientHostAccess,
        ReleaseGate::ReplayCannotExecuteProductionEffects,
        ReleaseGate::PublicBundlesRedactSensitiveByDefault,
        ReleaseGate::AdapterLossAndTrustGradesVisible,
        ReleaseGate::IncidentResponseAndRevocationTested,
    ];
}

/// The state of a proposed public stable release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseCandidate {
    pub name: String,
    pub passed: BTreeSet<ReleaseGate>,
    /// The adapter grades this release publishes, if any.
    ///
    /// [`ReleaseGate::AdapterLossAndTrustGradesVisible`] is the one gate whose evidence is a value
    /// rather than a test result, so it is carried here and checked rather than asserted.
    pub published_grades: BTreeMap<String, Grade>,
}

impl ReleaseCandidate {
    pub fn new(name: impl Into<String>) -> Self {
        ReleaseCandidate {
            name: name.into(),
            passed: BTreeSet::new(),
            published_grades: BTreeMap::new(),
        }
    }

    pub fn passing(mut self, gate: ReleaseGate) -> Self {
        self.passed.insert(gate);
        self
    }

    pub fn publishing(mut self, adapter: impl Into<String>, grade: Grade) -> Self {
        self.published_grades.insert(adapter.into(), grade);
        self
    }

    /// 23.29: "No public stable release until: ..." — the gates still blocking this candidate.
    ///
    /// A release naming adapters without stating a grade for each fails
    /// [`ReleaseGate::AdapterLossAndTrustGradesVisible`] regardless of what the candidate claims,
    /// because that gate is checkable from the release's own contents.
    pub fn blocking(&self, adapters_shipped: &BTreeSet<String>) -> BTreeSet<ReleaseGate> {
        let mut blocking: BTreeSet<ReleaseGate> = ReleaseGate::ALL
            .into_iter()
            .filter(|gate| !self.passed.contains(gate))
            .collect();
        if adapters_shipped
            .iter()
            .any(|adapter| !self.published_grades.contains_key(adapter))
        {
            blocking.insert(ReleaseGate::AdapterLossAndTrustGradesVisible);
        }
        blocking
    }

    pub fn releasable(&self, adapters_shipped: &BTreeSet<String>) -> bool {
        self.blocking(adapters_shipped).is_empty()
    }
}

/// 23.29's twelve red-team exercises, recorded and not executed.
pub const RED_TEAM_EXERCISES: [&str; 12] = [
    "malicious agent cards",
    "signed but misleading capability claims",
    "cross-message prompt injection",
    "hidden delegation",
    "stale revocation",
    "artifact-hash confusion",
    "trace poisoning",
    "denial-of-wallet attacks",
    "false quorum through sybils",
    "malicious verifier",
    "schema smuggling",
    "covert data leakage",
];
