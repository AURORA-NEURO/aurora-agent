//! Agent identity, attested capability, and contextual reputation.
//!
//! Blueprint 23.46. Also the identity half of 23.12's binding gate, since 23.46 is where the gates
//! are actually enumerated.
//!
//! # The three things this module refuses to let you write
//!
//! **A reputation score that spans contexts.** 23.46: "Do not publish one global reputation
//! score." That is a sentence in a document; here it is the absence of a method. [`Reputation`]
//! has no `overall()`, no `mean()`, no `Ord`, and no iterator that yields bare scores without
//! their [`ReputationContext`]. [`Reputation::lookup`] takes a full context and returns
//! [`ContextLookup::Unmeasured`] when nothing was measured *in that context* — it will not fall
//! back to a neighbouring capability dimension, a wider time window or an older software version,
//! because "an agent excellent at literature retrieval may be untested for code modification" and
//! a lookup that quietly answers from the wrong cell is how that sentence gets violated by an
//! implementation that quotes it in its documentation.
//!
//! **Unmeasured collapsed into measured-and-poor.** `bioprism-atlas` established the rule for
//! capability cells and it applies unchanged to reputation: [`ContextLookup::Unmeasured`] carries
//! no number, and [`ContextualScore`] cannot be constructed with a zero denominator. A context
//! nobody probed must not render as a context where the agent failed.
//!
//! **A claimed capability that reads as an attested one.** [`CapabilityAssertion`] is an enum with
//! two variants and no `Deref`, no `From<SelfDeclaration> for Attestation`, and no accessor that
//! returns the claim text without the variant. [`CapabilityCard`] records each rung of 23.46's
//! evidence ladder independently — "The card records each layer independently" — rather than
//! storing one maximum rung, so an agent that is self-declared *and* fixture-verified and
//! *not* PRISM-evaluated shows exactly that.
//!
//! # Not implemented
//!
//! No signatures, no certificates, no revocation registry fetch, no DNS, no clock. Validity
//! windows are logical and every query takes an explicit `as_of`, which is what makes an expiry
//! test reproducible. Sybil detection here is lineage bookkeeping over what a caller already
//! knows; it cannot discover that two endpoints are the same machine.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A logical time. Not a wall clock: 23.46's `issued`/`expires` are timestamps, and a crate that
/// resolved them against the host clock would produce a different answer on every run.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct LogicalTime(pub u64);

/// 23.46's seven identity layers, each independently `Option` because "These layers may differ"
/// and a card that fuses them cannot express a hosted endpoint changing model version behind a
/// stable DNS name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityLayers {
    pub principal: Option<String>,
    pub endpoint: Option<String>,
    pub runtime: Option<String>,
    pub model: Option<String>,
    pub organization: Option<String>,
    pub session: Option<String>,
    pub molecule: Option<String>,
}

/// Which identity layer a name refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentityLayer {
    Principal,
    Endpoint,
    Runtime,
    Model,
    Organization,
    Session,
    Molecule,
}

impl IdentityLayers {
    pub fn new() -> Self {
        IdentityLayers::default()
    }

    pub fn principal(mut self, value: impl Into<String>) -> Self {
        self.principal = Some(value.into());
        self
    }

    pub fn endpoint(mut self, value: impl Into<String>) -> Self {
        self.endpoint = Some(value.into());
        self
    }

    pub fn runtime(mut self, value: impl Into<String>) -> Self {
        self.runtime = Some(value.into());
        self
    }

    pub fn model(mut self, value: impl Into<String>) -> Self {
        self.model = Some(value.into());
        self
    }

    pub fn organization(mut self, value: impl Into<String>) -> Self {
        self.organization = Some(value.into());
        self
    }

    fn get(&self, layer: IdentityLayer) -> Option<&String> {
        match layer {
            IdentityLayer::Principal => self.principal.as_ref(),
            IdentityLayer::Endpoint => self.endpoint.as_ref(),
            IdentityLayer::Runtime => self.runtime.as_ref(),
            IdentityLayer::Model => self.model.as_ref(),
            IdentityLayer::Organization => self.organization.as_ref(),
            IdentityLayer::Session => self.session.as_ref(),
            IdentityLayer::Molecule => self.molecule.as_ref(),
        }
    }

