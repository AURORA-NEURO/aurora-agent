//! The typed threat model: assets, adversaries, attack surfaces, and mitigations that say whether
//! anything applies them.
//!
//! Implements blueprint 13.01 (security architecture and threat model) and 13.02 (threat model),
//! whose asset, adversary, trust-zone and attack-class lists are transcribed here as closed enums
//! so that a threat record cannot name an asset the model has never heard of.
//!
//! # The one rule this module exists to enforce
//!
//! 13.01 asks the platform to "map threats to preventative, detective, and recovery controls" and
//! to "document residual risk". The natural implementation records a mitigation as a string and
//! reports the threat as handled. That is how a threat model becomes decorative: every row has an
//! entry in the mitigation column and nobody can tell which entries are load-bearing.
//!
//! So [`Mitigation`] has three states and only one of them counts. A threat whose mitigations are
//! all [`Mitigation::DeclaredOnly`] reports [`ThreatStatus::DeclaredOnly`], never
//! [`ThreatStatus::Mitigated`], and [`Threat::rely`] returns
//! [`SafetyError::UnenforcedReliance`](crate::SafetyError::UnenforcedReliance) so that relying on
//! it is a compile-visible `?` rather than an assumption. This is the same rule the capability
//! atlas enforces over measurement — unmeasured is not zero — in the currency of controls.
//!
//! # Why `Enforcer` has one variant
//!
//! [`Mitigation::Enforced`] carries an [`Enforcer`], and `Enforcer` can name exactly one thing:
//! [`Enforcer::Unrepresentable`], a state this workspace made impossible to construct in Rust.
//! There is deliberately no `Enforcer::Sandbox`, no `Enforcer::Kernel`, no `Enforcer::ControlPlane`
//! and no `Enforcer::Signature`, because none of those run in this process and a variant naming one
//! would let a threat record claim containment the workspace does not have. This mirrors
//! `bioprism_sdk::sandbox::Enforcement`, which has one variant for the same reason.
//!
//! The consequence is severe and correct: on the shipped model in [`crate::model`], **most threats
//! in section 13 are `DeclaredOnly`**. A pure-Rust, single-process, no-network library mitigates
//! almost nothing. Saying so is the deliverable.
//!
//! # What is not modelled here
//!
//! No likelihood, no impact score, no risk matrix, no CVSS. 13.02 asks to "prioritize mitigations"
//! and this module declines: a number multiplied by a guess is a guess with a decimal point, and
//! ranking threats by it hides which ones have no control at all. [`ThreatModel::residual`] returns
//! the threats, in declaration order, and the operator prioritises.

use crate::error::SafetyError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A protected asset, from 13.01's "Protected assets" and 13.02's "Assets" lists.
///
/// Closed on purpose. A threat naming an asset outside this list is a threat against something the
/// architecture never claimed to protect, and the fix is to amend the model, visibly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Asset {
    /// Publisher and organisation identities.
    PublisherIdentity,
    /// Pack signing keys.
    SigningKey,
    /// Model-provider and cloud credentials.
    ProviderCredential,
    /// Hidden oracles, holdouts and private benchmark state.
    HiddenOracle,
    /// The claim that a published number came from the run it says it came from.
    ResultIntegrity,
    /// Host and worker infrastructure.
    WorkerFleet,
    /// The public registry and everything downstream of it.
    RegistrySupplyChain,
    /// User traces, prompts, incidents and proprietary code.
    PrivateTenantData,
    /// PHI and PII inside research datasets. Governance of this asset belongs to
    /// `bioprism-policy`; it is listed here because 13.02 lists it as an asset and a threat model
    /// that omitted it would be incomplete.
    ProtectedHealthInformation,
    /// The developer's own machine, running a local pack.
    UserMachine,
}

impl Asset {
    pub fn as_str(self) -> &'static str {
        match self {
            Asset::PublisherIdentity => "publisher_identity",
            Asset::SigningKey => "signing_key",
            Asset::ProviderCredential => "provider_credential",
            Asset::HiddenOracle => "hidden_oracle",
            Asset::ResultIntegrity => "result_integrity",
            Asset::WorkerFleet => "worker_fleet",
            Asset::RegistrySupplyChain => "registry_supply_chain",
            Asset::PrivateTenantData => "private_tenant_data",
            Asset::ProtectedHealthInformation => "protected_health_information",
            Asset::UserMachine => "user_machine",
        }
    }
}

