//! World State IR — what a Decision Cell freezes.
//!
//! Implements blueprint 03.03 (World State IR). The blueprint's detailed design has five parts and
//! all five are predicates over an artifact rather than descriptions of what people do, which is
//! why this module exists as code: state partitions, the snapshot model, visibility, external
//! state, and equality.
//!
//! # The three things worth enforcing
//!
//! **Oracle state cannot leak into the agent view.** 03.03: "Benchmark hidden state is never
//! injected into the agent view. The IR records visibility class so compilers and viewers cannot
//! accidentally leak oracle information." Here [`AgentView`] has a private component map and
//! exactly one constructor, [`WorldStateManifest::agent_view`], which filters
//! [`Visibility::OracleOnly`] out. There is no `AgentView::insert`, so a compiler cannot add a
//! component after the filter ran.
//!
//! **Equality is per component, and "not compared" is a third answer.** 03.03: "byte equality for
//! files, logical equality for databases, schema/value equality for tools, and declared equivalence
//! for nondeterministic services". The first three are decidable from the manifest. The fourth is
//! not — a declared equivalence is a claim someone made about a specific pair of captures, and if
//! no such claim covers this pair then the honest answer is neither *equal* nor *differs*. So
//! [`StateComparison::verdict`] is three-valued and a single [`ComponentVerdict::Unchecked`] makes
//! the whole comparison [`StateVerdict::Indeterminate`]. This is the same rule `bioprism-fiber`
//! applies to unknown-influence groups, in state currency.
//!
//! **Restoration confidence is declared per component and meets upward.** See [`crate::fidelity`].
//! A manifest's fidelity is the meet over its components, so one uncaptured component is visible in
//! the manifest's single headline number instead of being averaged away.
//!
//! # What is not implemented
//!
//! No actual capture and no actual restore: this is the IR, not the agent that fills it. The
//! digests are supplied by the caller. Delta application is structural (a delta names components it
//! replaces, adds or removes) and does not diff bytes — byte-level delta encoding is a storage
//! concern and 03.03 is explicit that a state is "a manifest of content-addressed components".
//!
//! `bioprism-runtime` owns 05.04's WorldTape, which is the *effect* ledger of a run. That and this
//! are different records: the tape says what the world answered, the manifest says what the world
//! was. A run has both and neither reconstructs the other.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};

use crate::error::SweepError;
use crate::fidelity::{meet_all, Declaration};

/// The state partitions 03.03 enumerates.
///
/// The list is the blueprint's, in its order: "Task state; environment state; agent internal
/// state; visible context; accessible-but-unread resources; tool surface; permission and policy
/// state; budget; external service state; oracle-only hidden state."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Partition {
    Task,
    Environment,
    AgentInternal,
    VisibleContext,
    AccessibleUnread,
    ToolSurface,
    PermissionPolicy,
    Budget,
    ExternalService,
    OracleHidden,
}

impl Partition {
    /// All ten, for exhaustiveness checks by callers that must account for every partition.
    pub const ALL: [Partition; 10] = [
        Partition::Task,
        Partition::Environment,
        Partition::AgentInternal,
        Partition::VisibleContext,
        Partition::AccessibleUnread,
        Partition::ToolSurface,
        Partition::PermissionPolicy,
        Partition::Budget,
        Partition::ExternalService,
        Partition::OracleHidden,
    ];

    /// The visibility class this partition forces, where it forces one.
    ///
    /// Two partitions are named after their visibility and so cannot be given another:
    /// `OracleHidden` is oracle-only by definition, and `AccessibleUnread` is 03.03's
    /// "accessible-but-unread resources" — present to the agent but not in its context. The rest
    /// carry an explicit class because a tool surface or a budget may or may not be shown.
    pub fn forced_visibility(self) -> Option<Visibility> {
        match self {
            Partition::OracleHidden => Some(Visibility::OracleOnly),
            Partition::AccessibleUnread => Some(Visibility::AccessibleUnread),
            _ => None,
        }
    }
}

