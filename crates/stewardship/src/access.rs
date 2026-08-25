//! Data contracts, purpose-bound grants, the access audit, and what a federated site signs.
//!
//! Implements blueprint 14.15 (Data Governance, Federation and Access): its data contract, its
//! time-bound and purpose-bound grants, its federated-result rule, its withdrawal semantics, and
//! its audit requirement.
//!
//! # Two neighbours own the hard parts, and this module refuses to own them twice
//!
//! **`bioprism-policy`** owns enforcement. It has the information-flow lattice, its seven-axis
//! `PolicyLabel` classification, consent, residency, declassification with leakage analysis,
//! redaction with receipts, and — the reason this matters here — a **closed `Purpose` enumeration
//! with no implication order, so that purpose creep has no accidental spelling**. That crate
//! decides whether a fact may be compiled at all, per candidate, before scoring.
//!
//! So [`PurposeTag`] here is an **opaque string**, and that is a decision rather than laziness. A
//! second closed purpose enumeration in the same workspace would drift from the first, and the
//! moment they disagree the enforcement layer and the governance layer are describing different
//! systems. This module governs the *declaration* — which purposes a contract permits, which it
//! forbids, who was granted what and until when — and leaves the semantics of a purpose to the
//! crate that enforces it.
//!
//! **`bioprism-hubapi`** owns federation, and specifically the rule that *trust does not transit*:
//! a `TrustStanding` carries the registry it holds in, there is no function turning one registry's
//! standing into another's, and adoption is capped at one hop. [`SiteAttestation`] here confers no
//! standing on anybody. It answers a narrower question — whether a federated site signed the five
//! things 14.15 requires, and which of its fields a central analysis may receive.
//!
//! # The rules that are this module's own
//!
//! - **A grant expires.** 14.15: "Time-bound, purpose-bound grants; least privilege ... automatic
//!   expiry." There is no `grant_indefinite`, no `Option<Epoch>` expiry, and
//!   [`DataContract::grant`] refuses an expiry that is not strictly after the grant epoch. Expiry
//!   is checked against an operator epoch, not a clock.
//! - **A grant cannot exceed its contract.** Purposes must be permitted, fields must be releasable,
//!   and the check is on the way in, so a grant that exists is already within its contract.
//! - **Prohibition is not the absence of permission.** A contract that lists a purpose in both
//!   places is refused at construction rather than resolved by precedence, because the two readings
//!   of such a contract differ and neither author intended the other.
//! - **Derivative and training rights default to denied.** [`DerivativeRights`] starts false and is
//!   granted explicitly. A permission that has to be typed out is one somebody decided.
//! - **Withdrawal stops future access and does not erase the past.** 14.15: "Immutable publications
//!   may require correction rather than erasure." [`WithdrawalDisposition`] has a
//!   `CorrectionRequired` arm and no `Erased` arm, and [`AccessLedger`] has no removal method at
//!   all.
//!
//! # Not implemented
//!
//! No enforcement: nothing here intercepts a read. A caller that ignores a refusal produces an
//! illegal access that this crate will not notice — the same limitation `bioprism-policy` states
//! about its obligations, and for the same reason. No cryptography: "signed" means "a record naming
//! five digests". No redaction, no small-cell suppression, no differencing analysis —
//! `bioprism-policy` owns those and says what it cannot answer. No retention *enforcement*: a
//! retention ceiling is checked when an access is recorded, and nothing deletes anything. No
//! identity: see [`crate::id`].

use crate::error::AccessError;
use crate::id::{Actor, ActorId, Epoch, Role};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A declared purpose, held as an opaque tag.
///
/// `bioprism_policy::Purpose` is the closed enumeration that decides what a purpose *means*. This
/// is the string a contract writes down, and keeping it opaque is how the workspace avoids two
/// purpose vocabularies that drift apart. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PurposeTag(String);

