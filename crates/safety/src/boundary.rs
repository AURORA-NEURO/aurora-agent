//! Trust zones, the directed edges between them, and the one boundary that is structurally hard.
//!
//! Implements blueprint 13.03 (sandbox and untrusted code), 13.04 (sandbox isolation and escape
//! resistance), 13.05 (evaluator and grader trust boundary) and 13.19 (tenant isolation and private
//! workers).
//!
//! # 13.05 is the interesting one, and here is why
//!
//! Sandbox escape has an external answer: a hypervisor, a seccomp profile, a separate node. Those
//! are real controls, none of them lives in this workspace, and modelling them honestly means
//! writing `DeclaredOnly` next to each one. The evaluator/grader boundary is different, because the
//! grader is *inside the system being graded*. The agent produces the state the grader reads. If
//! any path exists from the agent's zone to the grader's zone other than the sealed output bundle,
//! the grade is a function of something the graded party chose, and no amount of hypervisor fixes
//! that.
//!
//! So this module models influence as a directed graph and offers
//! [`BoundaryModel::influence_paths`], which returns *paths* — concrete node sequences a reviewer
//! can read — rather than a verdict. On [`BoundaryModel::evaluation_model`] there is exactly one
//! path from [`TrustZone::AgentSandbox`] to [`TrustZone::EvaluatorSandbox`] and it goes through
//! [`Channel::SealedOutputBundle`]; a test holds that.
//!
//! # The diode holds within a trial and leaks across trials
//!
//! Writing the model down produced a result the blueprint does not state. 13.05 describes the
//! diode as a property of the two sandboxes, and within one trial it is: nothing runs from
//! [`TrustZone::EvaluatorSandbox`] to [`TrustZone::AgentSandbox`]. But the evaluator's typed claim
//! goes to the control plane, and the control plane composes the inputs of the *next* trial — so
//! `evaluator → control plane → agent` is a path, and a grader that varies its claims can steer
//! what the agent is asked next. 13.14 calls the destination of that channel "reward hacking" and
//! 13.05 does not mention it.
//!
//! [`EdgeScope`] therefore labels every edge, [`BoundaryModel::influence_paths_within_trial`] is the
//! query the diode claim is actually about, and [`BoundaryModel::feedback_loops`] reports the
//! across-trial route rather than hiding it. Both are held by tests, including the one that says
//! the loop exists.
//!
//! # Deny by default
//!
//! An edge nobody modelled is closed: [`BoundaryModel::deliver`] returns
//! [`SafetyError::UnmodelledEdge`] rather than allowing the crossing. This is the one place where
//! forgetting to write something down fails safe, and it is deliberate — 13.01's control philosophy
//! opens with "deny by default", and a model where omission means permission would invert it.
//!
//! # What this module does not do
//!
//! It moves nothing and blocks nothing. [`BoundaryModel::deliver`] refuses a crossing *this crate
//! was asked to perform*, which in practice means a caller that routes its artifact handling
//! through this type gets a checked model of its own intent. An artifact that moves by any other
//! route — a shared filesystem, an environment variable, a global — is invisible here, and no test
//! in this crate can notice it. There is no filesystem, no process, no mount, no namespace, no
//! syscall filter, no cgroup, and no network. 13.04's entire "Kernel/runtime" and "Process
//! controls" sections are absent infrastructure, recorded as such in [`crate::model`].
//!
//! Tenant isolation (13.19) gets the same treatment as `bioprism_sdk`'s sandbox: [`TenantIsolation`]
//! has one variant, so no value in this workspace can record that a tenant was isolated by anything
//! other than someone saying so. [`TenantBoundary::admits`] is a real check, but on *declared
//! scopes*, not on storage.