    /// Layers that changed between two observations of what a caller believes is the same
    /// participant. 23.46's worked case: the endpoint is stable, the model is not, "that change
    /// must remain observable".
    pub fn drifted(&self, later: &IdentityLayers) -> BTreeSet<IdentityLayer> {
        [
            IdentityLayer::Principal,
            IdentityLayer::Endpoint,
            IdentityLayer::Runtime,
            IdentityLayer::Model,
            IdentityLayer::Organization,
            IdentityLayer::Session,
            IdentityLayer::Molecule,
        ]
        .into_iter()
        .filter(|layer| self.get(*layer) != later.get(*layer))
        .collect()
    }
}

/// 23.46's capability evidence ladder, ordered weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLayer {
    /// "The provider claims an interface or skill. Useful for discovery, never sufficient for
    /// high-assurance binding."
    SelfDeclared,
    SchemaConformant,
    FixtureVerified,
    PrismEvaluated,
    OperationallyObserved,
    IndependentlyAttested,
}

impl EvidenceLayer {
    pub const LADDER: [EvidenceLayer; 6] = [
        EvidenceLayer::SelfDeclared,
        EvidenceLayer::SchemaConformant,
        EvidenceLayer::FixtureVerified,
        EvidenceLayer::PrismEvaluated,
        EvidenceLayer::OperationallyObserved,
        EvidenceLayer::IndependentlyAttested,
    ];

    /// Whether evidence at this rung was produced by somebody other than the subject.
    pub fn is_third_party(&self) -> bool {
        !matches!(self, EvidenceLayer::SelfDeclared)
    }
}

/// A capability the provider asserts about itself. Carries no evidence by construction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfDeclaration {
    pub subject: String,
    pub claim: String,
}

/// Evidence produced by an issuer other than the subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    pub subject: String,
    pub claim: String,
    pub issuer: String,
    pub layer: EvidenceLayer,
    /// 23.46's `evidence:` block: opaque digests of a PRISM profile, benchmark pack, conformance
    /// report and runtime build. Opaque because this crate resolves none of them.
    pub evidence: BTreeMap<String, String>,
    pub validity: ValidityWindow,
    pub scope: AttestationScope,
    pub revoked: Option<RevocationReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidityWindow {
    pub issued: LogicalTime,
    pub expires: LogicalTime,
}

impl ValidityWindow {
    pub fn new(issued: u64, expires: u64) -> Self {
        ValidityWindow {
            issued: LogicalTime(issued),
            expires: LogicalTime(expires),
        }
    }

    pub fn contains(&self, at: LogicalTime) -> bool {
        at >= self.issued && at < self.expires
    }
}

/// 23.46's `scope:` block.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationScope {
    pub effects: BTreeSet<String>,
    pub data_labels: BTreeSet<String>,
}

impl Attestation {
    pub fn new(
        subject: impl Into<String>,
        claim: impl Into<String>,
        issuer: impl Into<String>,
        layer: EvidenceLayer,
        validity: ValidityWindow,
    ) -> Result<Self, ReputationError> {
        let issuer = issuer.into();
        let subject = subject.into();
        if issuer == subject {
            return Err(ReputationError::SelfIssuedAttestation { subject });
        }
        Ok(Attestation {
            subject,
            claim: claim.into(),
            issuer,
            layer,
            evidence: BTreeMap::new(),
            validity,
            scope: AttestationScope::default(),
            revoked: None,
        })
    }

    pub fn with_evidence(mut self, key: impl Into<String>, digest: impl Into<String>) -> Self {
        self.evidence.insert(key.into(), digest.into());
        self
    }

    pub fn scoped_to_effect(mut self, effect: impl Into<String>) -> Self {
        self.scope.effects.insert(effect.into());
        self
    }

    pub fn revoked_for(mut self, reason: RevocationReason) -> Self {
        self.revoked = Some(reason);
        self
    }

    /// Whether this attestation may be relied on at `as_of`.
    pub fn status(&self, as_of: LogicalTime) -> AttestationStatus {
        if let Some(reason) = self.revoked {
            return AttestationStatus::Revoked(reason);
        }
        if !self.validity.contains(as_of) {
            return AttestationStatus::Expired {
                expired_at: self.validity.expires,
            };
        }
        AttestationStatus::Valid
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum AttestationStatus {
    Valid,
    Expired { expired_at: LogicalTime },
    Revoked(RevocationReason),
}

/// 23.46's seven grounds for revocation or downgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevocationReason {
    ConfigurationChange,
    SecurityIncident,
    BenchmarkExploit,
    CalibrationDrift,
    PolicyViolation,
    StaleEvaluation,
    LostCredential,
}

/// What a role binding may rely on. The two variants never converge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "assertion")]
pub enum CapabilityAssertion {
    Claimed(SelfDeclaration),
    Attested(Attestation),
}

impl CapabilityAssertion {
    pub fn subject(&self) -> &str {
        match self {
            CapabilityAssertion::Claimed(d) => &d.subject,
            CapabilityAssertion::Attested(a) => &a.subject,
        }
    }

