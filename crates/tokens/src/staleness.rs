//! Staleness, TTL, invalidation and recomputation (39.18).
//!
//! Compiled context goes stale when the world moves under it. 39.18 states four invariants, and the
//! fourth — *"stale context is never silently reused"* — is the one this module is built around.
//! Silently is the load-bearing word: a compiler that reuses a stale capsule and says so is making
//! a decision, and one that reuses it without saying so has removed the decision from anyone's
//! view.
//!
//! # The rule: a stale context and a fresh one must not be indistinguishable
//!
//! [`Currency`] has five variants and **no `is_fresh`**. It follows `bioprism-hubapi`'s `Freshness`,
//! which took the same position for registry mirrors and for the same reason. In particular:
//!
//! - *"I could not check"* is [`Currency::Undetermined`], a third state. It is what an offline
//!   caller gets when it supplies no reference epoch, and it is emphatically not a synonym for
//!   fresh. Proceeding on it requires [`ReusePolicy::accept_undetermined`], and because that is a
//!   value it ends up in the record: a deployment that chose to trust its cache is then
//!   distinguishable from one that never noticed.
//! - [`Currency::WithinDeclaredValidity`] is named for whose claim it reports. The context declared
//!   its own TTL; nothing here verifies the declaration was wise.
//!
//! # There is no clock
//!
//! Staleness is judged against a [`ContextEpoch`] the caller supplies, or against a [`WorldDigest`]
//! the caller observed, and never against wall time. `bioprism-governance` and `bioprism-hubapi`
//! both reached this conclusion first: a TTL measured against the host's date makes the same bundle
//! fresh on one machine and stale on another, which is a reproducibility defect wearing a freshness
//! feature's clothes.
//!
//! # Biological valid time is not cache time
//!
//! 39.18's first invariant. A capsule can be perfectly current as a cache entry and refer to a
//! decision epoch whose biological facts have since been superseded by a reclassification, and the
//! two failures need different repairs. [`ValidityDeclaration`] carries both axes and
//! [`Currency::Expired`] names the [`ValidityAxis`] that failed, so "the cache is old" and "the
//! diagnosis was revised" never arrive as the same error.
//!
//! # Recomputation says what it would do before doing it
//!
//! [`plan_recomputation`] returns a [`RecomputationPlan`]: which units it would recompute, which
//! upstream change triggered each, why, and what it would leave alone. Nothing in this module
//! recomputes anything — there is no compiler here — so the plan is the entire product, which is
//! also what makes 39.18's "downstream impact graph" output inspectable rather than a side effect.
//!
//! A [`FanOutCeiling`] turns 39.18's named "recursive invalidation storm" failure into
//! [`StalenessError::InvalidationFanOutExceeded`], a typed refusal carrying the fan-out and the
//! ceiling, rather than a plan that quietly proposes rebuilding everything.
//!
//! # Not implemented
//!
//! No cache, no event bus, no continuation notification. 39.18's `ContextCache` and "world event
//! bus" interfaces are infrastructure; [`InvalidationGraph`] is a value describing dependencies
//! somebody else recorded, and [`RecomputationPlan`] is a value describing work somebody else will
//! do.

use crate::error::StalenessError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

/// A monotone counter standing in for time.
///
/// Epochs rather than instants, because there is no clock here. An epoch is whatever the operator
/// increments — a world revision, a data release, a run counter — and its only required property is
/// that it does not go backwards.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct ContextEpoch(pub u64);

impl ContextEpoch {
    pub fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for ContextEpoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "epoch {}", self.0)
    }
}

/// A digest of the world state a context was compiled against.
///
/// The alternative to an epoch, and the stronger of the two: an epoch says *when*, a digest says
/// *what*. A caller that can observe the world's current digest can detect staleness exactly rather
/// than by elapsed count.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct WorldDigest(pub String);