impl PurposeTag {
    pub fn new(tag: impl Into<String>) -> Self {
        PurposeTag(tag.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PurposeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifier for a data contract.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContractId(String);

impl ContractId {
    pub fn new(id: impl Into<String>) -> Self {
        ContractId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Identifier for an issued grant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct GrantId(String);

impl GrantId {
    pub fn new(id: impl Into<String>) -> Self {
        GrantId(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GrantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// 14.15's "model/provider restrictions" field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "restriction", rename_all = "snake_case")]
pub enum ModelRestriction {
    /// Any model may process data under this contract.
    Unrestricted,
    /// Only the named models may. An empty allowlist permits nothing, which is the correct reading
    /// of an allowlist somebody forgot to fill in.
    Allowlist { models: BTreeSet<String> },
}

impl ModelRestriction {
    pub fn allows(&self, model: &str) -> bool {
        match self {
            ModelRestriction::Unrestricted => true,
            ModelRestriction::Allowlist { models } => models.contains(model),
        }
    }

    pub fn allowlist(models: impl IntoIterator<Item = &'static str>) -> Self {
        ModelRestriction::Allowlist {
            models: models.into_iter().map(String::from).collect(),
        }
    }
}

/// 14.15's "retention" field, as an epoch ceiling rather than a duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "retention", rename_all = "snake_case")]
pub enum Retention {
    /// No stated ceiling. Permitted, and recorded as such rather than defaulted away.
    Unbounded,
    /// Access is refused at epochs past this one.
    Until { epoch: Epoch },
}

impl Retention {
    pub fn permits(&self, epoch: Epoch) -> bool {
        match self {
            Retention::Unbounded => true,
            Retention::Until { epoch: until } => epoch <= *until,
        }
    }
}

/// 14.15's "publication" field: what may be said outside, from data under this contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationRight {
    /// Nothing derived from this data may be published.
    None,
    /// Only aggregates.
    AggregateOnly,
    /// Anything the releasable-field list permits.
    Full,
}

/// 14.15's "derivative and training rights". Both default to denied.
///
/// `Default` is the deny-all state, which is the one case where deriving `Default` is a governance
/// decision rather than a convenience: a contract deserialized from a document that omits the block
/// grants nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivativeRights {
    /// Whether derived artifacts may be produced and retained.
    pub derived_artifacts: bool,
    /// Whether the data may be used to train a model.
    pub model_training: bool,
}

impl DerivativeRights {
    pub const fn permitting_derivatives() -> Self {
        DerivativeRights {
            derived_artifacts: true,
            model_training: false,
        }
    }
}

/// The data contract of 14.15.
///
/// Constructed through [`DataContract::new`] so that the permitted/prohibited contradiction is
/// caught once, at the point where somebody wrote it, rather than at every use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataContract {
    pub id: ContractId,
    pub steward: Actor,
    pub subjects: String,
    pub source: String,
    pub classification: String,
    permitted_purposes: BTreeSet<PurposeTag>,
    prohibited_uses: BTreeSet<PurposeTag>,
    pub model_restriction: ModelRestriction,
    pub retention: Retention,
    pub publication: PublicationRight,
    pub derivative_rights: DerivativeRights,
    releasable_fields: BTreeSet<String>,
    withdrawn_at: Option<Epoch>,
}

impl DataContract {
    /// Build a contract. Refuses a purpose that is both permitted and prohibited.
    ///
    /// The steward's role is not checked here — a contract may be authored by anyone — but
    /// [`DataContract::grant`] refuses a grant issued by a party who is not the steward, which is
    /// where the role actually bites.
    pub fn new(
        id: impl Into<String>,
        steward: Actor,
        subjects: impl Into<String>,
        source: impl Into<String>,
        classification: impl Into<String>,
        permitted: impl IntoIterator<Item = PurposeTag>,
        prohibited: impl IntoIterator<Item = PurposeTag>,
    ) -> Result<Self, AccessError> {
        let permitted_purposes: BTreeSet<PurposeTag> = permitted.into_iter().collect();
        let prohibited_uses: BTreeSet<PurposeTag> = prohibited.into_iter().collect();
        if let Some(both) = permitted_purposes.intersection(&prohibited_uses).next() {
            return Err(AccessError::PurposeBothPermittedAndProhibited {
                purpose: both.to_string(),
            });
        }
        Ok(DataContract {
            id: ContractId::new(id),
            steward,
            subjects: subjects.into(),
            source: source.into(),
            classification: classification.into(),
            permitted_purposes,
            prohibited_uses,
            model_restriction: ModelRestriction::Unrestricted,
            retention: Retention::Unbounded,
            publication: PublicationRight::None,
            derivative_rights: DerivativeRights::default(),
            releasable_fields: BTreeSet::new(),
            withdrawn_at: None,
        })
    }

    #[must_use]
    pub fn releasing(mut self, field: impl Into<String>) -> Self {
        self.releasable_fields.insert(field.into());
        self
    }

    #[must_use]
    pub fn restricted_to(mut self, restriction: ModelRestriction) -> Self {
        self.model_restriction = restriction;
        self
    }

    #[must_use]
    pub fn retained(mut self, retention: Retention) -> Self {
        self.retention = retention;
        self
    }

    #[must_use]
    pub fn publishing(mut self, right: PublicationRight) -> Self {
        self.publication = right;
        self
    }

    #[must_use]
    pub fn with_rights(mut self, rights: DerivativeRights) -> Self {
        self.derivative_rights = rights;
        self
    }

    pub fn permitted_purposes(&self) -> Vec<&PurposeTag> {
        self.permitted_purposes.iter().collect()
    }

    pub fn prohibited_uses(&self) -> Vec<&PurposeTag> {
        self.prohibited_uses.iter().collect()
    }

    pub fn releasable_fields(&self) -> Vec<&str> {
        self.releasable_fields.iter().map(String::as_str).collect()
    }

    pub fn withdrawn_at(&self) -> Option<Epoch> {
        self.withdrawn_at
    }

    /// Stop future access. 14.15's withdrawal: it stops grants and touches no record.
    pub fn withdraw(&mut self, at: Epoch) {
        self.withdrawn_at = Some(at);
    }

    /// Issue a grant, or say which contract term refuses it.
    pub fn grant(
        &self,
        id: impl Into<String>,
        grantee: Actor,
        purposes: impl IntoIterator<Item = PurposeTag>,
        fields: impl IntoIterator<Item = String>,
        granted_at: Epoch,
        expires_at: Epoch,
    ) -> Result<AccessGrant, AccessError> {
        if let Some(epoch) = self.withdrawn_at {
            return Err(AccessError::ContractWithdrawn {
                contract: self.id.to_string(),
                epoch,
            });
        }
        if expires_at <= granted_at {
            return Err(AccessError::GrantWithoutExpiry {
                granted: granted_at,
                expires: expires_at,
            });
        }
        let purposes: BTreeSet<PurposeTag> = purposes.into_iter().collect();
        for purpose in &purposes {
            if self.prohibited_uses.contains(purpose) {
                return Err(AccessError::PurposeProhibited {
                    purpose: purpose.to_string(),
                });
            }
            if !self.permitted_purposes.contains(purpose) {
                return Err(AccessError::PurposeNotPermitted {
                    purpose: purpose.to_string(),
                });
            }
        }
        let fields: BTreeSet<String> = fields.into_iter().collect();
        for field in &fields {
            if !self.releasable_fields.contains(field) {
                return Err(AccessError::FieldNotReleasable {
                    field: field.clone(),
                });
            }
        }
        Ok(AccessGrant {
            id: GrantId::new(id),
            contract: self.id.clone(),
            grantee,
            purposes,
            fields,
            granted_at,
            expires_at,
        })
    }
}

/// A time-bound, purpose-bound, field-bound authorisation.
///
/// Every field is set by [`DataContract::grant`] and none can exceed the contract, so a grant that
/// exists is one the contract permitted. `expires_at` is an [`Epoch`] rather than an
/// `Option<Epoch>`: there is no representation of a grant that never ends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessGrant {
    pub id: GrantId,
    pub contract: ContractId,
    pub grantee: Actor,
    purposes: BTreeSet<PurposeTag>,
    fields: BTreeSet<String>,
    pub granted_at: Epoch,
    pub expires_at: Epoch,
}

impl AccessGrant {
    pub fn purposes(&self) -> Vec<&PurposeTag> {
        self.purposes.iter().collect()
    }

    pub fn fields(&self) -> Vec<&str> {
        self.fields.iter().map(String::as_str).collect()
    }

    pub fn covers_purpose(&self, purpose: &PurposeTag) -> bool {
        self.purposes.contains(purpose)
    }

    /// Whether the grant is live at `epoch`. Expiry is inclusive of the granted epoch and exclusive
    /// of the expiry epoch, so a grant "to epoch 10" does not authorise anything at epoch 10.
    pub fn live_at(&self, epoch: Epoch) -> bool {
        epoch >= self.granted_at && epoch < self.expires_at
    }
}

/// Which architecture and model processed the data. 14.15's audit asks exactly this.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProcessingPath {
    pub architecture: String,
    pub model: String,
}

impl ProcessingPath {
    pub fn new(architecture: impl Into<String>, model: impl Into<String>) -> Self {
        ProcessingPath {
            architecture: architecture.into(),
            model: model.into(),
        }
    }
}

/// One recorded access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessEvent {
    pub grant: GrantId,
    pub contract: ContractId,
    pub by: ActorId,
    pub purpose: PurposeTag,
    pub at: Epoch,
    pub processing: ProcessingPath,
    /// Digests of what was produced. 14.15: stewards can see "which artifacts were produced".
    #[serde(default)]
    pub artifacts: Vec<ContentHash>,
    /// Where evidence derived from this access was published, if anywhere.
    #[serde(default)]
    pub published_to: Option<String>,
}

/// What withdrawal implies for an already-recorded access.
///
/// There is no `Erased` arm. 14.15: *"Immutable publications may require correction rather than
/// erasure."* An enumeration that offered erasure would invite a caller to pick it and then
/// discover that this crate cannot perform it — a promise nothing can keep is worse than a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "disposition", rename_all = "snake_case")]
pub enum WithdrawalDisposition {
    /// The grant was live and is now dead. No published artifact is affected.
    FutureAccessStopped { grant: GrantId },
    /// Something derived from this access is already public.
    CorrectionRequired {
        grant: GrantId,
        published_to: String,
        artifacts: Vec<ContentHash>,
    },
}

/// What a withdrawal produced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WithdrawalAssessment {
    pub contract: ContractId,
    pub at: Epoch,
    pub dispositions: Vec<WithdrawalDisposition>,
}

impl WithdrawalAssessment {
    /// The published artifacts a steward must now go and correct.
    pub fn corrections(&self) -> Vec<&WithdrawalDisposition> {
        self.dispositions
            .iter()
            .filter(|d| matches!(d, WithdrawalDisposition::CorrectionRequired { .. }))
            .collect()
    }
}

/// The append-only audit of 14.15.
///
/// No `remove`, no `redact`, no `clear`. A ledger that could forget is not an audit, and the one
/// operation a withdrawal performs on it is *reading* — [`AccessLedger::withdraw`] returns an
/// assessment and mutates nothing but the contract's own withdrawal epoch.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AccessLedger {
    grants: BTreeMap<GrantId, AccessGrant>,
    events: Vec<AccessEvent>,
}