    pub fn claim(&self) -> &str {
        match self {
            CapabilityAssertion::Claimed(d) => &d.claim,
            CapabilityAssertion::Attested(a) => &a.claim,
        }
    }

    /// The rung this assertion sits on. A self-declaration is always
    /// [`EvidenceLayer::SelfDeclared`]; there is no path by which it becomes anything else.
    pub fn layer(&self) -> EvidenceLayer {
        match self {
            CapabilityAssertion::Claimed(_) => EvidenceLayer::SelfDeclared,
            CapabilityAssertion::Attested(a) => a.layer,
        }
    }
}

/// One participant's capability card: every rung of the ladder, held separately.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityCard {
    pub subject: String,
    pub identity: IdentityLayers,
    pub declared: Vec<SelfDeclaration>,
    /// Keyed by rung so a card cannot silently report its maximum as though the intermediate rungs
    /// were also satisfied.
    pub attested: BTreeMap<EvidenceLayer, Vec<Attestation>>,
    /// A shared runtime lineage identifier where known. Two cards with the same lineage are not
    /// independent evidence sources; see [`independent_subjects`].
    pub lineage: Option<String>,
}

impl CapabilityCard {
    pub fn new(subject: impl Into<String>) -> Self {
        CapabilityCard {
            subject: subject.into(),
            identity: IdentityLayers::new(),
            declared: Vec::new(),
            attested: BTreeMap::new(),
            lineage: None,
        }
    }

    pub fn with_identity(mut self, identity: IdentityLayers) -> Self {
        self.identity = identity;
        self
    }

    pub fn declaring(mut self, claim: impl Into<String>) -> Self {
        self.declared.push(SelfDeclaration {
            subject: self.subject.clone(),
            claim: claim.into(),
        });
        self
    }

    pub fn attesting(mut self, attestation: Attestation) -> Result<Self, ReputationError> {
        if attestation.subject != self.subject {
            return Err(ReputationError::AttestationSubjectMismatch {
                card: self.subject.clone(),
                attestation: attestation.subject,
            });
        }
        self.attested
            .entry(attestation.layer)
            .or_default()
            .push(attestation);
        Ok(self)
    }

    pub fn with_lineage(mut self, lineage: impl Into<String>) -> Self {
        self.lineage = Some(lineage.into());
        self
    }

    /// The highest rung with at least one valid attestation for `claim` at `as_of`.
    ///
    /// `None` means no third-party evidence, whatever the card declares about itself. A
    /// self-declaration never appears here.
    pub fn highest_valid_rung(&self, claim: &str, as_of: LogicalTime) -> Option<EvidenceLayer> {
        self.attested
            .iter()
            .rev()
            .find(|(_, attestations)| {
                attestations
                    .iter()
                    .any(|a| a.claim == claim && a.status(as_of) == AttestationStatus::Valid)
            })
            .map(|(layer, _)| *layer)
    }

    /// Every rung's status for one claim, so a reader sees the ladder rather than its maximum.
    pub fn ladder(&self, claim: &str, as_of: LogicalTime) -> BTreeMap<EvidenceLayer, RungStatus> {
        let mut out = BTreeMap::new();
        for layer in EvidenceLayer::LADDER {
            let attestations: Vec<&Attestation> = self
                .attested
                .get(&layer)
                .map(|v| v.iter().filter(|a| a.claim == claim).collect())
                .unwrap_or_default();
            let status = if attestations.is_empty() {
                if layer == EvidenceLayer::SelfDeclared
                    && self.declared.iter().any(|d| d.claim == claim)
                {
                    RungStatus::SelfDeclaredOnly
                } else {
                    RungStatus::NoEvidence
                }
            } else if attestations
                .iter()
                .any(|a| a.status(as_of) == AttestationStatus::Valid)
            {
                RungStatus::Valid
            } else {
                RungStatus::AllInvalid
            };
            out.insert(layer, status);
        }
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RungStatus {
    /// Nobody produced evidence at this rung. Not a failure at this rung.
    NoEvidence,
    /// The subject asserted it and nobody checked.
    SelfDeclaredOnly,
    /// Evidence exists at this rung and at least one piece is valid now.
    Valid,
    /// Evidence exists and every piece is expired or revoked.
    AllInvalid,
}

/// 23.46's nine reputation index dimensions, as a compound key.
///
/// Every field participates in equality. That is the mechanism: a measurement recorded against
/// `capability=literature.retrieve, effect_risk=E0` cannot be found by a query for
/// `capability=code.modify, effect_risk=E3`, so "past performance under read-only access does not
/// establish safety with production credentials" is enforced by `BTreeMap` lookup rather than by
/// reviewer discipline.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ReputationContext {
    pub capability: String,
    pub task_domain: String,
    pub effect_risk: crate::effect::Irreversibility,
    pub data_classification: crate::flow::Sensitivity,
    pub evaluator: String,
    pub benchmark_version: String,
    pub window: WindowKey,
    pub software_version: String,
    pub deployment: DeploymentContext,
}

/// The time window a measurement covers, as an inclusive-exclusive pair of logical times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WindowKey {
    pub from: LogicalTime,
    pub to: LogicalTime,
}