impl fmt::Display for Asset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What an adversary can *do*, rather than who they are.
///
/// 13.01 and 13.02 both enumerate adversaries by role ("malicious pack publisher", "insider").
/// Roles are not composable and two roles with the same capabilities produce the same attacks, so
/// [`Adversary`] is a named bag of these capabilities instead. The blueprint's role list survives
/// as the shipped adversaries in [`crate::model`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Can author content the platform will ingest: a pack, a task, a document, a mutation.
    AuthorsContent,
    /// Can get code executed inside the agent sandbox — this is the benchmarked agent itself.
    ExecutesInAgentSandbox,
    /// Can get code executed inside the evaluator sandbox: a malicious grader or a backdoored
    /// oracle.
    ExecutesInEvaluatorSandbox,
    /// Controls a dependency, base image, or build step the platform consumes.
    ControlsBuildInput,
    /// Submits results for publication.
    SubmitsResults,
    /// Holds a valid credential for some part of the system: an insider or a compromised account.
    HoldsCredential,
    /// Controls a message from a peer agent in a multi-agent run.
    ControlsPeerMessage,
    /// Controls a remote service the platform calls: a model provider, a federation peer.
    ControlsExternalService,
    /// Sees only what is public: a leaderboard reader.
    ObservesPublicSurface,
}

impl Capability {
    pub fn as_str(self) -> &'static str {
        match self {
            Capability::AuthorsContent => "authors_content",
            Capability::ExecutesInAgentSandbox => "executes_in_agent_sandbox",
            Capability::ExecutesInEvaluatorSandbox => "executes_in_evaluator_sandbox",
            Capability::ControlsBuildInput => "controls_build_input",
            Capability::SubmitsResults => "submits_results",
            Capability::HoldsCredential => "holds_credential",
            Capability::ControlsPeerMessage => "controls_peer_message",
            Capability::ControlsExternalService => "controls_external_service",
            Capability::ObservesPublicSurface => "observes_public_surface",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named adversary with a capability set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Adversary {
    pub id: String,
    pub capabilities: BTreeSet<Capability>,
    /// One sentence on who this is, for a human reading the model.
    pub description: String,
}

impl Adversary {
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Adversary {
            id: id.into(),
            capabilities: BTreeSet::new(),
            description: description.into(),
        }
    }

    pub fn with(mut self, capability: Capability) -> Self {
        self.capabilities.insert(capability);
        self
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }

    /// Whether this adversary has every capability a threat requires.
    ///
    /// A threat requiring capabilities nobody in the model possesses is not thereby safe — see
    /// [`ThreatModel::unreachable_threats`], which reports those separately rather than dropping
    /// them.
    pub fn can_mount(&self, threat: &Threat) -> bool {
        threat
            .requires
            .iter()
            .all(|capability| self.capabilities.contains(capability))
    }
}

/// The attack classes 13.01 lists, as a closed enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttackClass {
    RemoteCodeExecution,
    SandboxEscape,
    SecretExfiltration,
    CrossTenantAccess,
    PromptOrToolInjection,
    EvaluatorTampering,
    HiddenTestDiscovery,
    ArtifactSubstitution,
    SignatureAbuse,
    DenialOfService,
    SupplyChainCompromise,
    PrivacyLeakage,
    LeaderboardFraud,
    /// Not one of 13.01's classes. It comes from 13.24's scope list, which counts a "misleading
    /// security claim" as a vulnerability, and it belongs here because on this platform the claim
    /// *is* the product: a threat model asserting a control nobody applies does more damage than
    /// the absent control alone, since it stops anyone looking for the gap.
    MisleadingSecurityClaim,
}