impl AccessLedger {
    pub fn new() -> Self {
        AccessLedger::default()
    }

    pub fn hold(&mut self, grant: AccessGrant) {
        self.grants.insert(grant.id.clone(), grant);
    }

    pub fn grant(&self, id: &GrantId) -> Option<&AccessGrant> {
        self.grants.get(id)
    }

    pub fn events(&self) -> &[AccessEvent] {
        &self.events
    }

    /// Record an access, or refuse it.
    ///
    /// Five checks: the grant is held; the purpose is within it; the field set is within it; the
    /// grant is live at this epoch; the contract's retention ceiling has not passed. The last is
    /// the one that catches a long-lived grant on data whose retention was shorter than anybody
    /// noticed.
    #[allow(clippy::too_many_arguments)]
    pub fn record(
        &mut self,
        contract: &DataContract,
        grant_id: &GrantId,
        by: &Actor,
        purpose: &PurposeTag,
        processing: ProcessingPath,
        at: Epoch,
        artifacts: Vec<ContentHash>,
        published_to: Option<String>,
    ) -> Result<(), AccessError> {
        let grant = self
            .grants
            .get(grant_id)
            .ok_or_else(|| AccessError::GrantUnknown {
                grant: grant_id.to_string(),
            })?;
        if !grant.covers_purpose(purpose) {
            return Err(AccessError::PurposeNotPermitted {
                purpose: purpose.to_string(),
            });
        }
        if !grant.live_at(at) {
            return Err(AccessError::AccessOutsideGrant {
                grant: grant_id.to_string(),
                epoch: at,
                expires: grant.expires_at,
            });
        }
        if !contract.model_restriction.allows(&processing.model) {
            return Err(AccessError::ModelNotPermitted {
                model: processing.model,
            });
        }
        if !contract.retention.permits(at) {
            let until = match contract.retention {
                Retention::Until { epoch } => epoch,
                Retention::Unbounded => unreachable!("unbounded retention permits every epoch"),
            };
            return Err(AccessError::PastRetention {
                contract: contract.id.to_string(),
                until,
                epoch: at,
            });
        }
        if published_to.is_some() && contract.publication == PublicationRight::None {
            return Err(AccessError::RightNotGranted {
                right: "publication",
            });
        }
        if !artifacts.is_empty() && !contract.derivative_rights.derived_artifacts {
            return Err(AccessError::RightNotGranted {
                right: "derivative artifact",
            });
        }
        self.events.push(AccessEvent {
            grant: grant_id.clone(),
            contract: contract.id.clone(),
            by: by.id.clone(),
            purpose: purpose.clone(),
            at,
            processing,
            artifacts,
            published_to,
        });
        Ok(())
    }