impl WorldDigest {
    pub fn new(digest: impl Into<String>) -> Self {
        WorldDigest(digest.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorldDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// How long a cached context claims to remain reusable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "cache_validity", rename_all = "snake_case")]
pub enum CacheValidity {
    /// Reusable for `ttl_epochs` after compilation.
    Ttl { ttl_epochs: u64 },
    /// Reusable while the world digest is unchanged. The precise form.
    UntilWorldChanges { compiled_against: WorldDigest },
    /// Reusable indefinitely. Legal only for a context whose inputs are themselves immutable, and
    /// worth reading with suspicion everywhere else.
    Immutable { argument: String },
}

/// What makes the *biology* in a context still true, as distinct from the cache entry being young.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "biological_validity", rename_all = "snake_case")]
pub enum BiologicalValidity {
    /// The context describes the world as of a decision epoch and makes no claim beyond it. The
    /// ordinary case: a historical decision does not become wrong because the world moved on.
    AsOfDecisionEpoch { decision_epoch: ContextEpoch },
    /// The context asserts a currently-held classification, which a later reclassification
    /// supersedes. 39.15's third invariant forbids overwriting the historical wording, so this
    /// records the supersession rather than editing the context.
    CurrentClassification { subject: String },
}

/// A context's declaration of what would make it stale.
///
/// Both axes are required. A declaration is not a measurement — the context is asserting its own
/// validity and nothing here checks that the assertion was well judged — but a context that
/// declares nothing cannot be checked at all, which is why
/// [`StalenessError::NoDeclaredValidity`] exists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidityDeclaration {
    pub context_id: String,
    pub compiled_at: ContextEpoch,
    pub cache: CacheValidity,
    pub biological: BiologicalValidity,
    /// Version identifiers of the upstream artifacts this context was compiled from. A change in
    /// any of them invalidates this context and, per 39.18's third invariant, only contexts that
    /// depend on it.
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

impl ValidityDeclaration {
    pub fn new(
        context_id: impl Into<String>,
        compiled_at: ContextEpoch,
        cache: CacheValidity,
        biological: BiologicalValidity,
    ) -> Self {
        ValidityDeclaration {
            context_id: context_id.into(),
            compiled_at,
            cache,
            biological,
            dependencies: BTreeMap::new(),
        }
    }

    pub fn depending_on(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.dependencies.insert(name.into(), version.into());
        self
    }

    /// Judge this declaration against what a caller actually observed.
    ///
    /// The whole method is a case analysis over what the observation *contains*, because the
    /// missing-observation cases are the ones that matter. An observation with no reference epoch
    /// and no world digest yields [`Currency::Undetermined`] whatever the TTL says, since a TTL
    /// with nothing to measure elapsed epochs against is an unevaluated promise.
    pub fn assess(&self, observed: &WorldObservation) -> Currency {
        if let Some(superseded) = self.superseded_by(observed) {
            return superseded;
        }
        if let Some(changed) = self.changed_dependency(observed) {
            return changed;
        }
        match &self.cache {
            CacheValidity::Immutable { argument } => Currency::DeclaredImmutable {
                argument: argument.clone(),
            },
            CacheValidity::UntilWorldChanges { compiled_against } => {
                match &observed.world_digest {
                    None => Currency::Undetermined {
                        missing: MissingObservation::WorldDigest,
                        compiled_at: self.compiled_at,
                    },
                    Some(current) if current == compiled_against => {
                        Currency::WithinDeclaredValidity {
                            axis: ValidityAxis::WorldDigest,
                            elapsed: None,
                        }
                    }
                    Some(current) => Currency::Expired {
                        axis: ValidityAxis::WorldDigest,
                        detail: format!(
                            "compiled against world {compiled_against}, observed world {current}"
                        ),
                    },
                }
            }
            CacheValidity::Ttl { ttl_epochs } => match observed.reference_epoch {
                None => Currency::Undetermined {
                    missing: MissingObservation::ReferenceEpoch,
                    compiled_at: self.compiled_at,
                },
                Some(now) if now < self.compiled_at => Currency::CompiledAfterReference {
                    compiled_at: self.compiled_at,
                    reference: now,
                },
                Some(now) => {
                    let elapsed = now.get() - self.compiled_at.get();
                    if elapsed <= *ttl_epochs {
                        Currency::WithinDeclaredValidity {
                            axis: ValidityAxis::CacheTtl,
                            elapsed: Some(elapsed),
                        }
                    } else {
                        Currency::Expired {
                            axis: ValidityAxis::CacheTtl,
                            detail: format!(
                                "{elapsed} epoch(s) since compilation, past the declared ttl of \
                                 {ttl_epochs}"
                            ),
                        }
                    }
                }
            },
        }
    }

    fn superseded_by(&self, observed: &WorldObservation) -> Option<Currency> {
        let BiologicalValidity::CurrentClassification { subject } = &self.biological else {
            return None;
        };
        observed
            .reclassified
            .get(subject)
            .map(|at| Currency::Expired {
                axis: ValidityAxis::BiologicalValidTime,
                detail: format!(
                    "`{subject}` was reclassified at {at}; the context asserts the classification \
                     current as of {}",
                    self.compiled_at
                ),
            })
    }

    fn changed_dependency(&self, observed: &WorldObservation) -> Option<Currency> {
        for (name, pinned) in &self.dependencies {
            match observed.dependency_versions.get(name) {
                Some(current) if current != pinned => {
                    return Some(Currency::Expired {
                        axis: ValidityAxis::Dependency(name.clone()),
                        detail: format!("pinned `{pinned}`, observed `{current}`"),
                    });
                }
                Some(_) => {}
                None => {
                    return Some(Currency::Undetermined {
                        missing: MissingObservation::DependencyVersion(name.clone()),
                        compiled_at: self.compiled_at,
                    })
                }
            }
        }
        None
    }
}

/// Which validity axis a currency verdict is about.
///
/// Present on both the good and the bad outcome, because "it is still within its TTL" and "its
/// world digest still matches" are different strengths of assurance and a consumer should be able
/// to tell which one it got.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "axis", rename_all = "snake_case")]
pub enum ValidityAxis {
    /// The declared time-to-live, measured in supplied epochs.
    CacheTtl,
    /// The digest of the world the context was compiled against.
    WorldDigest,
    /// A named upstream artifact's version.
    Dependency(String),
    /// The biological fact the context asserts, superseded by a later reclassification.
    BiologicalValidTime,
}

impl fmt::Display for ValidityAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidityAxis::CacheTtl => f.write_str("cache ttl"),
            ValidityAxis::WorldDigest => f.write_str("world digest"),
            ValidityAxis::Dependency(name) => write!(f, "dependency `{name}`"),
            ValidityAxis::BiologicalValidTime => f.write_str("biological valid time"),
        }
    }
}

/// What the caller did not supply, when currency could not be established.
///
/// Named rather than collapsed into one "unknown", because each of these has a different fix and a
/// caller told which one is missing can go and get it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "missing", rename_all = "snake_case")]
pub enum MissingObservation {
    /// No reference epoch, so elapsed epochs could not be computed. The ordinary offline case.
    ReferenceEpoch,
    /// No observed world digest to compare the compiled-against digest with.
    WorldDigest,
    /// A declared dependency whose current version was not observed.
    DependencyVersion(String),
}