use crate::error::SafetyError;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The trust zones 13.01 and 13.02 enumerate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustZone {
    /// A developer's browser or CLI.
    UserClient,
    /// The public HTTP surface.
    PublicApi,
    /// Orchestration, scheduling, publication policy.
    ControlPlane,
    /// Pack and result metadata.
    Catalog,
    /// Content-addressed blobs.
    ArtifactService,
    /// Where packs and images are built.
    BuildService,
    /// Where the benchmarked agent runs. Hostile by assumption (13.01, decision 1).
    AgentSandbox,
    /// Where the grader and hidden oracle run. Also hostile by assumption, in the other direction:
    /// 13.05 requires that a malicious grader cannot corrupt trial evidence.
    EvaluatorSandbox,
    /// Human review with access to embargoed detail.
    TrustedReview,
    /// A tenant-operated worker (13.19).
    PrivateWorker,
    /// A third-party model API.
    ModelProvider,
    /// A mirror of the public registry.
    PublicRegistryMirror,
}

impl TrustZone {
    pub fn as_str(self) -> &'static str {
        match self {
            TrustZone::UserClient => "user_client",
            TrustZone::PublicApi => "public_api",
            TrustZone::ControlPlane => "control_plane",
            TrustZone::Catalog => "catalog",
            TrustZone::ArtifactService => "artifact_service",
            TrustZone::BuildService => "build_service",
            TrustZone::AgentSandbox => "agent_sandbox",
            TrustZone::EvaluatorSandbox => "evaluator_sandbox",
            TrustZone::TrustedReview => "trusted_review",
            TrustZone::PrivateWorker => "private_worker",
            TrustZone::ModelProvider => "model_provider",
            TrustZone::PublicRegistryMirror => "public_registry_mirror",
        }
    }

    /// Zones whose contents 13.01 tells us to assume are adversary-controlled.
    pub fn is_hostile_by_assumption(self) -> bool {
        matches!(
            self,
            TrustZone::AgentSandbox
                | TrustZone::EvaluatorSandbox
                | TrustZone::PublicRegistryMirror
                | TrustZone::ModelProvider
        )
    }
}

impl fmt::Display for TrustZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The named ways something may move between zones.
///
/// Channels are part of the edge identity because 13.05's separation is not "the agent may not talk
/// to the evaluator" — it is "the agent may deposit a sealed bundle and nothing else". An edge
/// without a channel could not express that.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    /// 13.05's "sealed output/state bundle": the agent's only outbound path.
    SealedOutputBundle,
    /// The evaluator's only outbound path: typed claims, never code the control plane runs.
    TypedClaim,
    /// Read-only task inputs mounted into a sandbox.
    ReadOnlyInput,
    /// Hidden oracle assets, mounted just-in-time into the evaluator only.
    HiddenOracleMount,
    /// A digest-addressed artifact fetch.
    ArtifactFetch,
    /// A control-plane API call.
    ControlPlaneApi,
    /// An outbound call to a model provider.
    ProviderApi,
    /// A human carrying a decision across.
    HumanReview,
    /// A pack or image published to the registry.
    Publication,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::SealedOutputBundle => "sealed_output_bundle",
            Channel::TypedClaim => "typed_claim",
            Channel::ReadOnlyInput => "read_only_input",
            Channel::HiddenOracleMount => "hidden_oracle_mount",
            Channel::ArtifactFetch => "artifact_fetch",
            Channel::ControlPlaneApi => "control_plane_api",
            Channel::ProviderApi => "provider_api",
            Channel::HumanReview => "human_review",
            Channel::Publication => "publication",
        }
    }
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of thing is moving. Some kinds are confined regardless of which edges exist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// Whatever the agent produced.
    AgentOutput,
    /// Hidden tests, holdout labels, oracle data. 13.05: never visible to the agent.
    HiddenOracleAsset,
    /// A typed claim from the grader.
    GraderClaim,
    /// Pack metadata.
    PackManifest,
    /// A key or token. 13.03: no ambient credentials anywhere near a sandbox.
    Credential,
    /// Execution trace or log.
    Trace,
    /// A result accepted for publication.
    PublishedResult,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::AgentOutput => "agent_output",
            ArtifactKind::HiddenOracleAsset => "hidden_oracle_asset",
            ArtifactKind::GraderClaim => "grader_claim",
            ArtifactKind::PackManifest => "pack_manifest",
            ArtifactKind::Credential => "credential",
            ArtifactKind::Trace => "trace",
            ArtifactKind::PublishedResult => "published_result",
        }
    }

    /// Zones this kind may never enter, whatever the edge map says.
    ///
    /// The edge map describes routes; this describes contents. An artifact kind can be forbidden in
    /// a zone that is otherwise perfectly reachable, and conflating the two is how a hidden oracle
    /// ends up in a sandbox that was allowed to receive "inputs".
    pub fn forbidden_in(self) -> &'static [TrustZone] {
        match self {
            ArtifactKind::HiddenOracleAsset => &[
                TrustZone::AgentSandbox,
                TrustZone::PublicApi,
                TrustZone::PublicRegistryMirror,
                TrustZone::ModelProvider,
            ],
            ArtifactKind::Credential => &[
                TrustZone::AgentSandbox,
                TrustZone::EvaluatorSandbox,
                TrustZone::PublicRegistryMirror,
            ],
            _ => &[],
        }
    }
}