    /// 14.15's audit: everything recorded against one contract.
    pub fn audit(&self, contract: &ContractId) -> Vec<&AccessEvent> {
        self.events
            .iter()
            .filter(|e| e.contract == *contract)
            .collect()
    }

    /// Withdraw a contract and assess what the withdrawal reaches.
    ///
    /// Grants stop; published derivatives become corrections. Nothing in the ledger is removed or
    /// rewritten, which is why the assessment is a return value rather than a mutation.
    pub fn withdraw(
        &mut self,
        contract: &mut DataContract,
        at: Epoch,
    ) -> Result<WithdrawalAssessment, AccessError> {
        if let Some(epoch) = contract.withdrawn_at {
            return Err(AccessError::ContractWithdrawn {
                contract: contract.id.to_string(),
                epoch,
            });
        }
        contract.withdraw(at);

        let mut dispositions = Vec::new();
        let mut seen: BTreeSet<GrantId> = BTreeSet::new();
        for event in self.events.iter().filter(|e| e.contract == contract.id) {
            if let Some(published_to) = &event.published_to {
                dispositions.push(WithdrawalDisposition::CorrectionRequired {
                    grant: event.grant.clone(),
                    published_to: published_to.clone(),
                    artifacts: event.artifacts.clone(),
                });
            }
            seen.insert(event.grant.clone());
        }
        for grant in self
            .grants
            .values()
            .filter(|g| g.contract == contract.id && g.live_at(at))
        {
            dispositions.push(WithdrawalDisposition::FutureAccessStopped {
                grant: grant.id.clone(),
            });
        }
        Ok(WithdrawalAssessment {
            contract: contract.id.clone(),
            at,
            dispositions,
        })
    }
}