/// Who may see a component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    /// In the agent's context.
    AgentVisible,
    /// Reachable by the agent but not in its context: 03.03's accessible-but-unread resources.
    /// It is in the agent view, because the agent could have read it.
    AccessibleUnread,
    /// Benchmark hidden state. Never in the agent view under any policy.
    OracleOnly,
}

/// How the component's value was obtained.
///
/// The external-state modes are 03.03's: "Live services are represented by a fixture, emulator,
/// recorded response tape, or declared live dependency."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureMethod {
    /// Recorded as metadata only; the bytes are named but not held.
    Manifest,
    /// Full content captured.
    Snapshot,
    /// Captured as a change against a base.
    Delta,
    /// A stand-in authored for the benchmark.
    Fixture,
    /// A local implementation of the service's contract.
    Emulator,
    /// Recorded responses replayed in order.
    ResponseTape,
    /// The live service, declared as such.
    LiveDependency,
}

impl CaptureMethod {
    /// Whether this method leaves the run dependent on something outside the manifest.
    ///
    /// Only [`CaptureMethod::LiveDependency`] does. The point of enumerating the other three
    /// external modes separately is that a fixture, an emulator and a tape are all *in* the
    /// manifest and a live dependency is not, and a reproducibility claim must distinguish them.
    pub fn escapes_the_manifest(self) -> bool {
        matches!(self, CaptureMethod::LiveDependency)
    }
}

/// How two captures of this component are compared.
///
/// 03.03: "State equality is component-specific: byte equality for files, logical equality for
/// databases, schema/value equality for tools, and declared equivalence for nondeterministic
/// services."
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EqualityKind {
    /// Digest equality decides it.
    Bytes,
    /// Digest equality decides it, over a normalised logical form the caller supplied as the digest.
    Logical,
    /// Digest equality decides it, over a schema-and-value form the caller supplied as the digest.
    SchemaValue,
    /// Digest equality does *not* decide it. Only a registered equivalence basis can.
    DeclaredEquivalence,
}

/// One content-addressed piece of a world state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Component {
    id: String,
    partition: Partition,
    visibility: Visibility,
    capture: CaptureMethod,
    digest: ContentHash,
    equality: EqualityKind,
    restoration: Declaration,
}

impl Component {
    /// Build a component, checking the two structural rules 03.03 implies.
    ///
    /// A component in the oracle-hidden partition must be oracle-only, and — the direction that
    /// actually matters — a component *outside* that partition must not be, because a viewer that
    /// obeys the visibility class would then hide task state while a viewer that obeys the
    /// partition would show hidden state. Two encodings of the same fact that can disagree are
    /// worse than one.
    pub fn new(
        id: impl Into<String>,
        partition: Partition,
        visibility: Visibility,
        capture: CaptureMethod,
        digest: ContentHash,
        equality: EqualityKind,
        restoration: Declaration,
    ) -> Result<Self, SweepError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(SweepError::empty("Component", "id"));
        }
        if let Some(forced) = partition.forced_visibility() {
            if visibility != forced {
                return Err(SweepError::malformed(
                    "Component",
                    format!(
                        "partition {partition:?} forces visibility {forced:?}, got {visibility:?}"
                    ),
                ));
            }
        } else if visibility == Visibility::OracleOnly {
            return Err(SweepError::malformed(
                "Component",
                format!("oracle-only visibility belongs to the OracleHidden partition, not {partition:?}"),
            ));
        }
        Ok(Component {
            id,
            partition,
            visibility,
            capture,
            digest,
            equality,
            restoration,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn partition(&self) -> Partition {
        self.partition
    }

    pub fn visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn capture(&self) -> CaptureMethod {
        self.capture
    }

    pub fn digest(&self) -> &ContentHash {
        &self.digest
    }

    pub fn equality(&self) -> EqualityKind {
        self.equality
    }

    pub fn restoration(&self) -> &Declaration {
        &self.restoration
    }
}

/// A structural change to a manifest: 03.03's "base plus ordered deltas".
///
/// Deltas name components rather than byte ranges. A delta that removes a component must say so
/// explicitly; there is no "absent from the delta means unchanged *or* removed" ambiguity, because
/// that ambiguity is exactly how a state reconstruction silently drops evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    /// Components added or replaced, keyed by id.
    pub upsert: Vec<Component>,
    /// Component ids removed.
    pub remove: BTreeSet<String>,
}