impl fmt::Display for ArtifactKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Something with an origin, asked to go somewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MovingArtifact {
    pub id: String,
    pub kind: ArtifactKind,
    pub origin: TrustZone,
}

impl MovingArtifact {
    pub fn new(id: impl Into<String>, kind: ArtifactKind, origin: TrustZone) -> Self {
        MovingArtifact {
            id: id.into(),
            kind,
            origin,
        }
    }
}

/// A crossing the model permitted. Not a crossing anyone watched happen.
///
/// Fields are private and there is no `Deserialize`, so the only way to obtain one is
/// [`BoundaryModel::deliver`] returning `Ok`. A `Crossing` value is therefore evidence that the
/// model was consulted and did not object — which is a weaker claim than it looks, and exactly the
/// claim [`Crossing::honest_label`] prints. Adding `Deserialize` would let a stored crossing be read
/// back into a value that never passed the gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Crossing {
    artifact: String,
    kind: ArtifactKind,
    from: TrustZone,
    to: TrustZone,
    via: Channel,
}

impl Crossing {
    pub fn artifact(&self) -> &str {
        &self.artifact
    }

    pub fn kind(&self) -> ArtifactKind {
        self.kind
    }

    pub fn from(&self) -> TrustZone {
        self.from
    }

    pub fn to(&self) -> TrustZone {
        self.to
    }

    pub fn via(&self) -> Channel {
        self.via
    }

    /// The sentence that belongs beside this record wherever it is shown.
    pub fn honest_label(&self) -> String {
        format!(
            "the model permits {} from {} to {} via {}; nothing observed the transfer",
            self.artifact, self.from, self.to, self.via
        )
    }
}

/// When an edge's influence lands.
///
/// The distinction the diode claim turns on. An edge that only carries influence into a *later*
/// trial does not let the evaluator change the run it is grading, but it does let the evaluator
/// change what gets graded next, and calling both "no influence" would lose the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeScope {
    /// Influence lands inside the trial currently executing.
    WithinTrial,
    /// Influence lands on some future trial, through scheduling, selection or catalog state.
    AcrossTrials,
}

impl EdgeScope {
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeScope::WithinTrial => "within_trial",
            EdgeScope::AcrossTrials => "across_trials",
        }
    }
}

impl fmt::Display for EdgeScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A directed influence graph over trust zones, with channels and scopes on the edges.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryModel {
    edges: BTreeMap<(TrustZone, TrustZone), BTreeMap<Channel, EdgeScope>>,
}

impl BoundaryModel {
    pub fn new() -> Self {
        BoundaryModel::default()
    }

    /// Adds a within-trial edge.
    pub fn allow(self, from: TrustZone, to: TrustZone, via: Channel) -> Self {
        self.allow_scoped(from, to, via, EdgeScope::WithinTrial)
    }