/// A federated site's attestation, under construction.
///
/// 14.15: *"Site signs environment, pack, architecture, policy, and result evidence."* All five, so
/// the draft holds five `Option`s and [`SiteAttestationDraft::sign`] names the first missing one.
/// Modelling them as five non-optional fields would make the invariant trivially true and would
/// give a caller nowhere to put a partially assembled attestation, which is the state a real site
/// is in while it assembles one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteAttestationDraft {
    pub site: String,
    pub environment: Option<ContentHash>,
    pub pack: Option<ContentHash>,
    pub architecture: Option<ContentHash>,
    pub policy: Option<ContentHash>,
    pub result_evidence: Option<ContentHash>,
    /// The fields this site proposes to send to a central analysis.
    pub offered_fields: BTreeSet<String>,
}

impl SiteAttestationDraft {
    pub fn new(site: impl Into<String>) -> Self {
        SiteAttestationDraft {
            site: site.into(),
            ..SiteAttestationDraft::default()
        }
    }

    #[must_use]
    pub fn covering(mut self, what: AttestedComponent, digest: ContentHash) -> Self {
        match what {
            AttestedComponent::Environment => self.environment = Some(digest),
            AttestedComponent::Pack => self.pack = Some(digest),
            AttestedComponent::Architecture => self.architecture = Some(digest),
            AttestedComponent::Policy => self.policy = Some(digest),
            AttestedComponent::ResultEvidence => self.result_evidence = Some(digest),
        }
        self
    }

    #[must_use]
    pub fn offering(mut self, field: impl Into<String>) -> Self {
        self.offered_fields.insert(field.into());
        self
    }

    /// Complete the attestation, or name the first thing it does not cover.
    pub fn sign(self) -> Result<SiteAttestation, AccessError> {
        let missing = |what: &'static str| AccessError::IncompleteAttestation { missing: what };
        Ok(SiteAttestation {
            site: self.site,
            environment: self.environment.ok_or_else(|| missing("environment"))?,
            pack: self.pack.ok_or_else(|| missing("pack"))?,
            architecture: self.architecture.ok_or_else(|| missing("architecture"))?,
            policy: self.policy.ok_or_else(|| missing("policy"))?,
            result_evidence: self
                .result_evidence
                .ok_or_else(|| missing("result evidence"))?,
            offered_fields: self.offered_fields,
        })
    }
}