impl AttackClass {
    pub fn as_str(self) -> &'static str {
        match self {
            AttackClass::RemoteCodeExecution => "remote_code_execution",
            AttackClass::SandboxEscape => "sandbox_escape",
            AttackClass::SecretExfiltration => "secret_exfiltration",
            AttackClass::CrossTenantAccess => "cross_tenant_access",
            AttackClass::PromptOrToolInjection => "prompt_or_tool_injection",
            AttackClass::EvaluatorTampering => "evaluator_tampering",
            AttackClass::HiddenTestDiscovery => "hidden_test_discovery",
            AttackClass::ArtifactSubstitution => "artifact_substitution",
            AttackClass::SignatureAbuse => "signature_abuse",
            AttackClass::DenialOfService => "denial_of_service",
            AttackClass::SupplyChainCompromise => "supply_chain_compromise",
            AttackClass::PrivacyLeakage => "privacy_leakage",
            AttackClass::LeaderboardFraud => "leaderboard_fraud",
            AttackClass::MisleadingSecurityClaim => "misleading_security_claim",
        }
    }
}

impl fmt::Display for AttackClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A rule this workspace made unrepresentable in the Rust type system.
///
/// The only kind of enforcement a library of plain types can perform, and — because the compiler
/// runs before the program does — the only kind whose operation this crate can honestly witness.
/// Each variant names a specific type whose shape rules a state out, and each is held by a test in
/// the crate that owns it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unrepresentable {
    /// `bioprism_sdk::sandbox::Enforcement` has no `Enforced` variant, so no plugin registration
    /// can record that isolation was applied. 40.16's fourth invariant stays a request.
    NoValueClaimsIsolationWasApplied,
    /// [`Enforcer`] itself names no runtime component, so no threat record in this crate can claim
    /// a control that does not run here.
    NoValueNamesARuntimeEnforcer,
    /// [`crate::attest::Statement`] separates `Observed` from `Asserted`, and the `Observed`
    /// payload is a closed enum of computations this process performs, so an assertion cannot be
    /// filed as an observation.
    NoAssertionIsFiledAsAnObservation,
    /// [`crate::supply::SignatureStatus`] has one variant, so no artifact record can say a
    /// signature verified.
    NoValueClaimsASignatureVerified,
    /// [`crate::boundary::TenantIsolation`] has one variant, so no record can say a tenant was
    /// isolated by anything but declaration.
    NoValueClaimsTenantIsolationWasApplied,
    /// [`crate::incident::ContainmentReport`] has private fields, no public constructor and no
    /// `Deserialize`, so no containment claim exists that did not pass
    /// [`crate::incident::Incident::report_contained`]'s blast-radius gate.
    NoContainmentReportExistsWithoutACompleteBlastRadius,
    /// [`crate::boundary::Crossing`] is sealed the same way, so no crossing record exists for a
    /// movement [`crate::boundary::BoundaryModel::deliver`] refused.
    NoCrossingRecordExistsThatTheModelForbids,
    /// [`crate::attest::Attestation`] is sealed the same way, so no attestation pairs a claim this
    /// process cannot witness with an `Observed` statement.
    NoAttestationClaimsObservationWithoutOne,
}

impl Unrepresentable {
    pub fn as_str(self) -> &'static str {
        match self {
            Unrepresentable::NoValueClaimsIsolationWasApplied => {
                "no_value_claims_isolation_was_applied"
            }
            Unrepresentable::NoValueNamesARuntimeEnforcer => "no_value_names_a_runtime_enforcer",
            Unrepresentable::NoAssertionIsFiledAsAnObservation => {
                "no_assertion_is_filed_as_an_observation"
            }
            Unrepresentable::NoValueClaimsASignatureVerified => {
                "no_value_claims_a_signature_verified"
            }
            Unrepresentable::NoValueClaimsTenantIsolationWasApplied => {
                "no_value_claims_tenant_isolation_was_applied"
            }
            Unrepresentable::NoContainmentReportExistsWithoutACompleteBlastRadius => {
                "no_containment_report_exists_without_a_complete_blast_radius"
            }
            Unrepresentable::NoCrossingRecordExistsThatTheModelForbids => {
                "no_crossing_record_exists_that_the_model_forbids"
            }
            Unrepresentable::NoAttestationClaimsObservationWithoutOne => {
                "no_attestation_claims_observation_without_one"
            }
        }
    }
}

