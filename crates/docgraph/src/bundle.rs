//! The context bundle compiler (41.09).
//!
//! This is `bioprism-fiber` for documents. A route names a task; this module compiles the
//! smallest set of documentation modules that is *sufficient* for it, and emits an omission
//! record saying what was left out and whether that could have mattered.
//!
//! # The two rules it exists to enforce
//!
//! **Fail rather than truncate.** If the mandatory set does not fit the declared budget, the
//! compile returns [`BundleError::MandatorySetExceedsBudget`]. It does not drop the cheapest
//! mandatory module and hand back a bundle. This is the same rule `bioprism-fiber` enforces for
//! protected closure and the reason is identical: a truncated mandatory set is indistinguishable,
//! at the point of use, from a complete one. An agent handed nine of ten required contracts has
//! no way to notice, and 41.07 says outright that "omitted required nodes invalidate the bundle".
//! A caller who wants a smaller bundle writes a smaller route.
//!
//! **Zero influence is not unknown influence.** Every module not in the bundle appears in
//! [`ContextBundle::omissions`] with a [`OmissionReason`], and the reason maps to a
//! [`InfluenceClass`] under one constraint: [`OmissionReason::NoPathFromSeeds`] is classified
//! [`InfluenceClass::Zero`] *only* when the traversal that failed to reach it was exhaustive
//! ([`Completeness::licenses_zero_influence`]). A node missed because a depth cap stopped the
//! walk, or because an edge type was filtered, is [`InfluenceClass::Unknown`] — nobody looked.
//! The two never share a representation, and the second voids the sufficiency claim.
//!
//! # Protected closure runs first
//!
//! Every module carrying a 39.05 protected class is mandatory in every bundle, whether or not any
//! path reaches it. That mirrors `bioprism-fiber`'s pass order, and for the same reason: a
//! protected class is protected against the *selection*, so making its inclusion depend on the
//! selection finding it would defeat the point. In practice this is what stops a budget-driven
//! compile from quietly dropping the consent boundary because no `requires` edge happened to
//! point at it.
//!
//! # Where 41.09 under-specifies, and what was chosen
//!
//! The blueprint names an "omission list" but not what its entries mean. Mapping this crate's
//! reasons onto `bioprism-section`'s [`InfluenceClass`] exposes a genuine gap:
//! [`OmissionReason::BudgetExcluded`] describes a module that *was* examined and *is* related to
//! the task, and was dropped anyway. That is strictly worse for sufficiency than "nobody looked",
//! but `InfluenceClass` has no member for it, so it maps to [`InfluenceClass::Unknown`] — which
//! at least never supports a sufficiency claim. The precise state survives in
//! [`BundleOmission::reason`]; only the coarse projection into the shared vocabulary loses it.
//!
//! # Not implemented
//!
//! No re-ranking of optional modules by predicted usefulness — a relevance score would be a
//! second selection policy operating after the obligation closure, and an unaudited one. Optional
//! modules enter at [`ProfileLevel::Card`] in ascending cost order, which is the policy 41.04
//! already implies: a card is what an agent gets "before they decide to load the full file", so
//! the compiler's job is to fit cards, not to guess which optional module deserves full text.
//!
//! [`InfluenceClass::Zero`]: bioprism_section::InfluenceClass::Zero
//! [`InfluenceClass::Unknown`]: bioprism_section::InfluenceClass::Unknown

use crate::error::BundleError;
use crate::registry::{DocGraph, ModuleId, ProtectedClass};
use crate::route::{RouteDefect, TaskRoute};
use crate::tokens::{ProfileLevel, TokenCost};
use crate::traverse::{traverse, Completeness, Traversal, TraversalPolicy};
use crate::vocabulary::{CompanionRequirement, DocEdgeType};
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Why a module is in the bundle. Every entry has exactly one, so "why is this here?" is answered
/// by the artifact rather than by rerunning the compiler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum InclusionReason {
    /// Named in [`TaskRoute::must_read`]. The subject of the task.
    MustRead,
    /// Named in [`TaskRoute::non_omittable`]. A boundary the route author declared.
    NonOmittableByRoute,
    /// Carries a 39.05 protected class. Mandatory regardless of the route or the walk.
    ProtectedClass { class: ProtectedClass },
    /// Pulled in by an edge whose [`CompanionRequirement`] obliged it.
    CompanionOf { of: ModuleId, via: DocEdgeType },
    /// The other side of a live `contradicts` edge.
    ContradictionCounterpart { of: ModuleId },
    /// The module a `contradicts` edge names as settling the disagreement.
    ContradictionResolution { of: ModuleId },
    /// Named in [`TaskRoute::optional`] and fitted within budget.
    DeclaredOptional,
    /// Found by the traversal and fitted within budget.
    ReachedByTraversal { depth: u32 },
}