impl fmt::Display for MissingObservation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MissingObservation::ReferenceEpoch => f.write_str("no reference epoch was supplied"),
            MissingObservation::WorldDigest => f.write_str("no world digest was observed"),
            MissingObservation::DependencyVersion(name) => {
                write!(f, "the current version of dependency `{name}` was not observed")
            }
        }
    }
}

/// How much a cached context's currency is actually known.
///
/// Five outcomes and no boolean that collapses them. There is deliberately no `is_fresh` and no
/// `is_current`: the two methods that exist are named for what they actually report, and
/// [`Currency::is_undetermined`] is the one a cautious caller checks first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "currency", rename_all = "snake_case")]
pub enum Currency {
    /// The context is inside the validity it declared for itself, on the named axis.
    WithinDeclaredValidity {
        axis: ValidityAxis,
        /// Epochs elapsed, when the axis is one that measures elapsed epochs.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed: Option<u64>,
    },
    /// The context is outside its declared validity on the named axis.
    Expired { axis: ValidityAxis, detail: String },
    /// The check could not be performed: the caller did not supply what it needed. This is the
    /// third state, and it is not a synonym for fresh.
    Undetermined {
        missing: MissingObservation,
        compiled_at: ContextEpoch,
    },
    /// The context claims to have been compiled after the reference epoch. Somebody's epochs
    /// disagree, and reporting that as within-validity would launder a bookkeeping fault.
    CompiledAfterReference {
        compiled_at: ContextEpoch,
        reference: ContextEpoch,
    },
    /// The context declared itself immutable and gave an argument. Reported distinctly from
    /// within-validity so a reviewer can see that no check was performed because none was possible.
    DeclaredImmutable { argument: String },
}

impl Currency {
    /// True when the context's *own* declaration was checked and held. Named for whose claim it
    /// reports, because that is the whole content of it.
    ///
    /// False for [`Currency::DeclaredImmutable`]: an immutable declaration is trivially satisfied
    /// because nothing was checked, and folding it in here would make "checked and held"
    /// indistinguishable from "asserted and believed".
    pub fn is_within_declared_validity(&self) -> bool {
        matches!(self, Currency::WithinDeclaredValidity { .. })
    }

    /// True when the context asserted immutability instead of being checked.
    pub fn is_declared_immutable(&self) -> bool {
        matches!(self, Currency::DeclaredImmutable { .. })
    }

    /// True when nothing at all was established about currency.
    pub fn is_undetermined(&self) -> bool {
        matches!(
            self,
            Currency::Undetermined { .. } | Currency::CompiledAfterReference { .. }
        )
    }

    /// True when the declaration was checked and failed.
    pub fn is_expired(&self) -> bool {
        matches!(self, Currency::Expired { .. })
    }

    /// The axis a verdict is about, when it is about one.
    pub fn axis(&self) -> Option<&ValidityAxis> {
        match self {
            Currency::WithinDeclaredValidity { axis, .. } | Currency::Expired { axis, .. } => {
                Some(axis)
            }
            _ => None,
        }
    }
}

impl fmt::Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Currency::WithinDeclaredValidity {
                axis,
                elapsed: Some(elapsed),
            } => write!(f, "within its declared {axis} ({elapsed} epoch(s) elapsed)"),
            Currency::WithinDeclaredValidity { axis, .. } => {
                write!(f, "within its declared {axis}")
            }
            Currency::Expired { axis, detail } => write!(f, "expired against {axis}: {detail}"),
            Currency::Undetermined {
                missing,
                compiled_at,
            } => write!(
                f,
                "of undetermined currency: compiled at {compiled_at} and {missing}"
            ),
            Currency::CompiledAfterReference {
                compiled_at,
                reference,
            } => write!(
                f,
                "claiming compilation at {compiled_at}, later than the reference {reference}"
            ),
            Currency::DeclaredImmutable { argument } => {
                write!(f, "declared immutable: {argument}")
            }
        }
    }
}

/// What a caller actually observed about the world. Everything optional, because offline the honest
/// answer to most of it is "I do not know".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference_epoch: Option<ContextEpoch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub world_digest: Option<WorldDigest>,
    #[serde(default)]
    pub dependency_versions: BTreeMap<String, String>,
    /// Subjects known to have been reclassified, and when. 39.18's second invariant: corrections
    /// and retractions propagate, so they are observations rather than edits.
    #[serde(default)]
    pub reclassified: BTreeMap<String, ContextEpoch>,
}

impl WorldObservation {
    /// The offline default: nothing was observed. Named so the call site reads as a statement.
    pub fn nothing_observed() -> Self {
        WorldObservation::default()
    }

    pub fn at(epoch: ContextEpoch) -> Self {
        WorldObservation {
            reference_epoch: Some(epoch),
            ..WorldObservation::default()
        }
    }

    pub fn with_world(mut self, digest: WorldDigest) -> Self {
        self.world_digest = Some(digest);
        self
    }

    pub fn with_dependency(mut self, name: impl Into<String>, version: impl Into<String>) -> Self {
        self.dependency_versions.insert(name.into(), version.into());
        self
    }

    pub fn with_reclassification(mut self, subject: impl Into<String>, at: ContextEpoch) -> Self {
        self.reclassified.insert(subject.into(), at);
        self
    }
}