impl WindowKey {
    pub fn new(from: u64, to: u64) -> Self {
        WindowKey {
            from: LogicalTime(from),
            to: LogicalTime(to),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentContext {
    PublicBenchmark,
    HiddenBenchmark,
    Operational,
}

/// A measured score inside one context.
///
/// Constructed through [`ContextualScore::new`], which refuses a zero denominator. There is no
/// public field and no `Deref<Target = f64>`: the score is inseparable from its sample size, which
/// is what stops it being averaged with a score from somewhere else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextualScore {
    successes: u32,
    trials: u32,
}

impl ContextualScore {
    pub fn new(successes: u32, trials: u32) -> Result<Self, ReputationError> {
        if trials == 0 {
            return Err(ReputationError::EmptyDenominator);
        }
        if successes > trials {
            return Err(ReputationError::SuccessesExceedTrials { successes, trials });
        }
        Ok(ContextualScore { successes, trials })
    }

    pub fn successes(&self) -> u32 {
        self.successes
    }

    pub fn trials(&self) -> u32 {
        self.trials
    }

    /// The point estimate in basis points. Integer, so two runs cannot disagree.
    pub fn rate_bp(&self) -> u32 {
        ((self.successes as u64 * 10_000) / self.trials as u64) as u32
    }

    /// A conservative lower bound in basis points, shrunk toward zero by sample size so a 1/1
    /// result does not read as certainty. Not a Clopper-Pearson interval and does not claim to be:
    /// this is `successes / (trials + 1)`, the rule-of-succession floor, which is monotone in both
    /// arguments and needs no floating point.
    pub fn lower_bound_bp(&self) -> u32 {
        ((self.successes as u64 * 10_000) / (self.trials as u64 + 1)) as u32
    }
}

/// Why a context has no measurement. Mirrors `bioprism_atlas::UnmeasuredReason` in intent: a hole
/// with a reason is auditable, a hole rendered as a zero is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum UnmeasuredReason {
    NeverProbed,
    /// Measured under a different context. Names which one, so a reader can see the near miss and
    /// decide for itself rather than have the lookup decide for it.
    MeasuredElsewhere {
        nearest: Box<ReputationContext>,
    },
    ProbeInconclusive {
        note: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "lookup")]
pub enum ContextLookup {
    Measured {
        score: ContextualScore,
    },
    /// Carries no number. There is deliberately no `unwrap_or(0)` companion.
    Unmeasured {
        reason: UnmeasuredReason,
    },
}

impl ContextLookup {
    pub fn score(&self) -> Option<ContextualScore> {
        match self {
            ContextLookup::Measured { score } => Some(*score),
            ContextLookup::Unmeasured { .. } => None,
        }
    }
}

/// A participant's reputation: a map from context to score, and nothing else.
///
/// The API surface is the point. There is no aggregate, no comparison operator between two
/// [`Reputation`] values, and no way to obtain a score without having named the context it was
/// measured in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reputation {
    subject: String,
    entries: BTreeMap<ReputationContext, ContextualScore>,
}