impl InclusionReason {
    /// Whether this reason makes the entry undroppable.
    pub fn is_mandatory(&self) -> bool {
        !matches!(
            self,
            InclusionReason::DeclaredOptional | InclusionReason::ReachedByTraversal { .. }
        )
    }
}

/// One module as delivered.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleEntry {
    pub module: ModuleId,
    pub level: ProfileLevel,
    pub reason: InclusionReason,
    pub cost: TokenCost,
}

impl BundleEntry {
    pub fn is_mandatory(&self) -> bool {
        self.reason.is_mandatory()
    }
}

/// Why a module is not in the bundle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum OmissionReason {
    /// No path of any kind, in either direction, connects it to the route's seeds. Only means
    /// zero influence when the traversal that concluded it was exhaustive.
    NoPathFromSeeds,
    /// The walk stopped before reaching it. Nobody looked.
    NotExamined,
    /// 41.07's denied labels.
    DeniedByPolicyLabel { label: String },
    /// Reached, related, and dropped for budget. Examined and excluded — see the module docs on
    /// why this maps to `Unknown` rather than to a class of its own.
    BudgetExcluded { would_cost: u32 },
}

impl OmissionReason {
    /// Project onto the shared vocabulary of `bioprism-section`.
    ///
    /// `licensed_zero` is [`Completeness::licenses_zero_influence`] for the walk that produced
    /// this omission. It is a parameter and not a field because zero-influence is a property of
    /// the *search*, not of the node: the same unreached module is a proven zero under an
    /// exhaustive walk and an unknown under a depth-capped one.
    pub fn influence(&self, licensed_zero: bool) -> InfluenceClass {
        match self {
            OmissionReason::NoPathFromSeeds if licensed_zero => InfluenceClass::Zero,
            OmissionReason::NoPathFromSeeds => InfluenceClass::Unknown,
            OmissionReason::NotExamined => InfluenceClass::Unknown,
            OmissionReason::DeniedByPolicyLabel { .. } => InfluenceClass::InaccessibleByPolicy,
            OmissionReason::BudgetExcluded { .. } => InfluenceClass::Unknown,
        }
    }

    fn group_key(&self) -> &'static str {
        match self {
            OmissionReason::NoPathFromSeeds => "no_path_from_route_seeds",
            OmissionReason::NotExamined => "traversal_did_not_examine",
            OmissionReason::DeniedByPolicyLabel { .. } => "denied_by_policy_label",
            OmissionReason::BudgetExcluded { .. } => "examined_then_excluded_for_budget",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BundleOmission {
    pub module: ModuleId,
    pub reason: OmissionReason,
    pub influence: InfluenceClass,
}

/// Whether the bundle may be presented as enough to act on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "sufficiency")]
pub enum Sufficiency {
    /// Every omission is provably zero-influence or explicitly bounded.
    Sufficient,
    /// At least one omission cannot support the claim. The blocking reasons are named.
    NotSufficient { blocking: Vec<String> },
}

/// A compiled, route-specific documentation context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextBundle {
    pub route: crate::route::RouteId,
    pub entries: Vec<BundleEntry>,
    pub omissions: Vec<BundleOmission>,
    pub manifest: OmissionManifest,
    pub cost: TokenCost,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<u32>,
    pub sufficiency: Sufficiency,
    /// Completeness of the walk that justified the omission classification. Shipped so a reader
    /// can audit the zero-influence claims rather than trust them.
    pub traversal: Completeness,
    /// 39.05 classes represented in the bundle, for the reader who needs to confirm a boundary
    /// survived compilation.
    pub protected_classes: Vec<ProtectedClass>,
    /// Route defects that did not stop the compile but that a reader should know about.
    pub route_defects: Vec<RouteDefect>,
}

impl ContextBundle {
    pub fn contains(&self, module: &ModuleId) -> bool {
        self.entries.iter().any(|entry| &entry.module == module)
    }

    pub fn module_ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.entries.iter().map(|entry| &entry.module)
    }

    pub fn mandatory_ids(&self) -> impl Iterator<Item = &ModuleId> {
        self.entries
            .iter()
            .filter(|entry| entry.is_mandatory())
            .map(|entry| &entry.module)
    }