impl Delta {
    pub fn new() -> Self {
        Delta {
            upsert: Vec::new(),
            remove: BTreeSet::new(),
        }
    }

    pub fn upserting(mut self, component: Component) -> Self {
        self.upsert.push(component);
        self
    }

    pub fn removing(mut self, id: impl Into<String>) -> Self {
        self.remove.insert(id.into());
        self
    }
}

impl Default for Delta {
    fn default() -> Self {
        Delta::new()
    }
}

/// A world state: base components plus ordered deltas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldStateManifest {
    state_id: String,
    base: BTreeMap<String, Component>,
    deltas: Vec<Delta>,
}

impl WorldStateManifest {
    /// A manifest needs at least one component.
    ///
    /// The empty manifest is refused rather than allowed, because [`crate::fidelity::meet_all`]
    /// over nothing is `Exact` and an empty state that reports perfect restoration is the most
    /// flattering possible lie about a capture that did not happen.
    pub fn new(
        state_id: impl Into<String>,
        components: impl IntoIterator<Item = Component>,
    ) -> Result<Self, SweepError> {
        let state_id = state_id.into();
        if state_id.trim().is_empty() {
            return Err(SweepError::empty("WorldStateManifest", "state_id"));
        }
        let mut base = BTreeMap::new();
        for component in components {
            if base
                .insert(component.id.clone(), component.clone())
                .is_some()
            {
                return Err(SweepError::malformed(
                    "WorldStateManifest",
                    format!("duplicate component id {}", component.id),
                ));
            }
        }
        if base.is_empty() {
            return Err(SweepError::malformed(
                "WorldStateManifest",
                "a manifest with no components would report Exact restoration",
            ));
        }
        Ok(WorldStateManifest {
            state_id,
            base,
            deltas: Vec::new(),
        })
    }

    pub fn with_delta(mut self, delta: Delta) -> Self {
        self.deltas.push(delta);
        self
    }

    pub fn state_id(&self) -> &str {
        &self.state_id
    }

    pub fn delta_count(&self) -> usize {
        self.deltas.len()
    }

    /// Apply the deltas in order and return the effective component set.
    ///
    /// Fails if a delta removes a component that is not present, because a delta chain that no
    /// longer describes its base has been reordered or truncated and silently tolerating that
    /// produces a state nobody can reason about.
    pub fn resolve(&self) -> Result<BTreeMap<String, Component>, SweepError> {
        let mut current = self.base.clone();
        for (index, delta) in self.deltas.iter().enumerate() {
            for id in &delta.remove {
                if current.remove(id).is_none() {
                    return Err(SweepError::malformed(
                        "WorldStateManifest::resolve",
                        format!("delta {index} removes absent component {id}"),
                    ));
                }
            }
            for component in &delta.upsert {
                current.insert(component.id.clone(), component.clone());
            }
        }
        if current.is_empty() {
            return Err(SweepError::malformed(
                "WorldStateManifest::resolve",
                "the delta chain removed every component",
            ));
        }
        Ok(current)
    }

    /// The manifest's restoration declaration: the meet over its resolved components.
    pub fn restoration(&self) -> Result<Declaration, SweepError> {
        let resolved = self.resolve()?;
        Ok(meet_all(resolved.values().map(Component::restoration)))
    }

    /// Whether any resolved component depends on something outside the manifest.
    pub fn has_live_dependency(&self) -> Result<bool, SweepError> {
        Ok(self
            .resolve()?
            .values()
            .any(|c| c.capture.escapes_the_manifest()))
    }

    /// The agent's view of this state.
    ///
    /// The only constructor of [`AgentView`], and it filters [`Visibility::OracleOnly`].
    pub fn agent_view(&self) -> Result<AgentView, SweepError> {
        let resolved = self.resolve()?;
        let visible = resolved
            .into_iter()
            .filter(|(_, c)| c.visibility != Visibility::OracleOnly)
            .collect();
        Ok(AgentView {
            state_id: self.state_id.clone(),
            visible,
        })
    }
}