/// What a consumer will accept before reusing a cached context.
///
/// The default is strict: only a checked, satisfied declaration. An offline deployment relaxes it
/// explicitly, and because the relaxation is a value it travels into the record — a deployment that
/// decided to trust its cache is then distinguishable from one that never noticed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReusePolicy {
    /// Reuse when currency could not be established. The offline switch.
    pub accept_undetermined: bool,
    /// Reuse past the declared validity. Rarely right, and never a default.
    pub accept_expired: bool,
    /// Reuse a context that declared itself immutable without checking anything.
    pub accept_declared_immutable: bool,
}

impl ReusePolicy {
    /// Nothing is reused unless it was checked and passed.
    pub const STRICT: ReusePolicy = ReusePolicy {
        accept_undetermined: false,
        accept_expired: false,
        accept_declared_immutable: false,
    };

    /// The air-gapped setting: unverifiable currency is accepted, knowingly.
    pub const OFFLINE: ReusePolicy = ReusePolicy {
        accept_undetermined: true,
        accept_expired: false,
        accept_declared_immutable: true,
    };

    /// Decide whether a context may be reused, and refuse with the reason if not.
    ///
    /// This is 39.18's fourth invariant in one function: reuse goes through here or the context was
    /// reused silently.
    pub fn admit(&self, context_id: &str, currency: &Currency) -> Result<(), StalenessError> {
        match currency {
            Currency::WithinDeclaredValidity { .. } => Ok(()),
            Currency::DeclaredImmutable { .. } if self.accept_declared_immutable => Ok(()),
            Currency::DeclaredImmutable { .. } => Err(StalenessError::CurrencyUndetermined {
                context: context_id.to_string(),
                detail: currency.to_string(),
            }),
            Currency::Expired { axis, detail } if !self.accept_expired => {
                Err(StalenessError::Expired {
                    context: context_id.to_string(),
                    axis: axis.to_string(),
                    detail: detail.clone(),
                })
            }
            Currency::Expired { .. } => Ok(()),
            Currency::Undetermined { .. } | Currency::CompiledAfterReference { .. } => {
                if self.accept_undetermined {
                    Ok(())
                } else {
                    Err(StalenessError::CurrencyUndetermined {
                        context: context_id.to_string(),
                        detail: currency.to_string(),
                    })
                }
            }
        }
    }
}

/// A provenance edge: `dependent` was derived from `source`.
///
/// Direction matters and the field names say which way it runs. 39.18's second invariant is that
/// corrections propagate *through* provenance edges, so the graph is traversed from a changed
/// source towards its dependents and never the other way.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProvenanceEdge {
    pub dependent: String,
    pub source: String,
}

impl ProvenanceEdge {
    pub fn new(dependent: impl Into<String>, source: impl Into<String>) -> Self {
        ProvenanceEdge {
            dependent: dependent.into(),
            source: source.into(),
        }
    }
}

/// The dependency graph over cached projections.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidationGraph {
    pub units: BTreeSet<String>,
    pub edges: Vec<ProvenanceEdge>,
}

impl InvalidationGraph {
    pub fn new() -> Self {
        InvalidationGraph::default()
    }

    pub fn with_unit(mut self, unit: impl Into<String>) -> Self {
        self.units.insert(unit.into());
        self
    }

    pub fn derived(mut self, dependent: impl Into<String>, source: impl Into<String>) -> Self {
        let edge = ProvenanceEdge::new(dependent, source);
        self.units.insert(edge.dependent.clone());
        self.units.insert(edge.source.clone());
        self.edges.push(edge);
        self
    }

    /// Structural checks: every edge endpoint present, and no cycles.
    ///
    /// A cycle is refused rather than broken arbitrarily, because invalidation over a cycle either
    /// does not terminate or terminates at a place determined by traversal order, and both are
    /// worse than a refusal that names the unit.
    pub fn validate(&self) -> Result<(), StalenessError> {
        for edge in &self.edges {
            if !self.units.contains(&edge.source) {
                return Err(StalenessError::DanglingProvenanceEdge {
                    from: edge.dependent.clone(),
                    to: edge.source.clone(),
                });
            }
            if !self.units.contains(&edge.dependent) {
                return Err(StalenessError::DanglingProvenanceEdge {
                    from: edge.dependent.clone(),
                    to: edge.source.clone(),
                });
            }
        }
        for unit in &self.units {
            if self.reaches_itself(unit) {
                return Err(StalenessError::ProvenanceCycle(unit.clone()));
            }
        }
        Ok(())
    }

    fn reaches_itself(&self, start: &str) -> bool {
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = self.dependents_of(start).collect();
        while let Some(next) = queue.pop_front() {
            if next == start {
                return true;
            }
            if !seen.insert(next) {
                continue;
            }
            queue.extend(self.dependents_of(next));
        }
        false
    }

    fn dependents_of<'a>(&'a self, unit: &'a str) -> impl Iterator<Item = &'a str> {
        self.edges
            .iter()
            .filter(move |edge| edge.source == unit)
            .map(|edge| edge.dependent.as_str())
    }