    pub fn omission_for(&self, module: &ModuleId) -> Option<&BundleOmission> {
        self.omissions
            .iter()
            .find(|omission| &omission.module == module)
    }

    pub fn is_sufficient(&self) -> bool {
        matches!(self.sufficiency, Sufficiency::Sufficient)
    }

    /// 41.09's materialised bundle: "source boundaries, module IDs, hashes, and a budget report".
    ///
    /// Boundaries are explicit delimiters rather than heading levels, because a heading is
    /// indistinguishable from the document's own headings once concatenated, and 41.09's
    /// invariant is that "normative source boundaries are explicit".
    pub fn render_markdown(&self, graph: &DocGraph) -> String {
        let mut out = String::new();
        out.push_str(&format!("# context bundle: {}\n\n", self.route));
        out.push_str(&format!(
            "estimated tokens: {} (estimator {}, not a measurement)\n",
            self.cost.tokens,
            crate::tokens::ESTIMATOR_ID
        ));
        match self.budget {
            Some(budget) => out.push_str(&format!("budget: {budget}\n")),
            None => out.push_str("budget: unbounded\n"),
        }
        out.push_str(&format!(
            "sufficiency: {}\n\n",
            match &self.sufficiency {
                Sufficiency::Sufficient => "sufficient".to_string(),
                Sufficiency::NotSufficient { blocking } =>
                    format!("not sufficient ({})", blocking.join("; ")),
            }
        ));
        for entry in &self.entries {
            let Some(node) = graph.node(&entry.module) else {
                continue;
            };
            let hash = node
                .hash
                .as_ref()
                .map(|hash| hash.as_str().to_string())
                .unwrap_or_else(|| "unhashed".to_string());
            out.push_str(&format!(
                "<!-- BEGIN {} level={} path={} sha256={} -->\n",
                entry.module,
                entry.level.as_str(),
                node.path,
                hash
            ));
            if let Some(text) = node.text_at(entry.level) {
                out.push_str(&text);
                if !text.ends_with('\n') {
                    out.push('\n');
                }
            }
            out.push_str(&format!("<!-- END {} -->\n\n", entry.module));
        }
        out.push_str("## omitted\n\n");
        for group in &self.manifest.groups {
            out.push_str(&format!(
                "- {} ({}): {} module(s)\n",
                group.reason,
                group.influence.as_str(),
                group.count
            ));
        }
        out
    }
}