    /// Adds an edge whose influence lands on a later trial.
    pub fn allow_across_trials(self, from: TrustZone, to: TrustZone, via: Channel) -> Self {
        self.allow_scoped(from, to, via, EdgeScope::AcrossTrials)
    }

    pub fn allow_scoped(
        mut self,
        from: TrustZone,
        to: TrustZone,
        via: Channel,
        scope: EdgeScope,
    ) -> Self {
        self.edges.entry((from, to)).or_default().insert(via, scope);
        self
    }

    /// The channels the model allows from one zone to another. Empty means closed.
    pub fn channels(&self, from: TrustZone, to: TrustZone) -> BTreeSet<Channel> {
        self.edges
            .get(&(from, to))
            .map(|channels| channels.keys().copied().collect())
            .unwrap_or_default()
    }

    pub fn permits(&self, from: TrustZone, to: TrustZone, via: Channel) -> bool {
        self.edges
            .get(&(from, to))
            .is_some_and(|channels| channels.contains_key(&via))
    }

    /// The scope of a specific edge, or `None` when the model does not have it.
    pub fn scope_of(&self, from: TrustZone, to: TrustZone, via: Channel) -> Option<EdgeScope> {
        self.edges.get(&(from, to))?.get(&via).copied()
    }

    /// The gate. Refuses an unmodelled edge, a wrong channel, or a forbidden destination for the
    /// artifact's kind.
    pub fn deliver(
        &self,
        artifact: &MovingArtifact,
        to: TrustZone,
        via: Channel,
    ) -> Result<Crossing, SafetyError> {
        if artifact.kind.forbidden_in().contains(&to) {
            return Err(SafetyError::BoundaryViolation {
                artifact: artifact.id.clone(),
                from: artifact.origin.to_string(),
                to: to.to_string(),
                channel: Some(via.to_string()),
                reason: format!("a {} may never enter {to}", artifact.kind),
            });
        }
        let channels = self.channels(artifact.origin, to);
        if channels.is_empty() {
            return Err(SafetyError::UnmodelledEdge {
                from: artifact.origin.to_string(),
                to: to.to_string(),
            });
        }
        if !channels.contains(&via) {
            let permitted: Vec<&str> = channels.iter().map(|c| c.as_str()).collect();
            return Err(SafetyError::BoundaryViolation {
                artifact: artifact.id.clone(),
                from: artifact.origin.to_string(),
                to: to.to_string(),
                channel: Some(via.to_string()),
                reason: format!("only {} is permitted on this edge", permitted.join(", ")),
            });
        }
        Ok(Crossing {
            artifact: artifact.id.clone(),
            kind: artifact.kind,
            from: artifact.origin,
            to,
            via,
        })
    }

    /// Every simple path of influence from one zone to another, as node sequences.
    ///
    /// A path, not a probability and not a boolean. When 13.05's separation fails it fails through
    /// a specific chain of hops, and a reviewer needs the chain.
    pub fn influence_paths(&self, from: TrustZone, to: TrustZone) -> Vec<Vec<TrustZone>> {
        self.paths_over(from, to, &[EdgeScope::WithinTrial, EdgeScope::AcrossTrials])
    }

    /// Paths that land inside the trial currently executing. This is the query 13.05's diode claim
    /// is about.
    pub fn influence_paths_within_trial(
        &self,
        from: TrustZone,
        to: TrustZone,
    ) -> Vec<Vec<TrustZone>> {
        self.paths_over(from, to, &[EdgeScope::WithinTrial])
    }

    fn paths_over(
        &self,
        from: TrustZone,
        to: TrustZone,
        scopes: &[EdgeScope],
    ) -> Vec<Vec<TrustZone>> {
        let mut found = Vec::new();
        let mut stack = vec![from];
        self.walk(from, to, scopes, &mut stack, &mut found);
        found
    }