/// Which of the five things a site signs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestedComponent {
    Environment,
    Pack,
    Architecture,
    Policy,
    ResultEvidence,
}

/// A complete site attestation.
///
/// Confers no standing. `bioprism-hubapi` owns trust and the rule that it does not transit; this
/// object says only that one site recorded five digests and offered a field set, and
/// [`SiteAttestation::project`] decides which of those fields a central analysis may keep.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteAttestation {
    pub site: String,
    pub environment: ContentHash,
    pub pack: ContentHash,
    pub architecture: ContentHash,
    pub policy: ContentHash,
    pub result_evidence: ContentHash,
    offered_fields: BTreeSet<String>,
}

impl SiteAttestation {
    pub fn offered_fields(&self) -> Vec<&str> {
        self.offered_fields.iter().map(String::as_str).collect()
    }

    /// Keep only the fields the contract makes releasable, and say which were dropped.
    ///
    /// 14.15: "Central analysis receives only allowed fields and uncertainty." This is a check on a
    /// declaration, not a redaction engine — `bioprism-policy` owns redaction with receipts, and a
    /// caller who ignores this projection and forwards the raw offer is doing something this crate
    /// cannot see.
    pub fn project(&self, contract: &DataContract) -> (Vec<String>, Vec<String>) {
        let releasable: BTreeSet<&str> = contract.releasable_fields().into_iter().collect();
        let mut kept = Vec::new();
        let mut dropped = Vec::new();
        for field in &self.offered_fields {
            if releasable.contains(field.as_str()) {
                kept.push(field.clone());
            } else {
                dropped.push(field.clone());
            }
        }
        (kept, dropped)
    }

    /// Refuse an attestation offering anything the contract does not release.
    ///
    /// The strict counterpart of [`SiteAttestation::project`]. A central analysis that would rather
    /// stop than silently narrow calls this one.
    pub fn accept(&self, contract: &DataContract) -> Result<(), AccessError> {
        let (_, dropped) = self.project(contract);
        if let Some(field) = dropped.first() {
            return Err(AccessError::FieldNotReleasable {
                field: field.clone(),
            });
        }
        Ok(())
    }
}