impl fmt::Display for Unrepresentable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who applies a mitigation. Exactly one answer is available. See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Enforcer {
    /// The Rust type system, by making an illegal state impossible to construct.
    Unrepresentable(Unrepresentable),
}

impl fmt::Display for Enforcer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Enforcer::Unrepresentable(state) => write!(f, "rust-type-system({state})"),
        }
    }
}

/// Why a mitigation is missing.
///
/// [`AbsenceReason::NotAnalysed`] is categorically different from the others and must never be
/// reported as an accepted risk: nobody looked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum AbsenceReason {
    /// The control belongs to infrastructure this workspace does not include: a hypervisor, a
    /// kernel, a container runtime, a network.
    RequiresAbsentInfrastructure { component: String },
    /// Another crate owns it. Naming the owner keeps the gap from being double-counted.
    OwnedElsewhere { owner: String },
    /// A human process, not code: a review, a campaign, a notification.
    HumanProcess { process: String },
    /// Analysed, understood, and accepted by a named party at a named epoch.
    Accepted {
        by: String,
        epoch: u64,
        note: String,
    },
    /// Nobody looked. Never counts as handled.
    NotAnalysed,
}

impl AbsenceReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            AbsenceReason::RequiresAbsentInfrastructure { .. } => "requires_absent_infrastructure",
            AbsenceReason::OwnedElsewhere { .. } => "owned_elsewhere",
            AbsenceReason::HumanProcess { .. } => "human_process",
            AbsenceReason::Accepted { .. } => "accepted",
            AbsenceReason::NotAnalysed => "not_analysed",
        }
    }

    /// Whether an operator has looked at this gap and signed for it.
    pub fn is_accepted(&self) -> bool {
        matches!(self, AbsenceReason::Accepted { .. })
    }
}

impl fmt::Display for AbsenceReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What role a control plays. 13.01 asks for all three; the distinction matters because a detective
/// control never prevents anything and must not be counted as if it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlRole {
    Preventative,
    Detective,
    Recovery,
}

impl ControlRole {
    pub fn as_str(self) -> &'static str {
        match self {
            ControlRole::Preventative => "preventative",
            ControlRole::Detective => "detective",
            ControlRole::Recovery => "recovery",
        }
    }
}

impl fmt::Display for ControlRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A control, and whether anything applies it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Mitigation {
    /// Applied, by the one enforcer available. See [`Enforcer`].
    Enforced {
        name: String,
        role: ControlRole,
        by: Enforcer,
    },
    /// Written down, applied by nothing in this process. The honest state of nearly every control
    /// in section 13 as far as this workspace is concerned.
    DeclaredOnly {
        name: String,
        role: ControlRole,
        /// Where the declaration lives, so a reader can go and check it: a blueprint module id, a
        /// deployment runbook, a manifest field.
        declared_in: String,
    },
    /// Not present at all.
    Absent {
        name: String,
        role: ControlRole,
        reason: AbsenceReason,
    },
}

impl Mitigation {
    pub fn enforced(name: impl Into<String>, role: ControlRole, by: Unrepresentable) -> Self {
        Mitigation::Enforced {
            name: name.into(),
            role,
            by: Enforcer::Unrepresentable(by),
        }
    }

    pub fn declared(
        name: impl Into<String>,
        role: ControlRole,
        declared_in: impl Into<String>,
    ) -> Self {
        Mitigation::DeclaredOnly {
            name: name.into(),
            role,
            declared_in: declared_in.into(),
        }
    }

    pub fn absent(name: impl Into<String>, role: ControlRole, reason: AbsenceReason) -> Self {
        Mitigation::Absent {
            name: name.into(),
            role,
            reason,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Mitigation::Enforced { name, .. }
            | Mitigation::DeclaredOnly { name, .. }
            | Mitigation::Absent { name, .. } => name,
        }
    }