    fn walk(
        &self,
        current: TrustZone,
        target: TrustZone,
        scopes: &[EdgeScope],
        stack: &mut Vec<TrustZone>,
        found: &mut Vec<Vec<TrustZone>>,
    ) {
        for ((edge_from, edge_to), channels) in self.edges.iter() {
            if *edge_from != current || stack.contains(edge_to) {
                continue;
            }
            if !channels.values().any(|scope| scopes.contains(scope)) {
                continue;
            }
            stack.push(*edge_to);
            if *edge_to == target {
                found.push(stack.clone());
            } else {
                self.walk(*edge_to, target, scopes, stack, found);
            }
            stack.pop();
        }
    }

    /// Whether any influence at all runs from one zone to another, in either scope.
    pub fn reaches(&self, from: TrustZone, to: TrustZone) -> bool {
        !self.influence_paths(from, to).is_empty()
    }

    /// Routes by which the evaluator can influence what the agent is asked, in a later trial.
    ///
    /// 13.14 owns the consequence — an evaluator that shapes its own future inputs is a reward
    /// channel — and this is the structural half of it. The list is expected to be non-empty on
    /// [`BoundaryModel::evaluation_model`]; an empty one would mean the scheduler ignores grades,
    /// which no adaptive evaluation platform does.
    pub fn feedback_loops(&self) -> Vec<Vec<TrustZone>> {
        self.influence_paths(TrustZone::EvaluatorSandbox, TrustZone::AgentSandbox)
    }

    /// The shipped model of 13.05's two-sandbox architecture.
    ///
    /// Read the absences as carefully as the presences. There is no edge out of
    /// [`TrustZone::EvaluatorSandbox`] except [`Channel::TypedClaim`] to the control plane, and
    /// that one edge is [`EdgeScope::AcrossTrials`] — a grade arrives after the run it grades has
    /// ended, so it cannot reach back into that run. What it can do is change the next one, which
    /// is why [`BoundaryModel::feedback_loops`] is non-empty here.
    pub fn evaluation_model() -> Self {
        BoundaryModel::new()
            .allow(
                TrustZone::UserClient,
                TrustZone::PublicApi,
                Channel::ControlPlaneApi,
            )
            .allow(
                TrustZone::PublicApi,
                TrustZone::ControlPlane,
                Channel::ControlPlaneApi,
            )
            .allow(
                TrustZone::ControlPlane,
                TrustZone::AgentSandbox,
                Channel::ReadOnlyInput,
            )
            .allow(
                TrustZone::ControlPlane,
                TrustZone::EvaluatorSandbox,
                Channel::ReadOnlyInput,
            )
            .allow(
                TrustZone::ArtifactService,
                TrustZone::EvaluatorSandbox,
                Channel::HiddenOracleMount,
            )
            .allow(
                TrustZone::AgentSandbox,
                TrustZone::ArtifactService,
                Channel::SealedOutputBundle,
            )
            .allow(
                TrustZone::ArtifactService,
                TrustZone::EvaluatorSandbox,
                Channel::ArtifactFetch,
            )
            .allow_across_trials(
                TrustZone::EvaluatorSandbox,
                TrustZone::ControlPlane,
                Channel::TypedClaim,
            )
            .allow_across_trials(
                TrustZone::ControlPlane,
                TrustZone::Catalog,
                Channel::Publication,
            )
            .allow(
                TrustZone::AgentSandbox,
                TrustZone::ModelProvider,
                Channel::ProviderApi,
            )
    }
}

/// What 13.05 lets an evaluator do, and what it reserves for elsewhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluatorAction {
    /// Permitted: "Evaluator may issue claims and evidence".
    IssueClaim,
    /// Permitted.
    AttachEvidence,
    /// Reserved for the release-policy service.
    PublishResult,
    /// Reserved for the catalog.
    EditPackMetadata,
    /// Reserved for aggregation. A grader that picks its own comparison population picks its own
    /// result.
    SelectComparisonPopulation,
    /// Reserved: "It cannot mutate the original trial."
    MutateTrialEvidence,
    /// Reserved: 13.05's data diode, stated as an action so a caller can ask about it directly.
    ContactAgentSandbox,
}