/// What the agent may see of a world state.
///
/// Private map, no insertion API, and one constructor. 03.03 asks that hidden benchmark state
/// "is never injected into the agent view"; making the view unconstructable except by the filter is
/// how that becomes structural rather than a review item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentView {
    state_id: String,
    visible: BTreeMap<String, Component>,
}

impl AgentView {
    pub fn state_id(&self) -> &str {
        &self.state_id
    }

    pub fn contains(&self, id: &str) -> bool {
        self.visible.contains_key(id)
    }

    pub fn len(&self) -> usize {
        self.visible.len()
    }

    pub fn is_empty(&self) -> bool {
        self.visible.is_empty()
    }

    pub fn components(&self) -> impl Iterator<Item = &Component> {
        self.visible.values()
    }

    /// The components the agent could have read but had not.
    ///
    /// 10.14's decision lens asks for exactly this set, and it is a property of the state rather
    /// than of the viewer, so it lives here.
    pub fn accessible_unread(&self) -> impl Iterator<Item = &Component> {
        self.visible
            .values()
            .filter(|c| c.visibility == Visibility::AccessibleUnread)
    }
}

/// A claim that two specific captures of a nondeterministic component are interchangeable.
///
/// 03.03's fourth equality rule. The basis is required and names the pair, so the claim cannot be
/// reused for a different pair of digests by accident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceClaim {
    pub component: String,
    pub left: ContentHash,
    pub right: ContentHash,
    pub basis: String,
}

impl EquivalenceClaim {
    pub fn new(
        component: impl Into<String>,
        left: ContentHash,
        right: ContentHash,
        basis: impl Into<String>,
    ) -> Result<Self, SweepError> {
        let basis = basis.into();
        crate::error::require_nonempty(&basis, "EquivalenceClaim", "basis")?;
        Ok(EquivalenceClaim {
            component: component.into(),
            left,
            right,
            basis,
        })
    }

    fn covers(&self, component: &str, a: &ContentHash, b: &ContentHash) -> bool {
        self.component == component
            && ((&self.left == a && &self.right == b) || (&self.left == b && &self.right == a))
    }
}

/// What comparing one component of two states established.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "verdict")]
pub enum ComponentVerdict {
    /// Digests agree under a decidable equality kind.
    Equal,
    /// A registered [`EquivalenceClaim`] covers this pair.
    EquivalentByClaim { basis: String },
    /// Digests disagree under a decidable equality kind.
    Differs,
    /// Present on one side only.
    OneSided { side: Side },
    /// Not decidable from the manifests: the equality kind is `DeclaredEquivalence` and no claim
    /// covers this pair. Distinct from [`ComponentVerdict::Differs`], and the distinction is the
    /// point of the module.
    Unchecked { reason: String },
}

/// Which manifest a one-sided component came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Left,
    Right,
}

/// The three-valued result of comparing two world states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StateVerdict {
    /// Every component was compared and every comparison said equal.
    Equal,
    /// At least one component was compared and differs.
    Differs,
    /// No component differs, but at least one could not be compared. Not equal.
    Indeterminate,
}

/// A per-component comparison of two manifests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateComparison {
    pub components: BTreeMap<String, ComponentVerdict>,
}

impl StateComparison {
    /// The three-valued roll-up.
    ///
    /// `Differs` dominates `Indeterminate` because a known difference is more informative than an
    /// unknown one; `Indeterminate` dominates `Equal` because an unchecked component means the
    /// states were not shown to be the same. There is no path from `Unchecked` to `Equal`.
    pub fn verdict(&self) -> StateVerdict {
        let mut indeterminate = false;
        for verdict in self.components.values() {
            match verdict {
                ComponentVerdict::Differs | ComponentVerdict::OneSided { .. } => {
                    return StateVerdict::Differs
                }
                ComponentVerdict::Unchecked { .. } => indeterminate = true,
                ComponentVerdict::Equal | ComponentVerdict::EquivalentByClaim { .. } => {}
            }
        }
        if indeterminate {
            StateVerdict::Indeterminate
        } else {
            StateVerdict::Equal
        }
    }