/// Compile a route into a bundle.
///
/// Pass order is normative, not incidental, and mirrors `bioprism-fiber`: protected closure, then
/// route seeds, then companion closure to a fixpoint, then the budget check, then optional fill.
/// The budget check comes *after* the mandatory set is closed and *before* anything optional is
/// considered, which is what makes "fail rather than truncate" expressible at all.
pub fn compile_bundle(
    graph: &DocGraph,
    route: &TaskRoute,
    policy: &TraversalPolicy,
) -> Result<ContextBundle, BundleError> {
    let defects = route.check(graph);
    for defect in &defects {
        if let RouteDefect::MissingModule { module } = defect {
            return Err(BundleError::RouteReferencesMissingModule {
                route: route.id.clone(),
                module: module.clone(),
            });
        }
    }

    let mut mandatory: BTreeMap<ModuleId, InclusionReason> = BTreeMap::new();

    for node in graph.nodes() {
        if let Some(class) = node.protected_classes.first() {
            mandatory.insert(
                node.id.clone(),
                InclusionReason::ProtectedClass { class: *class },
            );
        }
    }
    for module in &route.must_read {
        mandatory.insert(module.clone(), InclusionReason::MustRead);
    }
    for module in &route.non_omittable {
        mandatory
            .entry(module.clone())
            .or_insert(InclusionReason::NonOmittableByRoute);
    }

    close_companions(graph, &mut mandatory)?;

    for module in mandatory.keys() {
        let Some(node) = graph.node(module) else {
            return Err(BundleError::RouteReferencesMissingModule {
                route: route.id.clone(),
                module: module.clone(),
            });
        };
        if let Some(label) = node
            .policy_labels
            .iter()
            .find(|label| policy.denied_labels.contains(*label))
        {
            return Err(BundleError::MandatoryModuleDeniedByPolicy {
                route: route.id.clone(),
                module: module.clone(),
                label: label.clone(),
            });
        }
    }

    let mut entries: Vec<BundleEntry> = Vec::new();
    for (module, reason) in &mandatory {
        let node = graph.node(module).expect("checked above");
        let level = mandatory_level(node);
        entries.push(BundleEntry {
            module: module.clone(),
            level,
            reason: reason.clone(),
            cost: node.cost_at(level),
        });
    }
    let mandatory_cost = TokenCost::sum(entries.iter().map(|entry| &entry.cost));

    if let Some(budget) = route.budget {
        if mandatory_cost.tokens > budget {
            return Err(BundleError::MandatorySetExceedsBudget {
                route: route.id.clone(),
                mandatory_cost: mandatory_cost.tokens,
                budget,
                shortfall: mandatory_cost.tokens - budget,
            });
        }
    }

    let walk = traverse(graph, &route.seeds(), policy);
    let completeness = walk.completeness();
    let licensed_zero = completeness.licenses_zero_influence();

    let mut spent = mandatory_cost.tokens;
    let mut candidates: Vec<(u32, ModuleId, InclusionReason)> = Vec::new();
    for module in &route.optional {
        if mandatory.contains_key(module) {
            continue;
        }
        if let Some(node) = graph.node(module) {
            candidates.push((
                node.cost_at(ProfileLevel::Card).tokens,
                module.clone(),
                InclusionReason::DeclaredOptional,
            ));
        }
    }
    for (module, depth) in &walk.reached {
        if mandatory.contains_key(module) || route.optional.contains(module) {
            continue;
        }
        if let Some(node) = graph.node(module) {
            candidates.push((
                node.cost_at(ProfileLevel::Card).tokens,
                module.clone(),
                InclusionReason::ReachedByTraversal { depth: *depth },
            ));
        }
    }
    candidates.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

    let mut omissions: Vec<BundleOmission> = Vec::new();
    let mut admitted: BTreeSet<ModuleId> = BTreeSet::new();
    for (cost, module, reason) in candidates {
        let fits = route.budget.is_none_or(|budget| spent + cost <= budget);
        if fits {
            spent += cost;
            admitted.insert(module.clone());
            let node = graph.node(&module).expect("candidate came from the graph");
            entries.push(BundleEntry {
                module,
                level: ProfileLevel::Card,
                reason,
                cost: node.cost_at(ProfileLevel::Card),
            });
        } else {
            let reason = OmissionReason::BudgetExcluded { would_cost: cost };
            omissions.push(BundleOmission {
                influence: reason.influence(licensed_zero),
                module,
                reason,
            });
        }
    }

    for node in graph.nodes() {
        if mandatory.contains_key(&node.id) || admitted.contains(&node.id) {
            continue;
        }
        if omissions.iter().any(|omission| omission.module == node.id) {
            continue;
        }
        let reason = if let Some(label) = walk.blocked_by_policy.get(&node.id) {
            OmissionReason::DeniedByPolicyLabel {
                label: label.clone(),
            }
        } else if licensed_zero {
            OmissionReason::NoPathFromSeeds
        } else {
            OmissionReason::NotExamined
        };
        omissions.push(BundleOmission {
            influence: reason.influence(licensed_zero),
            module: node.id.clone(),
            reason,
        });
    }

    entries.sort_by(|a, b| a.module.cmp(&b.module));
    omissions.sort_by(|a, b| a.module.cmp(&b.module));

    let manifest = build_manifest(&omissions);
    let sufficiency = if manifest.supports_sufficiency_claim() {
        Sufficiency::Sufficient
    } else {
        Sufficiency::NotSufficient {
            blocking: manifest
                .blocking_groups()
                .map(|group| format!("{} ({})", group.reason, group.influence.as_str()))
                .collect(),
        }
    };

    let mut protected_classes: BTreeSet<ProtectedClass> = BTreeSet::new();
    for entry in &entries {
        if let Some(node) = graph.node(&entry.module) {
            protected_classes.extend(node.protected_classes.iter().copied());
        }
    }

    let cost = TokenCost::sum(entries.iter().map(|entry| &entry.cost));

    Ok(ContextBundle {
        route: route.id.clone(),
        entries,
        omissions,
        manifest,
        cost,
        budget: route.budget,
        sufficiency,
        traversal: completeness,
        protected_classes: protected_classes.into_iter().collect(),
        route_defects: defects,
    })
}