impl Reputation {
    pub fn new(subject: impl Into<String>) -> Self {
        Reputation {
            subject: subject.into(),
            entries: BTreeMap::new(),
        }
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn record(&mut self, context: ReputationContext, score: ContextualScore) {
        self.entries.insert(context, score);
    }

    /// Exact-context lookup. No fallback, no nearest-neighbour, no widening.
    pub fn lookup(&self, context: &ReputationContext) -> ContextLookup {
        match self.entries.get(context) {
            Some(score) => ContextLookup::Measured { score: *score },
            None => ContextLookup::Unmeasured {
                reason: match self
                    .entries
                    .keys()
                    .find(|k| k.capability == context.capability)
                {
                    Some(nearest) => UnmeasuredReason::MeasuredElsewhere {
                        nearest: Box::new(nearest.clone()),
                    },
                    None => UnmeasuredReason::NeverProbed,
                },
            },
        }
    }

    /// Contexts with a measurement, each paired with its context. Never bare scores.
    pub fn measured(&self) -> impl Iterator<Item = (&ReputationContext, &ContextualScore)> {
        self.entries.iter()
    }

    pub fn measured_context_count(&self) -> usize {
        self.entries.len()
    }
}

/// What a role demands of a candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleRequirement {
    pub capability: String,
    pub minimum_rung: EvidenceLayer,
    /// In basis points, compared against [`ContextualScore::lower_bound_bp`].
    pub minimum_lower_bound_bp: Option<u32>,
    pub context: ReputationContext,
    pub required_effects: BTreeSet<String>,
    pub permitted_organizations: BTreeSet<String>,
    /// Lineages already bound in this thread. A candidate sharing one is a correlated-failure risk
    /// and 23.46's binding policy lists it as a gate.
    pub bound_lineages: BTreeSet<String>,
}

/// A gate that failed, named. 23.46's binding policy in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "gate")]
pub enum FailedGate {
    NoAttestationForCapability {
        capability: String,
    },
    RungTooLow {
        required: EvidenceLayer,
        offered: EvidenceLayer,
    },
    /// The candidate has never been measured in the demanded context. Distinct from
    /// [`FailedGate::BelowLowerBound`].
    Unmeasured {
        reason: UnmeasuredReason,
    },
    BelowLowerBound {
        required: u32,
        offered: u32,
    },
    EffectOutsideAttestedScope {
        effects: BTreeSet<String>,
    },
    OrganizationNotPermitted {
        organization: Option<String>,
    },
    AttestationExpired {
        expired_at: LogicalTime,
    },
    AttestationRevoked {
        reason: RevocationReason,
    },
    LineageAlreadyBound {
        lineage: String,
    },
}

/// The outcome of 23.46's binding policy.
///
/// `Candidate` exists because "Self-advertised capability may generate a candidate but cannot
/// override a failed gate": a self-declaration is enough to *appear*, never enough to *bind*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "decision")]
pub enum BindingDecision {
    Bound {
        rung: EvidenceLayer,
        lower_bound_bp: u32,
    },
    /// Discoverable on the strength of a self-declaration, and nothing more.
    Candidate {
        failed: Vec<FailedGate>,
    },
    Rejected {
        failed: Vec<FailedGate>,
    },
}