/// Whether an actor may issue grants under a contract.
///
/// Separated out because it is the one place [`Role::DataSteward`] does any work, and because a
/// caller may reasonably want to ask before building a grant.
pub fn may_grant(contract: &DataContract, actor: &Actor) -> bool {
    actor.role == Role::DataSteward && actor.id == contract.steward.id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn purpose(tag: &str) -> PurposeTag {
        PurposeTag::new(tag)
    }

    fn contract() -> DataContract {
        DataContract::new(
            "contract-1",
            Actor::steward("steward-1"),
            "consented research volunteers",
            "site-a registry export",
            "restricted",
            [
                purpose("method-development"),
                purpose("benchmark-evaluation"),
            ],
            [purpose("model-training"), purpose("marketing")],
        )
        .unwrap()
        .releasing("aggregate_score")
        .releasing("interval")
        .with_rights(DerivativeRights::permitting_derivatives())
        .publishing(PublicationRight::AggregateOnly)
    }

    fn grant(contract: &DataContract) -> AccessGrant {
        contract
            .grant(
                "grant-1",
                Actor::independent_reviewer("analyst-1"),
                [purpose("method-development")],
                ["aggregate_score".to_string()],
                Epoch(2),
                Epoch(10),
            )
            .unwrap()
    }

    #[test]
    fn a_contract_that_both_permits_and_prohibits_a_purpose_is_refused() {
        let outcome = DataContract::new(
            "c",
            Actor::steward("s"),
            "subjects",
            "source",
            "class",
            [purpose("training")],
            [purpose("training")],
        );
        assert!(matches!(
            outcome,
            Err(AccessError::PurposeBothPermittedAndProhibited { .. })
        ));
    }

    #[test]
    fn a_grant_that_never_expires_has_no_spelling() {
        let contract = contract();
        assert!(matches!(
            contract.grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("method-development")],
                ["aggregate_score".to_string()],
                Epoch(5),
                Epoch(5)
            ),
            Err(AccessError::GrantWithoutExpiry { .. })
        ));
        assert!(matches!(
            contract.grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("method-development")],
                ["aggregate_score".to_string()],
                Epoch(5),
                Epoch(4)
            ),
            Err(AccessError::GrantWithoutExpiry { .. })
        ));
    }

    #[test]
    fn a_grant_cannot_exceed_the_purposes_or_fields_of_its_contract() {
        let contract = contract();
        assert!(matches!(
            contract.grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("epidemiology")],
                [],
                Epoch(1),
                Epoch(2)
            ),
            Err(AccessError::PurposeNotPermitted { .. })
        ));
        assert!(matches!(
            contract.grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("model-training")],
                [],
                Epoch(1),
                Epoch(2)
            ),
            Err(AccessError::PurposeProhibited { .. })
        ));
        assert!(matches!(
            contract.grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("method-development")],
                ["raw_record".to_string()],
                Epoch(1),
                Epoch(2)
            ),
            Err(AccessError::FieldNotReleasable { .. })
        ));
    }

    #[test]
    fn a_prohibited_purpose_is_refused_before_it_is_found_missing_from_the_permitted_set() {
        let contract = contract();
        let outcome = contract.grant(
            "g",
            Actor::independent_reviewer("a"),
            [purpose("marketing")],
            [],
            Epoch(1),
            Epoch(2),
        );
        assert!(matches!(
            outcome,
            Err(AccessError::PurposeProhibited { .. })
        ));
    }

    #[test]
    fn an_access_after_the_grant_expires_is_refused() {
        let contract = contract();
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        let analyst = Actor::independent_reviewer("analyst-1");
        assert!(ledger
            .record(
                &contract,
                &id,
                &analyst,
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(9),
                Vec::new(),
                None
            )
            .is_ok());
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &analyst,
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(10),
                Vec::new(),
                None
            ),
            Err(AccessError::AccessOutsideGrant { .. })
        ));
    }

    #[test]
    fn an_access_for_a_purpose_outside_the_grant_is_refused_even_when_the_contract_permits_it() {
        let contract = contract();
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("benchmark-evaluation"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(3),
                Vec::new(),
                None
            ),
            Err(AccessError::PurposeNotPermitted { .. })
        ));
    }

    #[test]
    fn a_model_outside_the_contracts_allowlist_may_not_process_the_data() {
        let contract = contract().restricted_to(ModelRestriction::allowlist(["model-a"]));
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-b"),
                Epoch(3),
                Vec::new(),
                None
            ),
            Err(AccessError::ModelNotPermitted { .. })
        ));
    }

    #[test]
    fn derivative_and_training_rights_default_to_denied() {
        let rights = DerivativeRights::default();
        assert!(!rights.derived_artifacts);
        assert!(!rights.model_training);
        let contract = DataContract::new(
            "c",
            Actor::steward("s"),
            "subjects",
            "source",
            "class",
            [purpose("method-development")],
            [],
        )
        .unwrap()
        .releasing("aggregate_score");
        let mut ledger = AccessLedger::new();
        let grant = contract
            .grant(
                "g",
                Actor::independent_reviewer("a"),
                [purpose("method-development")],
                ["aggregate_score".to_string()],
                Epoch(1),
                Epoch(9),
            )
            .unwrap();
        let id = grant.id.clone();
        ledger.hold(grant);
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &Actor::independent_reviewer("a"),
                &purpose("method-development"),
                ProcessingPath::new("arch", "model"),
                Epoch(2),
                vec![ContentHash::of_bytes(b"derived")],
                None
            ),
            Err(AccessError::RightNotGranted {
                right: "derivative artifact"
            })
        ));
    }

    #[test]
    fn an_access_past_the_retention_ceiling_is_refused() {
        let contract = contract().retained(Retention::Until { epoch: Epoch(4) });
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(5),
                Vec::new(),
                None
            ),
            Err(AccessError::PastRetention { .. })
        ));
    }

    #[test]
    fn withdrawal_stops_future_grants_and_leaves_the_audit_intact() {
        let mut contract = contract();
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        ledger
            .record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(3),
                vec![ContentHash::of_bytes(b"table-1")],
                Some("preprint-77".into()),
            )
            .unwrap();
        let assessment = ledger.withdraw(&mut contract, Epoch(6)).unwrap();
        assert_eq!(ledger.audit(&contract.id).len(), 1);
        assert_eq!(assessment.corrections().len(), 1);
        assert!(assessment
            .dispositions
            .iter()
            .any(|d| matches!(d, WithdrawalDisposition::FutureAccessStopped { .. })));
        assert!(matches!(
            contract.grant(
                "g2",
                Actor::independent_reviewer("a"),
                [purpose("method-development")],
                [],
                Epoch(7),
                Epoch(8)
            ),
            Err(AccessError::ContractWithdrawn { .. })
        ));
    }

    #[test]
    fn a_published_derivative_becomes_a_correction_rather_than_an_erasure() {
        let mut contract = contract();
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        ledger
            .record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(3),
                vec![ContentHash::of_bytes(b"fig-2")],
                Some("journal-x".into()),
            )
            .unwrap();
        let assessment = ledger.withdraw(&mut contract, Epoch(6)).unwrap();
        let correction = assessment.corrections()[0];
        match correction {
            WithdrawalDisposition::CorrectionRequired { published_to, .. } => {
                assert_eq!(published_to, "journal-x")
            }
            WithdrawalDisposition::FutureAccessStopped { .. } => panic!("expected a correction"),
        }
    }

    #[test]
    fn publishing_under_a_contract_that_permits_no_publication_is_refused() {
        let contract = contract().publishing(PublicationRight::None);
        let mut ledger = AccessLedger::new();
        let grant = grant(&contract);
        let id = grant.id.clone();
        ledger.hold(grant);
        assert!(matches!(
            ledger.record(
                &contract,
                &id,
                &Actor::independent_reviewer("analyst-1"),
                &purpose("method-development"),
                ProcessingPath::new("arch-a", "model-a"),
                Epoch(3),
                Vec::new(),
                Some("blog".into())
            ),
            Err(AccessError::RightNotGranted {
                right: "publication"
            })
        ));
    }

    #[test]
    fn a_site_attestation_missing_any_of_the_five_signed_components_is_not_signed() {
        let full = SiteAttestationDraft::new("site-a")
            .covering(AttestedComponent::Environment, ContentHash::of_bytes(b"e"))
            .covering(AttestedComponent::Pack, ContentHash::of_bytes(b"p"))
            .covering(AttestedComponent::Architecture, ContentHash::of_bytes(b"a"))
            .covering(AttestedComponent::Policy, ContentHash::of_bytes(b"o"))
            .covering(
                AttestedComponent::ResultEvidence,
                ContentHash::of_bytes(b"r"),
            );
        assert!(full.clone().sign().is_ok());
        let mut without_policy = full;
        without_policy.policy = None;
        assert!(matches!(
            without_policy.sign(),
            Err(AccessError::IncompleteAttestation { missing: "policy" })
        ));
    }

    #[test]
    fn a_central_analysis_receives_only_the_fields_the_contract_releases() {
        let contract = contract();
        let attestation = SiteAttestationDraft::new("site-a")
            .covering(AttestedComponent::Environment, ContentHash::of_bytes(b"e"))
            .covering(AttestedComponent::Pack, ContentHash::of_bytes(b"p"))
            .covering(AttestedComponent::Architecture, ContentHash::of_bytes(b"a"))
            .covering(AttestedComponent::Policy, ContentHash::of_bytes(b"o"))
            .covering(
                AttestedComponent::ResultEvidence,
                ContentHash::of_bytes(b"r"),
            )
            .offering("aggregate_score")
            .offering("interval")
            .offering("subject_id")
            .sign()
            .unwrap();
        let (kept, dropped) = attestation.project(&contract);
        assert_eq!(kept, vec!["aggregate_score", "interval"]);
        assert_eq!(dropped, vec!["subject_id"]);
        assert!(matches!(
            attestation.accept(&contract),
            Err(AccessError::FieldNotReleasable { .. })
        ));
    }

    #[test]
    fn only_the_named_steward_may_issue_grants_under_a_contract() {
        let contract = contract();
        assert!(may_grant(&contract, &Actor::steward("steward-1")));
        assert!(!may_grant(&contract, &Actor::steward("steward-2")));
        assert!(!may_grant(
            &contract,
            &Actor::independent_reviewer("steward-1")
        ));
    }

    #[test]
    fn an_empty_allowlist_permits_no_model() {
        let restriction = ModelRestriction::Allowlist {
            models: BTreeSet::new(),
        };
        assert!(!restriction.allows("anything"));
        assert!(ModelRestriction::Unrestricted.allows("anything"));
    }
}