    /// Everything downstream of the changed units, and everything that is not.
    ///
    /// 39.18's third invariant is that a version change invalidates *only* dependent projections,
    /// so the report carries the untouched set explicitly. A caller that only ever sees the
    /// affected list cannot tell a targeted invalidation from one that swept the cache.
    pub fn impact(&self, changed: &[String]) -> Result<ImpactReport, StalenessError> {
        self.validate()?;
        let mut affected: BTreeSet<String> = BTreeSet::new();
        let mut trigger: BTreeMap<String, String> = BTreeMap::new();
        let mut depth: BTreeMap<String, usize> = BTreeMap::new();
        let mut queue: VecDeque<(String, String, usize)> = VecDeque::new();
        for unit in changed {
            if !self.units.contains(unit) {
                return Err(StalenessError::DanglingProvenanceEdge {
                    from: unit.clone(),
                    to: unit.clone(),
                });
            }
            if affected.insert(unit.clone()) {
                trigger.insert(unit.clone(), unit.clone());
                depth.insert(unit.clone(), 0);
                queue.push_back((unit.clone(), unit.clone(), 0));
            }
        }
        while let Some((unit, root, level)) = queue.pop_front() {
            let downstream: Vec<String> = self
                .dependents_of(&unit)
                .map(|dependent| dependent.to_string())
                .collect();
            for dependent in downstream {
                if affected.insert(dependent.clone()) {
                    trigger.insert(dependent.clone(), root.clone());
                    depth.insert(dependent.clone(), level + 1);
                    queue.push_back((dependent, root.clone(), level + 1));
                }
            }
        }
        let unaffected = self.units.difference(&affected).cloned().collect();
        Ok(ImpactReport {
            changed: changed.iter().cloned().collect(),
            affected,
            unaffected,
            trigger,
            depth,
            total_units: self.units.len(),
        })
    }
}

/// The downstream impact graph of 39.18, as a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    pub changed: BTreeSet<String>,
    /// The changed units and everything transitively derived from them.
    pub affected: BTreeSet<String>,
    /// Everything else. Carried explicitly so a targeted invalidation is legible as targeted.
    pub unaffected: BTreeSet<String>,
    /// For each affected unit, the changed unit that reached it.
    pub trigger: BTreeMap<String, String>,
    /// Provenance hops from the trigger. Zero for the changed units themselves.
    pub depth: BTreeMap<String, usize>,
    pub total_units: usize,
}

impl ImpactReport {
    pub fn fan_out(&self) -> usize {
        self.affected.len()
    }

    pub fn touches(&self, unit: &str) -> bool {
        self.affected.contains(unit)
    }
}

/// A ceiling on how much of the cache one change may invalidate.
///
/// 39.18 names "recursive invalidation storm" as a failure mode. A ceiling turns it into a typed
/// refusal that a caller can act on, rather than a plan that proposes rebuilding the world and
/// leaves somebody to notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanOutCeiling {
    pub max_units: usize,
}

impl FanOutCeiling {
    pub fn units(max_units: usize) -> Self {
        FanOutCeiling { max_units }
    }
}

/// Why one unit would be recomputed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "recompute_reason", rename_all = "snake_case")]
pub enum RecomputeReason {
    /// This unit is one of the changed ones.
    SourceChanged,
    /// This unit is derived, transitively, from a changed one.
    DerivedFromChanged { trigger: String, hops: usize },
    /// This unit's own declared validity failed, independently of the change set.
    ValidityFailed { currency: String },
    /// This unit's currency could not be established, and the policy does not accept that.
    CurrencyUndetermined { detail: String },
}

impl RecomputeReason {
    pub fn describe(&self) -> String {
        match self {
            RecomputeReason::SourceChanged => "its source changed".to_string(),
            RecomputeReason::DerivedFromChanged { trigger, hops } => {
                format!("it is derived from `{trigger}` across {hops} provenance hop(s)")
            }
            RecomputeReason::ValidityFailed { currency } => {
                format!("it is {currency}")
            }
            RecomputeReason::CurrencyUndetermined { detail } => {
                format!("its currency could not be established: {detail}")
            }
        }
    }
}

/// One unit the plan would recompute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecomputationUnit {
    pub unit: String,
    pub reason: RecomputeReason,
}

/// What a recomputation would do, stated before it does it.
///
/// The plan is the product. Nothing here executes; a caller reads [`RecomputationPlan::explain`],
/// decides, and then performs the work with a compiler this crate does not contain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecomputationPlan {
    pub units: Vec<RecomputationUnit>,
    /// Units the plan deliberately leaves alone. Present so "only dependents were invalidated" is
    /// checkable rather than asserted.
    pub retained: BTreeSet<String>,
    pub total_units: usize,
}