/// Expand the mandatory set until no companion obligation is unmet.
///
/// The three obligations of [`CompanionRequirement`] are read off the edge type, so adding an
/// edge type to the vocabulary automatically participates here and nothing in this function
/// mentions an edge by name.
fn close_companions(
    graph: &DocGraph,
    mandatory: &mut BTreeMap<ModuleId, InclusionReason>,
) -> Result<(), BundleError> {
    loop {
        let mut additions: Vec<(ModuleId, InclusionReason)> = Vec::new();
        for module in mandatory.keys() {
            for edge in graph.edges() {
                match edge.kind.companion() {
                    CompanionRequirement::TargetWithSource if &edge.from == module => {
                        if !mandatory.contains_key(&edge.to) {
                            if !graph.contains(&edge.to) {
                                continue;
                            }
                            additions.push((
                                edge.to.clone(),
                                InclusionReason::CompanionOf {
                                    of: module.clone(),
                                    via: edge.kind,
                                },
                            ));
                        }
                    }
                    CompanionRequirement::SourceWithTarget if &edge.to == module => {
                        if !mandatory.contains_key(&edge.from) {
                            if !graph.contains(&edge.from) {
                                if edge.kind == DocEdgeType::Supersedes {
                                    return Err(BundleError::SuccessorMissing {
                                        superseded: module.clone(),
                                        successor: edge.from.clone(),
                                    });
                                }
                                continue;
                            }
                            additions.push((
                                edge.from.clone(),
                                InclusionReason::CompanionOf {
                                    of: module.clone(),
                                    via: edge.kind,
                                },
                            ));
                        }
                    }
                    CompanionRequirement::BothOrNeither => {
                        let other = if &edge.from == module {
                            Some(&edge.to)
                        } else if &edge.to == module {
                            Some(&edge.from)
                        } else {
                            None
                        };
                        if let Some(other) = other {
                            if !mandatory.contains_key(other) && graph.contains(other) {
                                additions.push((
                                    other.clone(),
                                    InclusionReason::ContradictionCounterpart {
                                        of: module.clone(),
                                    },
                                ));
                            }
                            if let Some(resolver) = &edge.resolved_by {
                                if !mandatory.contains_key(resolver) && graph.contains(resolver) {
                                    additions.push((
                                        resolver.clone(),
                                        InclusionReason::ContradictionResolution {
                                            of: module.clone(),
                                        },
                                    ));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        if additions.is_empty() {
            return Ok(());
        }
        for (module, reason) in additions {
            mandatory.entry(module).or_insert(reason);
        }
    }
}

/// The level a mandatory module is delivered at.
///
/// Contract whenever contract text exists: 41.04 says "cards never replace normative contracts",
/// so a module that a bundle depends on being *correct* about cannot be delivered as a summary.
/// [`ProfileLevel::DeepReference`] is never chosen automatically — appendices are not obligations,
/// and promoting every mandatory module to L4 would make budgets meaningless.
fn mandatory_level(node: &crate::registry::ModuleNode) -> ProfileLevel {
    if node.body.contract.is_some() {
        ProfileLevel::Contract
    } else if node.body.brief.is_some() {
        ProfileLevel::Brief
    } else {
        ProfileLevel::Card
    }
}

/// Group omissions by reason.
///
/// Grouping by reason alone is sound because the reason-to-[`InfluenceClass`] mapping is a
/// function of the *walk*, and one compile has one walk. Grouping by the pair would need
/// `InfluenceClass: Ord`, which `bioprism-section` does not provide, and inventing an ordering
/// here would put a comparison on that type that its owner did not sanction.
fn build_manifest(omissions: &[BundleOmission]) -> OmissionManifest {
    let mut grouped: BTreeMap<&'static str, (InfluenceClass, Vec<String>)> = BTreeMap::new();
    for omission in omissions {
        grouped
            .entry(omission.reason.group_key())
            .or_insert_with(|| (omission.influence, Vec::new()))
            .1
            .push(omission.module.to_string());
    }
    let mut manifest = OmissionManifest::default();
    for (reason, (influence, mut members)) in grouped {
        members.sort();
        let count = members.len();
        members.truncate(5);
        manifest.push(OmissionGroup {
            reason: reason.to_string(),
            influence,
            count,
            bound: None,
            examples: members,
        });
    }
    manifest
}

/// Which entries of an existing bundle a walk would no longer justify.
///
/// Used by [`crate::impact`] to answer "is this bundle still valid?" without recompiling.
pub fn entries_touching<'a>(
    bundle: &'a ContextBundle,
    modules: &BTreeSet<ModuleId>,
) -> Vec<&'a BundleEntry> {
    bundle
        .entries
        .iter()
        .filter(|entry| modules.contains(&entry.module))
        .collect()
}

/// The walk a bundle's omission classification rests on, recomputed.
pub fn justifying_walk(graph: &DocGraph, route: &TaskRoute, policy: &TraversalPolicy) -> Traversal {
    traverse(graph, &route.seeds(), policy)
}