    pub fn role(&self) -> ControlRole {
        match self {
            Mitigation::Enforced { role, .. }
            | Mitigation::DeclaredOnly { role, .. }
            | Mitigation::Absent { role, .. } => *role,
        }
    }

    /// The only predicate that may gate a "this is handled" claim.
    ///
    /// A detective [`Mitigation::Enforced`] is effective *at detecting*, and callers that need
    /// prevention should also check [`Mitigation::role`]; [`Threat::prevented`] does exactly that.
    pub fn is_effective(&self) -> bool {
        matches!(self, Mitigation::Enforced { .. })
    }

    /// The sentence to print beside this control wherever it is displayed.
    pub fn honest_label(&self) -> String {
        match self {
            Mitigation::Enforced { name, by, .. } => {
                format!("{name}: enforced by {by} — the state cannot be constructed")
            }
            Mitigation::DeclaredOnly {
                name, declared_in, ..
            } => format!("{name}: declared in {declared_in}; nothing in this process applies it"),
            Mitigation::Absent { name, reason, .. } => format!("{name}: absent ({reason})"),
        }
    }
}

/// How well a threat is handled. There is no fourth state and no partial credit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreatStatus {
    /// At least one mitigation is [`Mitigation::Enforced`].
    Mitigated,
    /// Mitigations exist and every one of them is a declaration.
    DeclaredOnly,
    /// Nothing is even declared.
    Unmitigated,
}

impl ThreatStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            ThreatStatus::Mitigated => "mitigated",
            ThreatStatus::DeclaredOnly => "declared_only",
            ThreatStatus::Unmitigated => "unmitigated",
        }
    }
}

impl fmt::Display for ThreatStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One row of the threat model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Threat {
    pub id: String,
    /// The blueprint module this threat is drawn from, e.g. `"13.14"`.
    pub module: String,
    pub asset: Asset,
    pub class: AttackClass,
    /// The capabilities an adversary needs to mount it.
    pub requires: BTreeSet<Capability>,
    /// Where the attack lands, as a zone name from [`crate::boundary::TrustZone`].
    pub surface: String,
    pub narrative: String,
    pub mitigations: Vec<Mitigation>,
}

impl Threat {
    pub fn new(
        id: impl Into<String>,
        module: impl Into<String>,
        asset: Asset,
        class: AttackClass,
        surface: impl Into<String>,
        narrative: impl Into<String>,
    ) -> Self {
        Threat {
            id: id.into(),
            module: module.into(),
            asset,
            class,
            requires: BTreeSet::new(),
            surface: surface.into(),
            narrative: narrative.into(),
            mitigations: Vec::new(),
        }
    }

    pub fn requiring(mut self, capability: Capability) -> Self {
        self.requires.insert(capability);
        self
    }

    pub fn mitigated_by(mut self, mitigation: Mitigation) -> Self {
        self.mitigations.push(mitigation);
        self
    }

    /// The status, computed from the mitigations rather than stored.
    ///
    /// Stored status is how a threat model drifts: someone downgrades a control and forgets the
    /// summary row.
    pub fn status(&self) -> ThreatStatus {
        if self.mitigations.iter().any(Mitigation::is_effective) {
            ThreatStatus::Mitigated
        } else if self.mitigations.is_empty()
            || self
                .mitigations
                .iter()
                .all(|m| matches!(m, Mitigation::Absent { .. }))
        {
            ThreatStatus::Unmitigated
        } else {
            ThreatStatus::DeclaredOnly
        }
    }

    /// Whether an enforced *preventative* control exists, as opposed to an enforced detective one.
    pub fn prevented(&self) -> bool {
        self.mitigations
            .iter()
            .any(|m| m.is_effective() && m.role() == ControlRole::Preventative)
    }

    /// The gate a caller must pass before treating this threat as handled.
    ///
    /// Returns the enforced mitigations on success. On failure the error names the strongest
    /// declaration, so the message reads as "you were about to trust this sentence".
    pub fn rely(&self) -> Result<Vec<&Mitigation>, SafetyError> {
        let enforced: Vec<&Mitigation> = self
            .mitigations
            .iter()
            .filter(|m| m.is_effective())
            .collect();
        if !enforced.is_empty() {
            return Ok(enforced);
        }
        match self
            .mitigations
            .iter()
            .find(|m| matches!(m, Mitigation::DeclaredOnly { .. }))
        {
            Some(declared) => Err(SafetyError::UnenforcedReliance {
                threat: self.id.clone(),
                mitigation: declared.name().to_string(),
            }),
            None => Err(SafetyError::UnmitigatedThreat {
                threat: self.id.clone(),
            }),
        }
    }