impl RecomputationPlan {
    pub fn would_recompute(&self) -> BTreeSet<String> {
        self.units.iter().map(|unit| unit.unit.clone()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }

    pub fn fan_out(&self) -> usize {
        self.units.len()
    }

    /// One line per unit, saying what would be recomputed and why.
    pub fn explain(&self) -> Vec<String> {
        let mut lines: Vec<String> = self
            .units
            .iter()
            .map(|unit| format!("recompute `{}` because {}", unit.unit, unit.reason.describe()))
            .collect();
        lines.sort();
        lines
    }
}

/// Plan the minimal recomputation for a set of changed sources plus a set of independently checked
/// contexts.
///
/// Two inputs because 39.18 has two triggers: an upstream change, and a context whose own
/// declaration expired. A unit can be reached by either, and a unit reached by both is reported
/// under the change that reached it first in provenance order, which is deterministic given the
/// graph.
///
/// `ceiling` refuses a storm rather than proposing one.
pub fn plan_recomputation(
    graph: &InvalidationGraph,
    changed: &[String],
    checked: &BTreeMap<String, Currency>,
    policy: ReusePolicy,
    ceiling: Option<FanOutCeiling>,
) -> Result<RecomputationPlan, StalenessError> {
    let impact = graph.impact(changed)?;
    let mut units: Vec<RecomputationUnit> = Vec::new();
    let mut selected: BTreeSet<String> = BTreeSet::new();

    for unit in &impact.affected {
        let hops = impact.depth.get(unit).copied().unwrap_or(0);
        let reason = if hops == 0 {
            RecomputeReason::SourceChanged
        } else {
            RecomputeReason::DerivedFromChanged {
                trigger: impact
                    .trigger
                    .get(unit)
                    .cloned()
                    .unwrap_or_else(|| unit.clone()),
                hops,
            }
        };
        selected.insert(unit.clone());
        units.push(RecomputationUnit {
            unit: unit.clone(),
            reason,
        });
    }

    for (unit, currency) in checked {
        if selected.contains(unit) {
            continue;
        }
        let reason = match policy.admit(unit, currency) {
            Ok(()) => continue,
            Err(StalenessError::Expired { .. }) => RecomputeReason::ValidityFailed {
                currency: currency.to_string(),
            },
            Err(_) => RecomputeReason::CurrencyUndetermined {
                detail: currency.to_string(),
            },
        };
        selected.insert(unit.clone());
        units.push(RecomputationUnit {
            unit: unit.clone(),
            reason,
        });
    }

    units.sort_by(|left, right| left.unit.cmp(&right.unit));

    if let Some(ceiling) = ceiling {
        if units.len() > ceiling.max_units {
            return Err(StalenessError::InvalidationFanOutExceeded {
                changed: changed.len(),
                fan_out: units.len(),
                total: graph.units.len(),
                ceiling: ceiling.max_units,
            });
        }
    }

    let retained = graph.units.difference(&selected).cloned().collect();
    Ok(RecomputationPlan {
        units,
        retained,
        total_units: graph.units.len(),
    })
}

/// A change to one upstream artifact, for a caller assembling a change set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyChange {
    pub unit: String,
    pub from_version: String,
    pub to_version: String,
}

impl DependencyChange {
    pub fn new(
        unit: impl Into<String>,
        from_version: impl Into<String>,
        to_version: impl Into<String>,
    ) -> Self {
        DependencyChange {
            unit: unit.into(),
            from_version: from_version.into(),
            to_version: to_version.into(),
        }
    }