/// Run 23.46's binding gates against one candidate.
///
/// Gates are evaluated in a fixed order and every failure is collected, because a caller that has
/// to fix three gates should learn all three at once rather than one per round trip.
pub fn bind(
    requirement: &RoleRequirement,
    card: &CapabilityCard,
    reputation: &Reputation,
    as_of: LogicalTime,
) -> BindingDecision {
    let mut failed = Vec::new();

    let attestations: Vec<&Attestation> = card
        .attested
        .values()
        .flatten()
        .filter(|a| a.claim == requirement.capability)
        .collect();

    if attestations.is_empty() {
        failed.push(FailedGate::NoAttestationForCapability {
            capability: requirement.capability.clone(),
        });
    } else {
        for attestation in &attestations {
            match attestation.status(as_of) {
                AttestationStatus::Valid => {}
                AttestationStatus::Expired { expired_at } => {
                    failed.push(FailedGate::AttestationExpired { expired_at })
                }
                AttestationStatus::Revoked(reason) => {
                    failed.push(FailedGate::AttestationRevoked { reason })
                }
            }
        }
    }

    let rung = card.highest_valid_rung(&requirement.capability, as_of);
    match rung {
        Some(rung) if rung >= requirement.minimum_rung => {}
        Some(rung) => failed.push(FailedGate::RungTooLow {
            required: requirement.minimum_rung,
            offered: rung,
        }),
        None => {}
    }

    let lookup = reputation.lookup(&requirement.context);
    let lower_bound = match (&lookup, requirement.minimum_lower_bound_bp) {
        (ContextLookup::Measured { score }, Some(required)) => {
            let offered = score.lower_bound_bp();
            if offered < required {
                failed.push(FailedGate::BelowLowerBound { required, offered });
            }
            offered
        }
        (ContextLookup::Measured { score }, None) => score.lower_bound_bp(),
        (ContextLookup::Unmeasured { reason }, Some(_)) => {
            failed.push(FailedGate::Unmeasured {
                reason: reason.clone(),
            });
            0
        }
        (ContextLookup::Unmeasured { .. }, None) => 0,
    };

    let attested_effects: BTreeSet<String> = attestations
        .iter()
        .filter(|a| a.status(as_of) == AttestationStatus::Valid)
        .flat_map(|a| a.scope.effects.iter().cloned())
        .collect();
    let outside: BTreeSet<String> = requirement
        .required_effects
        .difference(&attested_effects)
        .cloned()
        .collect();
    if !outside.is_empty() {
        failed.push(FailedGate::EffectOutsideAttestedScope { effects: outside });
    }

    if !requirement.permitted_organizations.is_empty() {
        let permitted = card
            .identity
            .organization
            .as_ref()
            .map(|org| requirement.permitted_organizations.contains(org))
            .unwrap_or(false);
        if !permitted {
            failed.push(FailedGate::OrganizationNotPermitted {
                organization: card.identity.organization.clone(),
            });
        }
    }

    if let Some(lineage) = &card.lineage {
        if requirement.bound_lineages.contains(lineage) {
            failed.push(FailedGate::LineageAlreadyBound {
                lineage: lineage.clone(),
            });
        }
    }

    if failed.is_empty() {
        BindingDecision::Bound {
            rung: rung.unwrap_or(EvidenceLayer::SelfDeclared),
            lower_bound_bp: lower_bound,
        }
    } else if card
        .declared
        .iter()
        .any(|d| d.claim == requirement.capability)
    {
        BindingDecision::Candidate { failed }
    } else {
        BindingDecision::Rejected { failed }
    }
}

/// The subjects among `cards` that are independent evidence sources.
///
/// One representative per known lineage; cards with no lineage are each their own, because an
/// unknown lineage is not a shared one. 23.46 asks the registry to "group evaluations by
/// underlying runtime lineage where known" — *where known* is the load-bearing clause and this
/// function cannot discover a lineage nobody recorded.
pub fn independent_subjects(cards: &[CapabilityCard]) -> BTreeSet<String> {
    let mut seen_lineages = BTreeSet::new();
    let mut out = BTreeSet::new();
    for card in cards {
        match &card.lineage {
            Some(lineage) => {
                if seen_lineages.insert(lineage.clone()) {
                    out.insert(card.subject.clone());
                }
            }
            None => {
                out.insert(card.subject.clone());
            }
        }
    }
    out
}

/// A revocation delivered to an in-flight thread, and what policy says to do about it (23.46:
/// "Active threads receive a typed revocation event and rebind, reduce authority, or stop").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevocationEvent {
    pub subject: String,
    pub claim: String,
    pub reason: RevocationReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevocationResponse {
    Rebind,
    ReduceAuthority,
    Stop,
}

impl RevocationEvent {
    /// The response this crate recommends. A security incident or policy violation stops the
    /// thread; a stale evaluation or configuration change is a rebind; drift and exploits reduce
    /// authority pending re-evaluation. **23.46 does not state this mapping**; it lists the reasons
    /// and lists the responses and leaves the relation to the reader, so this is a reading.
    pub fn recommended_response(&self) -> RevocationResponse {
        match self.reason {
            RevocationReason::SecurityIncident | RevocationReason::PolicyViolation => {
                RevocationResponse::Stop
            }
            RevocationReason::StaleEvaluation
            | RevocationReason::ConfigurationChange
            | RevocationReason::LostCredential => RevocationResponse::Rebind,
            RevocationReason::CalibrationDrift | RevocationReason::BenchmarkExploit => {
                RevocationResponse::ReduceAuthority
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ReputationError {
    #[error("a contextual score needs at least one trial; zero trials is unmeasured, not zero")]
    EmptyDenominator,

    #[error("{successes} successes in {trials} trials")]
    SuccessesExceedTrials { successes: u32, trials: u32 },

    #[error("{subject} cannot attest to itself; that is a self-declaration")]
    SelfIssuedAttestation { subject: String },

    #[error("card for {card} cannot hold an attestation about {attestation}")]
    AttestationSubjectMismatch { card: String, attestation: String },
}