    /// Every absence that nobody has signed for, including the ones nobody analysed.
    pub fn unaccepted_gaps(&self) -> Vec<&AbsenceReason> {
        self.mitigations
            .iter()
            .filter_map(|m| match m {
                Mitigation::Absent { reason, .. } if !reason.is_accepted() => Some(reason),
                _ => None,
            })
            .collect()
    }

    /// A residual-risk acceptance with no named party documents nothing (13.01).
    pub fn audit_acceptances(&self) -> Result<(), SafetyError> {
        for mitigation in &self.mitigations {
            if let Mitigation::Absent {
                reason: AbsenceReason::Accepted { by, .. },
                ..
            } = mitigation
            {
                if by.trim().is_empty() {
                    return Err(SafetyError::AnonymousAcceptance {
                        threat: self.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Counts over a [`ThreatModel`]. Three numbers that must never be added together.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    pub mitigated: usize,
    pub declared_only: usize,
    pub unmitigated: usize,
}

impl Coverage {
    pub fn total(&self) -> usize {
        self.mitigated + self.declared_only + self.unmitigated
    }

    /// Deliberately not a percentage of "handled" threats.
    ///
    /// A single ratio would have to decide whether a declaration counts, and either answer is
    /// wrong: counting it inflates, excluding it hides the fact that declarations are the *plan*.
    /// Callers that want a headline number should print all three.
    pub fn summary(&self) -> String {
        format!(
            "{} enforced, {} declared-only, {} unmitigated (of {})",
            self.mitigated,
            self.declared_only,
            self.unmitigated,
            self.total()
        )
    }
}

/// The whole model: adversaries and threats, with the queries a reviewer actually runs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreatModel {
    pub adversaries: Vec<Adversary>,
    pub threats: Vec<Threat>,
}

impl ThreatModel {
    pub fn new() -> Self {
        ThreatModel::default()
    }

    pub fn with_adversary(mut self, adversary: Adversary) -> Self {
        self.adversaries.push(adversary);
        self
    }

    pub fn with_threat(mut self, threat: Threat) -> Self {
        self.threats.push(threat);
        self
    }

    pub fn threat(&self, id: &str) -> Option<&Threat> {
        self.threats.iter().find(|t| t.id == id)
    }

    pub fn coverage(&self) -> Coverage {
        let mut coverage = Coverage::default();
        for threat in &self.threats {
            match threat.status() {
                ThreatStatus::Mitigated => coverage.mitigated += 1,
                ThreatStatus::DeclaredOnly => coverage.declared_only += 1,
                ThreatStatus::Unmitigated => coverage.unmitigated += 1,
            }
        }
        coverage
    }

    /// Threats not reported as [`ThreatStatus::Mitigated`], in declaration order.
    ///
    /// This is 13.01's "document residual risk", and it is a list rather than a score.
    pub fn residual(&self) -> Vec<&Threat> {
        self.threats
            .iter()
            .filter(|t| t.status() != ThreatStatus::Mitigated)
            .collect()
    }

    /// Threats no modelled adversary has the capabilities to mount.
    ///
    /// Reported, not dropped: an unreachable threat usually means the adversary list is short, and
    /// discovering that is the point of running the query.
    pub fn unreachable_threats(&self) -> Vec<&Threat> {
        self.threats
            .iter()
            .filter(|threat| !self.adversaries.iter().any(|a| a.can_mount(threat)))
            .collect()
    }

    /// Every threat a given adversary can mount, whatever its status.
    pub fn reachable_by(&self, adversary_id: &str) -> Vec<&Threat> {
        match self.adversaries.iter().find(|a| a.id == adversary_id) {
            Some(adversary) => self
                .threats
                .iter()
                .filter(|threat| adversary.can_mount(threat))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Threats against one asset.
    pub fn against(&self, asset: Asset) -> Vec<&Threat> {
        self.threats.iter().filter(|t| t.asset == asset).collect()
    }

    /// Assets no threat mentions. A short list here is suspicious, not reassuring.
    pub fn unthreatened_assets(&self, universe: &[Asset]) -> Vec<Asset> {
        universe
            .iter()
            .copied()
            .filter(|asset| !self.threats.iter().any(|t| t.asset == *asset))
            .collect()
    }

    /// Every acceptance in the model has a named accepting party.
    pub fn audit_acceptances(&self) -> Result<(), SafetyError> {
        for threat in &self.threats {
            threat.audit_acceptances()?;
        }
        Ok(())
    }

    /// Threats whose only mitigations are absences nobody analysed.
    pub fn unanalysed(&self) -> Vec<&Threat> {
        self.threats
            .iter()
            .filter(|t| {
                !t.mitigations.is_empty()
                    && t.mitigations.iter().all(|m| {
                        matches!(
                            m,
                            Mitigation::Absent {
                                reason: AbsenceReason::NotAnalysed,
                                ..
                            }
                        )
                    })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn threat_with(mitigations: Vec<Mitigation>) -> Threat {
        let mut threat = Threat::new(
            "T-1",
            "13.03",
            Asset::WorkerFleet,
            AttackClass::SandboxEscape,
            "agent_sandbox",
            "benchmark code breaks out of the container",
        );
        threat.mitigations = mitigations;
        threat
    }

    #[test]
    fn a_threat_whose_only_mitigation_is_declared_is_not_reported_as_mitigated() {
        let threat = threat_with(vec![Mitigation::declared(
            "hardened container or microVM",
            ControlRole::Preventative,
            "13.04",
        )]);
        assert_eq!(threat.status(), ThreatStatus::DeclaredOnly);
        assert!(!threat.prevented());
    }

    #[test]
    fn relying_on_a_declared_only_threat_is_a_typed_error_naming_the_declaration() {
        let threat = threat_with(vec![Mitigation::declared(
            "seccomp profile",
            ControlRole::Preventative,
            "13.04",
        )]);
        let error = threat.rely().expect_err("a declaration is not a control");
        assert_eq!(
            error,
            SafetyError::UnenforcedReliance {
                threat: "T-1".into(),
                mitigation: "seccomp profile".into(),
            }
        );
    }

    #[test]
    fn a_threat_with_no_mitigations_at_all_is_unmitigated_not_declared_only() {
        let threat = threat_with(Vec::new());
        assert_eq!(threat.status(), ThreatStatus::Unmitigated);
        assert!(matches!(
            threat.rely().expect_err("nothing to rely on"),
            SafetyError::UnmitigatedThreat { .. }
        ));
    }

    #[test]
    fn an_absent_mitigation_does_not_upgrade_a_threat_out_of_unmitigated() {
        let threat = threat_with(vec![Mitigation::absent(
            "microVM boundary",
            ControlRole::Preventative,
            AbsenceReason::RequiresAbsentInfrastructure {
                component: "hypervisor".into(),
            },
        )]);
        assert_eq!(threat.status(), ThreatStatus::Unmitigated);
    }

    #[test]
    fn one_enforced_mitigation_is_enough_to_report_a_threat_as_mitigated() {
        let threat = threat_with(vec![
            Mitigation::declared("audit log", ControlRole::Detective, "13.20"),
            Mitigation::enforced(
                "no value claims isolation was applied",
                ControlRole::Preventative,
                Unrepresentable::NoValueClaimsIsolationWasApplied,
            ),
        ]);
        assert_eq!(threat.status(), ThreatStatus::Mitigated);
        assert_eq!(threat.rely().expect("enforced").len(), 1);
    }

    #[test]
    fn an_enforced_detective_control_does_not_make_a_threat_prevented() {
        let threat = threat_with(vec![Mitigation::enforced(
            "assertions cannot be filed as observations",
            ControlRole::Detective,
            Unrepresentable::NoAssertionIsFiledAsAnObservation,
        )]);
        assert_eq!(threat.status(), ThreatStatus::Mitigated);
        assert!(!threat.prevented(), "detection is not prevention");
    }

    #[test]
    fn an_unanalysed_absence_is_never_an_accepted_risk() {
        assert!(!AbsenceReason::NotAnalysed.is_accepted());
        let threat = threat_with(vec![Mitigation::absent(
            "escape detection",
            ControlRole::Detective,
            AbsenceReason::NotAnalysed,
        )]);
        assert_eq!(threat.unaccepted_gaps().len(), 1);
        assert_eq!(ThreatModel::new().with_threat(threat).unanalysed().len(), 1);
    }

    #[test]
    fn an_acceptance_with_no_named_party_is_refused() {
        let threat = threat_with(vec![Mitigation::absent(
            "dedicated worker pool",
            ControlRole::Preventative,
            AbsenceReason::Accepted {
                by: "   ".into(),
                epoch: 4,
                note: "single-tenant deployment".into(),
            },
        )]);
        assert!(matches!(
            threat.audit_acceptances().expect_err("nobody signed"),
            SafetyError::AnonymousAcceptance { .. }
        ));
    }

    #[test]
    fn coverage_reports_three_numbers_and_refuses_to_collapse_them() {
        let model = ThreatModel::new()
            .with_threat(threat_with(vec![Mitigation::enforced(
                "typed",
                ControlRole::Preventative,
                Unrepresentable::NoValueNamesARuntimeEnforcer,
            )]))
            .with_threat(threat_with(vec![Mitigation::declared(
                "runbook",
                ControlRole::Recovery,
                "13.22",
            )]))
            .with_threat(threat_with(Vec::new()));
        let coverage = model.coverage();
        assert_eq!(coverage.mitigated, 1);
        assert_eq!(coverage.declared_only, 1);
        assert_eq!(coverage.unmitigated, 1);
        assert_eq!(coverage.total(), 3);
        assert_eq!(model.residual().len(), 2);
    }

    #[test]
    fn an_adversary_missing_one_required_capability_cannot_mount_the_threat() {
        let threat = threat_with(Vec::new())
            .requiring(Capability::ExecutesInAgentSandbox)
            .requiring(Capability::AuthorsContent);
        let partial = Adversary::new("reader", "leaderboard reader")
            .with(Capability::ObservesPublicSurface)
            .with(Capability::AuthorsContent);
        let full = Adversary::new("agent", "the benchmarked agent")
            .with(Capability::ExecutesInAgentSandbox)
            .with(Capability::AuthorsContent);
        assert!(!partial.can_mount(&threat));
        assert!(full.can_mount(&threat));
    }

    #[test]
    fn a_threat_no_modelled_adversary_can_mount_is_reported_rather_than_dropped() {
        let model = ThreatModel::new()
            .with_adversary(
                Adversary::new("reader", "public reader").with(Capability::ObservesPublicSurface),
            )
            .with_threat(threat_with(Vec::new()).requiring(Capability::HoldsCredential));
        assert_eq!(model.unreachable_threats().len(), 1);
        assert!(model.reachable_by("reader").is_empty());
    }

    #[test]
    fn the_enforcement_state_survives_a_json_round_trip_with_its_enforcer_named() {
        let mitigation = Mitigation::enforced(
            "signature status has one variant",
            ControlRole::Preventative,
            Unrepresentable::NoValueClaimsASignatureVerified,
        );
        let json = serde_json::to_string(&mitigation).expect("serialises");
        assert!(json.contains("\"state\":\"enforced\""), "{json}");
        assert!(
            json.contains("no_value_claims_a_signature_verified"),
            "{json}"
        );
        let back: Mitigation = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(back, mitigation);
    }

    #[test]
    fn an_honest_label_for_a_declaration_says_nothing_applies_it() {
        let label =
            Mitigation::declared("no ambient credentials", ControlRole::Preventative, "13.03")
                .honest_label();
        assert!(
            label.contains("nothing in this process applies it"),
            "{label}"
        );
    }
}