    /// A change is only a change if the versions differ. A no-op version bump must not invalidate
    /// anything, which is 39.18's third invariant read in the trivial direction.
    pub fn is_material(&self) -> bool {
        self.from_version != self.to_version
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttl_context(compiled_at: u64, ttl: u64) -> ValidityDeclaration {
        ValidityDeclaration::new(
            "ctx/a",
            ContextEpoch(compiled_at),
            CacheValidity::Ttl { ttl_epochs: ttl },
            BiologicalValidity::AsOfDecisionEpoch {
                decision_epoch: ContextEpoch(compiled_at),
            },
        )
    }

    fn world_context(digest: &str) -> ValidityDeclaration {
        ValidityDeclaration::new(
            "ctx/w",
            ContextEpoch(3),
            CacheValidity::UntilWorldChanges {
                compiled_against: WorldDigest::new(digest),
            },
            BiologicalValidity::AsOfDecisionEpoch {
                decision_epoch: ContextEpoch(3),
            },
        )
    }

    #[test]
    fn a_context_whose_freshness_could_not_be_checked_is_not_reported_as_fresh() {
        let currency = ttl_context(2, 5).assess(&WorldObservation::nothing_observed());
        assert!(matches!(
            currency,
            Currency::Undetermined {
                missing: MissingObservation::ReferenceEpoch,
                ..
            }
        ));
        assert!(!currency.is_within_declared_validity());
        assert!(currency.is_undetermined());
        assert!(!currency.is_expired());
    }

    #[test]
    fn a_stale_context_and_a_fresh_one_are_different_values_and_not_a_flag() {
        let fresh = ttl_context(2, 5).assess(&WorldObservation::at(ContextEpoch(4)));
        let stale = ttl_context(2, 5).assess(&WorldObservation::at(ContextEpoch(20)));
        assert_ne!(fresh, stale);
        assert!(fresh.is_within_declared_validity());
        assert!(stale.is_expired());
        assert!(stale.to_string().contains("past the declared ttl"));
    }

    #[test]
    fn there_is_no_way_to_ask_a_currency_whether_it_is_fresh() {
        let currency = ttl_context(2, 5).assess(&WorldObservation::at(ContextEpoch(3)));
        let reported: Vec<bool> = vec![
            currency.is_within_declared_validity(),
            currency.is_undetermined(),
            currency.is_expired(),
        ];
        assert_eq!(reported, vec![true, false, false]);
    }

    #[test]
    fn a_world_digest_that_still_matches_is_a_stronger_answer_than_an_unexpired_ttl() {
        let matched = world_context("abc")
            .assess(&WorldObservation::nothing_observed().with_world(WorldDigest::new("abc")));
        assert_eq!(
            matched.axis(),
            Some(&ValidityAxis::WorldDigest),
            "the verdict says which axis established it"
        );
        assert!(matched.is_within_declared_validity());
    }

    #[test]
    fn a_changed_world_digest_expires_the_context_and_reports_both_digests() {
        let moved = world_context("abc")
            .assess(&WorldObservation::nothing_observed().with_world(WorldDigest::new("def")));
        assert!(moved.is_expired());
        let text = moved.to_string();
        assert!(text.contains("abc") && text.contains("def"));
    }

    #[test]
    fn a_world_pinned_context_with_no_observed_digest_is_undetermined_rather_than_assumed_good() {
        let currency = world_context("abc").assess(&WorldObservation::at(ContextEpoch(99)));
        assert!(matches!(
            currency,
            Currency::Undetermined {
                missing: MissingObservation::WorldDigest,
                ..
            }
        ));
    }

    #[test]
    fn staleness_is_never_measured_against_wall_time_only_against_a_supplied_epoch() {
        let declaration = ttl_context(2, 5);
        let unchecked = declaration.assess(&WorldObservation::nothing_observed());
        let checked = declaration.assess(&WorldObservation::at(ContextEpoch(2)));
        assert!(unchecked.is_undetermined());
        assert!(checked.is_within_declared_validity());
        for _ in 0..5 {
            assert_eq!(
                declaration.assess(&WorldObservation::nothing_observed()),
                unchecked
            );
        }
    }

    #[test]
    fn a_context_compiled_after_the_reference_epoch_is_reported_rather_than_rounded_to_zero_lag() {
        let currency = ttl_context(30, 5).assess(&WorldObservation::at(ContextEpoch(9)));
        assert!(matches!(currency, Currency::CompiledAfterReference { .. }));
        assert!(!currency.is_within_declared_validity());
        assert!(currency.is_undetermined());
    }

    #[test]
    fn a_changed_dependency_version_expires_a_context_that_was_otherwise_inside_its_ttl() {
        let declaration = ttl_context(2, 100).depending_on("gencode", "v44");
        let observation = WorldObservation::at(ContextEpoch(3)).with_dependency("gencode", "v45");
        let currency = declaration.assess(&observation);
        assert!(currency.is_expired());
        assert_eq!(
            currency.axis(),
            Some(&ValidityAxis::Dependency("gencode".to_string()))
        );
    }

    #[test]
    fn an_unobserved_dependency_version_is_undetermined_and_names_which_dependency() {
        let declaration = ttl_context(2, 100).depending_on("gencode", "v44");
        let currency = declaration.assess(&WorldObservation::at(ContextEpoch(3)));
        assert!(matches!(
            currency,
            Currency::Undetermined {
                missing: MissingObservation::DependencyVersion(ref name),
                ..
            } if name == "gencode"
        ));
    }

    #[test]
    fn biological_supersession_and_cache_expiry_are_reported_on_different_axes() {
        let declaration = ValidityDeclaration::new(
            "ctx/dx",
            ContextEpoch(1),
            CacheValidity::Ttl { ttl_epochs: 1000 },
            BiologicalValidity::CurrentClassification {
                subject: "tumor/1".to_string(),
            },
        );
        let observation = WorldObservation::at(ContextEpoch(2))
            .with_reclassification("tumor/1", ContextEpoch(2));
        let currency = declaration.assess(&observation);
        assert_eq!(currency.axis(), Some(&ValidityAxis::BiologicalValidTime));
        assert!(currency.is_expired());
    }

    #[test]
    fn a_reclassification_of_an_unrelated_subject_does_not_expire_a_context() {
        let declaration = ValidityDeclaration::new(
            "ctx/dx",
            ContextEpoch(1),
            CacheValidity::Ttl { ttl_epochs: 1000 },
            BiologicalValidity::CurrentClassification {
                subject: "tumor/1".to_string(),
            },
        );
        let observation = WorldObservation::at(ContextEpoch(2))
            .with_reclassification("tumor/2", ContextEpoch(2));
        assert!(declaration.assess(&observation).is_within_declared_validity());
    }

    #[test]
    fn the_strict_policy_refuses_an_undetermined_context_and_the_offline_policy_admits_it() {
        let currency = ttl_context(2, 5).assess(&WorldObservation::nothing_observed());
        assert!(matches!(
            ReusePolicy::STRICT.admit("ctx/a", &currency),
            Err(StalenessError::CurrencyUndetermined { .. })
        ));
        assert!(ReusePolicy::OFFLINE.admit("ctx/a", &currency).is_ok());
    }

    #[test]
    fn even_the_offline_policy_refuses_an_expired_context() {
        let expired = ttl_context(2, 1).assess(&WorldObservation::at(ContextEpoch(40)));
        assert!(matches!(
            ReusePolicy::OFFLINE.admit("ctx/a", &expired),
            Err(StalenessError::Expired { .. })
        ));
    }

    #[test]
    fn accepting_an_unverifiable_context_is_a_recorded_decision_and_not_a_default() {
        let default = ReusePolicy::default();
        assert!(!default.accept_undetermined);
        assert!(!default.accept_expired);
        assert!(!default.accept_declared_immutable);
    }

    fn chain() -> InvalidationGraph {
        InvalidationGraph::new()
            .derived("view/idh", "assay/idh")
            .derived("capsule/molecular", "view/idh")
            .derived("capsule/board", "capsule/molecular")
            .derived("view/mgmt", "assay/mgmt")
            .with_unit("assay/unrelated")
    }

    #[test]
    fn a_version_change_invalidates_dependents_and_the_report_names_what_it_left_alone() {
        let impact = chain()
            .impact(&["assay/idh".to_string()])
            .expect("computes impact");
        assert!(impact.touches("capsule/board"));
        assert!(!impact.touches("view/mgmt"));
        assert!(impact.unaffected.contains("view/mgmt"));
        assert!(impact.unaffected.contains("assay/unrelated"));
    }

    #[test]
    fn every_affected_unit_names_the_change_that_reached_it_and_how_far_away_it_was() {
        let impact = chain()
            .impact(&["assay/idh".to_string()])
            .expect("computes impact");
        assert_eq!(impact.depth.get("assay/idh"), Some(&0));
        assert_eq!(impact.depth.get("view/idh"), Some(&1));
        assert_eq!(impact.depth.get("capsule/board"), Some(&3));
        assert_eq!(
            impact.trigger.get("capsule/board"),
            Some(&"assay/idh".to_string())
        );
    }

    #[test]
    fn a_provenance_cycle_is_refused_rather_than_broken_at_an_arbitrary_edge() {
        let cyclic = InvalidationGraph::new()
            .derived("a", "b")
            .derived("b", "c")
            .derived("c", "a");
        assert!(matches!(
            cyclic.validate(),
            Err(StalenessError::ProvenanceCycle(_))
        ));
    }

    #[test]
    fn an_edge_to_a_unit_the_graph_does_not_contain_is_refused() {
        let mut graph = InvalidationGraph::new().with_unit("a");
        graph.edges.push(ProvenanceEdge::new("a", "ghost"));
        assert!(matches!(
            graph.validate(),
            Err(StalenessError::DanglingProvenanceEdge { .. })
        ));
    }

    #[test]
    fn a_recomputation_plan_says_what_it_would_recompute_and_why_before_anything_runs() {
        let plan = plan_recomputation(
            &chain(),
            &["assay/idh".to_string()],
            &BTreeMap::new(),
            ReusePolicy::STRICT,
            None,
        )
        .expect("plans");
        let explanation = plan.explain();
        assert!(explanation
            .iter()
            .any(|line| line.contains("capsule/board") && line.contains("provenance hop")));
        assert!(explanation
            .iter()
            .any(|line| line.contains("assay/idh") && line.contains("its source changed")));
        assert!(plan.retained.contains("view/mgmt"));
    }

    #[test]
    fn a_context_whose_own_validity_failed_is_recomputed_even_with_no_upstream_change() {
        let mut checked = BTreeMap::new();
        checked.insert(
            "view/mgmt".to_string(),
            ttl_context(1, 1).assess(&WorldObservation::at(ContextEpoch(50))),
        );
        let plan = plan_recomputation(&chain(), &[], &checked, ReusePolicy::STRICT, None)
            .expect("plans");
        assert_eq!(
            plan.would_recompute().into_iter().collect::<Vec<_>>(),
            vec!["view/mgmt".to_string()]
        );
        assert!(plan.explain()[0].contains("expired"));
    }

    #[test]
    fn an_undetermined_context_is_recomputed_under_a_strict_policy_and_reused_under_an_offline_one()
    {
        let mut checked = BTreeMap::new();
        checked.insert(
            "view/mgmt".to_string(),
            ttl_context(1, 1).assess(&WorldObservation::nothing_observed()),
        );
        let strict = plan_recomputation(&chain(), &[], &checked, ReusePolicy::STRICT, None)
            .expect("plans");
        assert_eq!(strict.fan_out(), 1);
        let offline = plan_recomputation(&chain(), &[], &checked, ReusePolicy::OFFLINE, None)
            .expect("plans");
        assert!(offline.is_empty());
    }

    #[test]
    fn an_invalidation_storm_is_a_typed_refusal_carrying_the_fan_out_and_the_ceiling() {
        let result = plan_recomputation(
            &chain(),
            &["assay/idh".to_string()],
            &BTreeMap::new(),
            ReusePolicy::STRICT,
            Some(FanOutCeiling::units(2)),
        );
        assert!(matches!(
            result,
            Err(StalenessError::InvalidationFanOutExceeded {
                fan_out: 4,
                ceiling: 2,
                ..
            })
        ));
    }

    #[test]
    fn a_no_op_version_bump_is_not_a_material_change() {
        assert!(!DependencyChange::new("gencode", "v44", "v44").is_material());
        assert!(DependencyChange::new("gencode", "v44", "v45").is_material());
    }

    #[test]
    fn a_recomputation_plan_is_identical_across_repeated_calls() {
        let first = plan_recomputation(
            &chain(),
            &["assay/idh".to_string()],
            &BTreeMap::new(),
            ReusePolicy::STRICT,
            None,
        )
        .expect("plans");
        for _ in 0..8 {
            let again = plan_recomputation(
                &chain(),
                &["assay/idh".to_string()],
                &BTreeMap::new(),
                ReusePolicy::STRICT,
                None,
            )
            .expect("plans");
            assert_eq!(again, first);
        }
    }

    #[test]
    fn a_currency_verdict_survives_a_json_round_trip_with_its_variant_intact() {
        let expired = ttl_context(2, 1).assess(&WorldObservation::at(ContextEpoch(9)));
        let text = serde_json::to_string(&expired).expect("serialises");
        assert!(text.contains("expired"));
        let back: Currency = serde_json::from_str(&text).expect("deserialises");
        assert_eq!(back, expired);
    }

    #[test]
    fn an_immutable_declaration_is_a_distinct_verdict_from_a_checked_one() {
        let declaration = ValidityDeclaration::new(
            "ctx/frozen",
            ContextEpoch(0),
            CacheValidity::Immutable {
                argument: "compiled from a content-addressed release bundle".to_string(),
            },
            BiologicalValidity::AsOfDecisionEpoch {
                decision_epoch: ContextEpoch(0),
            },
        );
        let currency = declaration.assess(&WorldObservation::at(ContextEpoch(9999)));
        assert!(matches!(currency, Currency::DeclaredImmutable { .. }));
        assert!(matches!(
            ReusePolicy::STRICT.admit("ctx/frozen", &currency),
            Err(StalenessError::CurrencyUndetermined { .. })
        ));
        assert!(ReusePolicy::OFFLINE.admit("ctx/frozen", &currency).is_ok());
    }
}