    /// The ids that could not be compared. Empty exactly when the verdict is not
    /// [`StateVerdict::Indeterminate`] for want of a claim.
    pub fn unchecked(&self) -> Vec<&str> {
        self.components
            .iter()
            .filter(|(_, v)| matches!(v, ComponentVerdict::Unchecked { .. }))
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

/// Compare two world states component by component.
///
/// `claims` supplies the declared equivalences; components whose equality kind is
/// `DeclaredEquivalence` and which no claim covers come back `Unchecked`.
pub fn compare(
    left: &WorldStateManifest,
    right: &WorldStateManifest,
    claims: &[EquivalenceClaim],
) -> Result<StateComparison, SweepError> {
    let l = left.resolve()?;
    let r = right.resolve()?;
    let mut components = BTreeMap::new();
    let ids: BTreeSet<&String> = l.keys().chain(r.keys()).collect();
    for id in ids {
        let verdict = match (l.get(id), r.get(id)) {
            (Some(_), None) => ComponentVerdict::OneSided { side: Side::Left },
            (None, Some(_)) => ComponentVerdict::OneSided { side: Side::Right },
            (Some(a), Some(b)) => compare_pair(id, a, b, claims),
            (None, None) => {
                return Err(SweepError::malformed(
                    "state comparison",
                    format!("component `{id}` disappeared while comparing the resolved states"),
                ));
            }
        };
        components.insert(id.clone(), verdict);
    }
    Ok(StateComparison { components })
}

fn compare_pair(
    id: &str,
    a: &Component,
    b: &Component,
    claims: &[EquivalenceClaim],
) -> ComponentVerdict {
    if a.equality != b.equality {
        return ComponentVerdict::Unchecked {
            reason: format!(
                "equality kind differs: {:?} on the left, {:?} on the right",
                a.equality, b.equality
            ),
        };
    }
    match a.equality {
        EqualityKind::Bytes | EqualityKind::Logical | EqualityKind::SchemaValue => {
            if a.digest == b.digest {
                ComponentVerdict::Equal
            } else {
                ComponentVerdict::Differs
            }
        }
        EqualityKind::DeclaredEquivalence => {
            if a.digest == b.digest {
                ComponentVerdict::Equal
            } else if let Some(claim) = claims.iter().find(|c| c.covers(id, &a.digest, &b.digest)) {
                ComponentVerdict::EquivalentByClaim {
                    basis: claim.basis.clone(),
                }
            } else {
                ComponentVerdict::Unchecked {
                    reason: "nondeterministic component with no equivalence claim for this pair"
                        .to_string(),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn component(id: &str, partition: Partition, visibility: Visibility) -> Component {
        Component::new(
            id,
            partition,
            visibility,
            CaptureMethod::Snapshot,
            hash(id),
            EqualityKind::Bytes,
            Declaration::exact(),
        )
        .unwrap()
    }

    fn manifest(id: &str, components: Vec<Component>) -> WorldStateManifest {
        WorldStateManifest::new(id, components).unwrap()
    }

    #[test]
    fn the_partition_list_is_the_blueprints_ten() {
        assert_eq!(Partition::ALL.len(), 10);
        let unique: BTreeSet<_> = Partition::ALL.iter().collect();
        assert_eq!(unique.len(), 10);
    }

    #[test]
    fn oracle_only_visibility_outside_the_hidden_partition_is_refused() {
        let err = Component::new(
            "leak",
            Partition::Task,
            Visibility::OracleOnly,
            CaptureMethod::Snapshot,
            hash("leak"),
            EqualityKind::Bytes,
            Declaration::exact(),
        )
        .unwrap_err();
        assert!(matches!(err, SweepError::Malformed { .. }));
    }

    #[test]
    fn a_hidden_partition_component_cannot_be_declared_agent_visible() {
        assert!(Component::new(
            "answer",
            Partition::OracleHidden,
            Visibility::AgentVisible,
            CaptureMethod::Snapshot,
            hash("answer"),
            EqualityKind::Bytes,
            Declaration::exact(),
        )
        .is_err());
    }

    #[test]
    fn the_agent_view_never_contains_oracle_state() {
        let m = manifest(
            "s1",
            vec![
                component("task", Partition::Task, Visibility::AgentVisible),
                component("answer", Partition::OracleHidden, Visibility::OracleOnly),
            ],
        );
        let view = m.agent_view().unwrap();
        assert!(view.contains("task"));
        assert!(!view.contains("answer"));
        assert_eq!(view.len(), 1);
    }

    #[test]
    fn accessible_but_unread_resources_stay_in_the_agent_view() {
        let m = manifest(
            "s1",
            vec![
                component("ctx", Partition::VisibleContext, Visibility::AgentVisible),
                component(
                    "manual",
                    Partition::AccessibleUnread,
                    Visibility::AccessibleUnread,
                ),
            ],
        );
        let view = m.agent_view().unwrap();
        assert_eq!(view.accessible_unread().count(), 1);
        assert_eq!(view.len(), 2);
    }

    #[test]
    fn a_manifest_with_no_components_is_refused_because_it_would_report_exact() {
        assert!(WorldStateManifest::new("empty", Vec::new()).is_err());
    }

    #[test]
    fn manifest_restoration_is_the_worst_component_not_the_average() {
        let weak = Component::new(
            "memory",
            Partition::AgentInternal,
            Visibility::AgentVisible,
            CaptureMethod::Manifest,
            hash("memory"),
            EqualityKind::Bytes,
            Declaration::degraded("process memory capture is opt-in and was off").unwrap(),
        )
        .unwrap();
        let m = manifest(
            "s1",
            vec![
                component("task", Partition::Task, Visibility::AgentVisible),
                weak,
            ],
        );
        assert_eq!(
            m.restoration().unwrap().level(),
            crate::fidelity::Level::Degraded
        );
    }

    #[test]
    fn a_delta_that_removes_an_absent_component_fails_rather_than_being_ignored() {
        let m = manifest(
            "s1",
            vec![component("task", Partition::Task, Visibility::AgentVisible)],
        )
        .with_delta(Delta::new().removing("never-existed"));
        assert!(m.resolve().is_err());
    }

    #[test]
    fn deltas_apply_in_order_and_the_last_writer_wins() {
        let first = Component::new(
            "task",
            Partition::Task,
            Visibility::AgentVisible,
            CaptureMethod::Snapshot,
            hash("v2"),
            EqualityKind::Bytes,
            Declaration::exact(),
        )
        .unwrap();
        let second = Component::new(
            "task",
            Partition::Task,
            Visibility::AgentVisible,
            CaptureMethod::Snapshot,
            hash("v3"),
            EqualityKind::Bytes,
            Declaration::exact(),
        )
        .unwrap();
        let m = manifest(
            "s1",
            vec![component("task", Partition::Task, Visibility::AgentVisible)],
        )
        .with_delta(Delta::new().upserting(first))
        .with_delta(Delta::new().upserting(second));
        assert_eq!(m.resolve().unwrap()["task"].digest(), &hash("v3"));
        assert_eq!(m.delta_count(), 2);
    }

    #[test]
    fn a_live_dependency_is_visible_as_escaping_the_manifest() {
        let live = Component::new(
            "ncbi",
            Partition::ExternalService,
            Visibility::AgentVisible,
            CaptureMethod::LiveDependency,
            hash("ncbi"),
            EqualityKind::DeclaredEquivalence,
            Declaration::degraded("live service, not captured").unwrap(),
        )
        .unwrap();
        let m = manifest("s1", vec![live]);
        assert!(m.has_live_dependency().unwrap());
        let taped = manifest(
            "s2",
            vec![component("task", Partition::Task, Visibility::AgentVisible)],
        );
        assert!(!taped.has_live_dependency().unwrap());
    }

    #[test]
    fn equal_digests_under_a_decidable_kind_compare_equal() {
        let a = manifest(
            "a",
            vec![component("f", Partition::Task, Visibility::AgentVisible)],
        );
        let b = manifest(
            "b",
            vec![component("f", Partition::Task, Visibility::AgentVisible)],
        );
        assert_eq!(compare(&a, &b, &[]).unwrap().verdict(), StateVerdict::Equal);
    }

    #[test]
    fn a_nondeterministic_component_with_no_claim_is_indeterminate_not_differing() {
        let left = Component::new(
            "svc",
            Partition::ExternalService,
            Visibility::AgentVisible,
            CaptureMethod::ResponseTape,
            hash("resp-1"),
            EqualityKind::DeclaredEquivalence,
            Declaration::equivalent("tape").unwrap(),
        )
        .unwrap();
        let right = Component::new(
            "svc",
            Partition::ExternalService,
            Visibility::AgentVisible,
            CaptureMethod::ResponseTape,
            hash("resp-2"),
            EqualityKind::DeclaredEquivalence,
            Declaration::equivalent("tape").unwrap(),
        )
        .unwrap();
        let comparison =
            compare(&manifest("a", vec![left]), &manifest("b", vec![right]), &[]).unwrap();
        assert_eq!(comparison.verdict(), StateVerdict::Indeterminate);
        assert_eq!(comparison.unchecked(), vec!["svc"]);
    }

    #[test]
    fn a_covering_equivalence_claim_turns_indeterminate_into_equivalent() {
        let left = Component::new(
            "svc",
            Partition::ExternalService,
            Visibility::AgentVisible,
            CaptureMethod::ResponseTape,
            hash("resp-1"),
            EqualityKind::DeclaredEquivalence,
            Declaration::equivalent("tape").unwrap(),
        )
        .unwrap();
        let right = Component::new(
            "svc",
            Partition::ExternalService,
            Visibility::AgentVisible,
            CaptureMethod::ResponseTape,
            hash("resp-2"),
            EqualityKind::DeclaredEquivalence,
            Declaration::equivalent("tape").unwrap(),
        )
        .unwrap();
        let claim = EquivalenceClaim::new(
            "svc",
            hash("resp-1"),
            hash("resp-2"),
            "identical bodies; only the request id differs",
        )
        .unwrap();
        let comparison = compare(
            &manifest("a", vec![left]),
            &manifest("b", vec![right]),
            &[claim],
        )
        .unwrap();
        assert_eq!(comparison.verdict(), StateVerdict::Equal);
    }

    #[test]
    fn an_equivalence_claim_does_not_cover_a_different_pair_of_digests() {
        let claim = EquivalenceClaim::new("svc", hash("a"), hash("b"), "same bodies").unwrap();
        assert!(claim.covers("svc", &hash("b"), &hash("a")));
        assert!(!claim.covers("svc", &hash("a"), &hash("c")));
        assert!(!claim.covers("other", &hash("a"), &hash("b")));
    }

    #[test]
    fn a_one_sided_component_makes_the_states_differ() {
        let a = manifest(
            "a",
            vec![
                component("f", Partition::Task, Visibility::AgentVisible),
                component("g", Partition::Task, Visibility::AgentVisible),
            ],
        );
        let b = manifest(
            "b",
            vec![component("f", Partition::Task, Visibility::AgentVisible)],
        );
        let comparison = compare(&a, &b, &[]).unwrap();
        assert_eq!(comparison.verdict(), StateVerdict::Differs);
        assert_eq!(
            comparison.components["g"],
            ComponentVerdict::OneSided { side: Side::Left }
        );
    }

    #[test]
    fn a_known_difference_dominates_an_unknown_one() {
        let mut components = BTreeMap::new();
        components.insert("a".to_string(), ComponentVerdict::Differs);
        components.insert(
            "b".to_string(),
            ComponentVerdict::Unchecked {
                reason: "no claim".into(),
            },
        );
        assert_eq!(
            StateComparison { components }.verdict(),
            StateVerdict::Differs
        );
    }

    #[test]
    fn duplicate_component_ids_are_refused_at_construction() {
        let dup = vec![
            component("f", Partition::Task, Visibility::AgentVisible),
            component("f", Partition::Task, Visibility::AgentVisible),
        ];
        assert!(WorldStateManifest::new("s", dup).is_err());
    }
}