impl EvaluatorAction {
    pub fn as_str(self) -> &'static str {
        match self {
            EvaluatorAction::IssueClaim => "issue a claim",
            EvaluatorAction::AttachEvidence => "attach evidence",
            EvaluatorAction::PublishResult => "publish a result",
            EvaluatorAction::EditPackMetadata => "edit pack metadata",
            EvaluatorAction::SelectComparisonPopulation => "select the comparison population",
            EvaluatorAction::MutateTrialEvidence => "mutate trial evidence",
            EvaluatorAction::ContactAgentSandbox => "contact the agent sandbox",
        }
    }

    pub fn is_permitted(self) -> bool {
        matches!(
            self,
            EvaluatorAction::IssueClaim | EvaluatorAction::AttachEvidence
        )
    }
}

impl fmt::Display for EvaluatorAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A named evaluator, and the authority check 13.05 asks for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorAuthority {
    pub evaluator: String,
}

impl EvaluatorAuthority {
    pub fn new(evaluator: impl Into<String>) -> Self {
        EvaluatorAuthority {
            evaluator: evaluator.into(),
        }
    }

    /// The permitted set is fixed by the specification, not configurable per evaluator.
    ///
    /// There is no `grant` method here on purpose: an authority list a deployment can extend is an
    /// authority list an attacker with deployment access can extend, and 13.05's separation only
    /// means anything if it is the same everywhere.
    pub fn authorize(&self, action: EvaluatorAction) -> Result<(), SafetyError> {
        if action.is_permitted() {
            Ok(())
        } else {
            Err(SafetyError::EvaluatorOverreach {
                evaluator: self.evaluator.clone(),
                action: action.to_string(),
            })
        }
    }
}

/// Whether tenant isolation was applied. It was not.
///
/// One variant, for the reason `bioprism_sdk::sandbox::Enforcement` has one variant: there is no
/// storage, no encryption namespace, no worker pool and no cache in this process, so nothing here
/// can separate two tenants, and a second variant would let some future refactor say it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TenantIsolation {
    /// Declared in a manifest, applied by nothing.
    DeclaredOnly,
}

impl fmt::Display for TenantIsolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("declared-only")
    }
}

/// A tenant, expressed as a scope, plus the isolation claim that goes with it.
///
/// [`TenantBoundary::admits`] is a genuine check — scope refinement is computed, not asserted — but
/// it checks *declared scopes against each other*. It says whether a resource's declared scope sits
/// inside the tenant's declared scope. It does not say the resource is stored where it claims, and
/// 13.19's "Tenant derived from authenticated principal ... never a client-supplied query filter
/// alone" is precisely the property this crate cannot supply.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantBoundary {
    pub tenant: String,
    pub scope: ScopeKey,
    pub isolation: TenantIsolation,
}

impl TenantBoundary {
    pub fn declared(tenant: impl Into<String>, scope: ScopeKey) -> Self {
        TenantBoundary {
            tenant: tenant.into(),
            scope,
            isolation: TenantIsolation::DeclaredOnly,
        }
    }

    /// Whether a resource's declared scope refines this tenant's.
    pub fn admits(&self, resource: &ScopeKey) -> bool {
        resource.refines(&self.scope)
    }

    /// Always an error. Calling it is how a caller discovers, at the `?`, that the isolation it was
    /// about to depend on is a sentence in a manifest.
    pub fn rely(&self) -> Result<(), SafetyError> {
        Err(SafetyError::UnenforcedReliance {
            threat: format!("cross-tenant access against {}", self.tenant),
            mitigation: "declared tenant isolation".into(),
        })
    }

    pub fn honest_label(&self) -> String {
        format!(
            "tenant {} isolation is {}; this process shares one address space with every tenant it models",
            self.tenant, self.isolation
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_scope::ScopeValue;

    #[test]
    fn nothing_flows_from_the_evaluator_sandbox_back_into_the_trial_it_is_grading() {
        let model = BoundaryModel::evaluation_model();
        assert!(
            model
                .influence_paths_within_trial(TrustZone::EvaluatorSandbox, TrustZone::AgentSandbox)
                .is_empty(),
            "13.05's data diode is one-way within a trial"
        );
    }

    #[test]
    fn the_model_surfaces_the_across_trial_loop_from_grader_to_the_next_agent_input() {
        let model = BoundaryModel::evaluation_model();
        let loops = model.feedback_loops();
        assert_eq!(
            loops,
            vec![vec![
                TrustZone::EvaluatorSandbox,
                TrustZone::ControlPlane,
                TrustZone::AgentSandbox
            ]],
            "the grade shapes the next trial's inputs; 13.05 does not say so and 13.14 pays for it"
        );
        assert_eq!(
            model.scope_of(
                TrustZone::EvaluatorSandbox,
                TrustZone::ControlPlane,
                Channel::TypedClaim
            ),
            Some(EdgeScope::AcrossTrials)
        );
    }

    #[test]
    fn the_only_agent_to_evaluator_path_goes_through_the_sealed_output_bundle() {
        let model = BoundaryModel::evaluation_model();
        let paths = model.influence_paths(TrustZone::AgentSandbox, TrustZone::EvaluatorSandbox);
        assert_eq!(paths.len(), 1, "{paths:?}");
        assert_eq!(
            paths[0],
            vec![
                TrustZone::AgentSandbox,
                TrustZone::ArtifactService,
                TrustZone::EvaluatorSandbox
            ]
        );
        assert!(model.permits(
            TrustZone::AgentSandbox,
            TrustZone::ArtifactService,
            Channel::SealedOutputBundle
        ));
    }

    #[test]
    fn an_unmodelled_edge_is_closed_rather_than_open() {
        let model = BoundaryModel::evaluation_model();
        let bundle =
            MovingArtifact::new("out-1", ArtifactKind::AgentOutput, TrustZone::AgentSandbox);
        let error = model
            .deliver(&bundle, TrustZone::Catalog, Channel::Publication)
            .expect_err("no edge was modelled");
        assert!(matches!(error, SafetyError::UnmodelledEdge { .. }));
    }

    #[test]
    fn a_hidden_oracle_asset_cannot_enter_the_agent_sandbox_even_on_a_modelled_edge() {
        let model = BoundaryModel::evaluation_model().allow(
            TrustZone::ArtifactService,
            TrustZone::AgentSandbox,
            Channel::ArtifactFetch,
        );
        let holdout = MovingArtifact::new(
            "holdout-labels",
            ArtifactKind::HiddenOracleAsset,
            TrustZone::ArtifactService,
        );
        let error = model
            .deliver(&holdout, TrustZone::AgentSandbox, Channel::ArtifactFetch)
            .expect_err("kind confinement outranks the edge map");
        assert!(matches!(error, SafetyError::BoundaryViolation { .. }));
        let ordinary = MovingArtifact::new(
            "task-input",
            ArtifactKind::AgentOutput,
            TrustZone::ArtifactService,
        );
        assert!(model
            .deliver(&ordinary, TrustZone::AgentSandbox, Channel::ArtifactFetch)
            .is_ok());
    }

    #[test]
    fn a_credential_may_not_enter_either_sandbox() {
        let key = MovingArtifact::new(
            "provider-key",
            ArtifactKind::Credential,
            TrustZone::ControlPlane,
        );
        assert!(key.kind.forbidden_in().contains(&TrustZone::AgentSandbox));
        assert!(key
            .kind
            .forbidden_in()
            .contains(&TrustZone::EvaluatorSandbox));
        let model = BoundaryModel::evaluation_model();
        assert!(model
            .deliver(&key, TrustZone::EvaluatorSandbox, Channel::ReadOnlyInput)
            .is_err());
    }

    #[test]
    fn the_right_edge_with_the_wrong_channel_is_refused_and_names_the_permitted_one() {
        let model = BoundaryModel::evaluation_model();
        let claim = MovingArtifact::new(
            "grade-1",
            ArtifactKind::GraderClaim,
            TrustZone::EvaluatorSandbox,
        );
        let error = model
            .deliver(&claim, TrustZone::ControlPlane, Channel::ControlPlaneApi)
            .expect_err("only typed claims leave the evaluator");
        assert!(error.to_string().contains("typed_claim"), "{error}");
        assert!(model
            .deliver(&claim, TrustZone::ControlPlane, Channel::TypedClaim)
            .is_ok());
    }

    #[test]
    fn a_permitted_crossing_records_that_the_model_allowed_it_not_that_it_happened() {
        let model = BoundaryModel::evaluation_model();
        let bundle = MovingArtifact::new(
            "bundle-9",
            ArtifactKind::AgentOutput,
            TrustZone::AgentSandbox,
        );
        let crossing = model
            .deliver(
                &bundle,
                TrustZone::ArtifactService,
                Channel::SealedOutputBundle,
            )
            .expect("the sealed bundle is the agent's outbound path");
        assert!(
            crossing
                .honest_label()
                .contains("nothing observed the transfer"),
            "{}",
            crossing.honest_label()
        );
    }

    #[test]
    fn an_evaluator_may_issue_claims_but_may_not_publish_or_pick_its_comparison_group() {
        let authority = EvaluatorAuthority::new("exact-match-grader");
        assert!(authority.authorize(EvaluatorAction::IssueClaim).is_ok());
        assert!(authority.authorize(EvaluatorAction::AttachEvidence).is_ok());
        for reserved in [
            EvaluatorAction::PublishResult,
            EvaluatorAction::EditPackMetadata,
            EvaluatorAction::SelectComparisonPopulation,
            EvaluatorAction::MutateTrialEvidence,
            EvaluatorAction::ContactAgentSandbox,
        ] {
            assert!(
                matches!(
                    authority.authorize(reserved),
                    Err(SafetyError::EvaluatorOverreach { .. })
                ),
                "{reserved} must be refused"
            );
        }
    }

    #[test]
    fn both_sandboxes_are_hostile_by_assumption_not_just_the_agent_one() {
        assert!(TrustZone::AgentSandbox.is_hostile_by_assumption());
        assert!(TrustZone::EvaluatorSandbox.is_hostile_by_assumption());
        assert!(!TrustZone::ControlPlane.is_hostile_by_assumption());
    }

    #[test]
    fn tenant_isolation_can_never_be_recorded_as_applied_and_relying_on_it_is_an_error() {
        let tenant = ScopeKey::default().exact("tenant", "acme");
        let boundary = TenantBoundary::declared("acme", tenant.clone());
        assert_eq!(boundary.isolation, TenantIsolation::DeclaredOnly);
        assert!(matches!(
            boundary.rely().expect_err("nothing isolates anything here"),
            SafetyError::UnenforcedReliance { .. }
        ));
        assert!(boundary.honest_label().contains("one address space"));
    }

    #[test]
    fn a_tenant_boundary_admits_a_finer_scope_and_refuses_a_sibling() {
        let boundary =
            TenantBoundary::declared("acme", ScopeKey::default().exact("tenant", "acme"));
        let inside = ScopeKey::default()
            .exact("tenant", "acme")
            .bind("project", ScopeValue::Exact("p1".into()));
        let sibling = ScopeKey::default().exact("tenant", "globex");
        assert!(boundary.admits(&inside));
        assert!(!boundary.admits(&sibling));
    }

    #[test]
    fn influence_paths_are_simple_paths_and_terminate_on_a_cycle() {
        let cyclic = BoundaryModel::new()
            .allow(
                TrustZone::ControlPlane,
                TrustZone::Catalog,
                Channel::Publication,
            )
            .allow(
                TrustZone::Catalog,
                TrustZone::ControlPlane,
                Channel::ControlPlaneApi,
            )
            .allow(
                TrustZone::Catalog,
                TrustZone::PublicApi,
                Channel::ControlPlaneApi,
            );
        let paths = cyclic.influence_paths(TrustZone::ControlPlane, TrustZone::PublicApi);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].len(), 3);
    }
}
