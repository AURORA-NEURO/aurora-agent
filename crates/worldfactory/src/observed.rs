//! Observed worlds from real data and workflows (27.02).
//!
//! 27.02's purpose sentence contains its whole discipline: import real datasets and real workflows
//! "without claiming inaccessible latent truth". An observed world is the only rung that touched
//! reality and the only rung with no ground truth, and those are the same fact.
//!
//! This module holds **declarations about** data: which sources, pinned to which versions, under
//! which access policy, assembled by which selection, and — 27.02's workflow step 5 — which
//! counterfactuals the study design does not identify. It holds no data. There is no cohort here,
//! no imported dataset, no workflow runner and no result. A crate that shipped a real cohort would
//! be shipping the thing 27.02's failure list calls "controlled data accidentally embedded".
//!
//! # The three checks [`declare`] runs
//!
//! * **Cohort count reconciliation** (27.02 validation). If the declared size and the strata
//!   disagree, one of the two numbers is wrong and neither can be used for anything. This is the
//!   cheapest real check in §27 and it catches the commonest import error.
//! * **Source-version pinning** (27.02 validation). An unpinned source changes underneath every
//!   result computed against it, silently and retroactively.
//! * **Selection, when the world claims to stand for a population.** A cohort assembled by an
//!   undeclared procedure is fine evidence about itself. It stops being fine the moment somebody
//!   writes down which population it represents, and that is where the refusal fires — not at
//!   import, where it would ban most real data for no gain.
//!
//! What [`declare`] deliberately does *not* check is whether the declarations are true. Nothing in
//! this crate can look at a dataset. 27.02's validation list also names "result reproduction" and
//! "data-use enforcement", both of which need an execution environment and a policy engine; those
//! belong to the runtime and the governance crates and are not simulated here.

use crate::error::ObservedRefusal;
use crate::provenance::{Provenance, Selection};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Whether an asset may be redistributed.
///
/// 27.02's failure "real data redistribution is prohibited" is a licence fact, not a technical one,
/// so it is declared rather than inferred. [`crate::authoring::freeze`] reads it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "access", rename_all = "snake_case")]
pub enum Access {
    Public,
    Controlled { policy: String },
}

impl Access {
    pub fn is_controlled(&self) -> bool {
        matches!(self, Access::Controlled { .. })
    }
}

/// A dataset or workflow the world is built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    pub name: String,
    /// The pinned version. `None` is a refusal, not a default — see [`declare`].
    pub version: Option<String>,
    pub access: Access,
    /// Whether the world bundles the asset rather than referring to it.
    #[serde(default)]
    pub embedded: bool,
}

impl SourceRef {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        SourceRef {
            name: name.into(),
            version: Some(version.into()),
            access: Access::Public,
            embedded: false,
        }
    }

    /// A source with no pinned version. Exists so the refusal can be tested and so a caller
    /// importing a moving target has to say so out loud.
    pub fn unpinned(name: impl Into<String>) -> Self {
        SourceRef {
            name: name.into(),
            version: None,
            access: Access::Public,
            embedded: false,
        }
    }

    pub fn under(mut self, access: Access) -> Self {
        self.access = access;
        self
    }

    pub fn embedded(mut self) -> Self {
        self.embedded = true;
        self
    }
}

/// A named subset of the cohort and its size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stratum {
    pub name: String,
    pub count: u64,
}

impl Stratum {
    pub fn new(name: impl Into<String>, count: u64) -> Self {
        Stratum {
            name: name.into(),
            count,
        }
    }
}

/// 27.02's required artifact "study design", reduced to the parts that are checkable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyDesign {
    pub cohort_size: u64,
    /// Sizes that must sum to `cohort_size`. An empty list skips the reconciliation, which is
    /// honest: a world that declared no strata has not contradicted itself.
    pub strata: Vec<Stratum>,
    pub selection: Selection,
    /// The population this world is offered as standing for, if any. Declaring one is what turns
    /// the selection from metadata into a constraint.
    pub stands_for_population: Option<String>,
    /// 27.02 workflow step 5. Counterfactuals the design does not identify, named so that
    /// [`crate::provenance::support`] can refuse a claim resting on one.
    pub unsupported_counterfactuals: BTreeSet<String>,
}

impl StudyDesign {
    pub fn new(cohort_size: u64, selection: Selection) -> Self {
        StudyDesign {
            cohort_size,
            strata: Vec::new(),
            selection,
            stands_for_population: None,
            unsupported_counterfactuals: BTreeSet::new(),
        }
    }

    pub fn with_stratum(mut self, stratum: Stratum) -> Self {
        self.strata.push(stratum);
        self
    }

    pub fn standing_for(mut self, population: impl Into<String>) -> Self {
        self.stands_for_population = Some(population.into());
        self
    }

    pub fn not_identifying(mut self, counterfactual: impl Into<String>) -> Self {
        self.unsupported_counterfactuals
            .insert(counterfactual.into());
        self
    }
}

/// An imported world. Constructed only by [`declare`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObservedWorld {
    id: String,
    sources: Vec<SourceRef>,
    design: StudyDesign,
    /// 27.02 required artifact "observed outcome labels" — what actually happened, which is the
    /// only thing an observed world can supply in place of ground truth.
    outcome_labels: BTreeSet<String>,
}

impl ObservedWorld {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn sources(&self) -> &[SourceRef] {
        &self.sources
    }

    pub fn design(&self) -> &StudyDesign {
        &self.design
    }

    pub fn outcome_labels(&self) -> &BTreeSet<String> {
        &self.outcome_labels
    }

    /// Sources the world may not redistribute.
    pub fn controlled_sources(&self) -> Vec<&SourceRef> {
        self.sources
            .iter()
            .filter(|s| s.access.is_controlled())
            .collect()
    }

    /// The provenance this world confers on anything built from it.
    pub fn provenance(&self) -> Provenance {
        Provenance::observed(self.design.selection.clone())
            .declaring_unsupported(self.design.unsupported_counterfactuals.iter().cloned())
    }
}

/// Declare an observed world, or say why the declaration is inconsistent.
///
/// See the module header for the three checks and for what is deliberately not checked.
pub fn declare(
    id: impl Into<String>,
    sources: Vec<SourceRef>,
    design: StudyDesign,
    outcome_labels: BTreeSet<String>,
) -> Result<ObservedWorld, ObservedRefusal> {
    for source in &sources {
        if source.version.is_none() {
            return Err(ObservedRefusal::UnpinnedSource {
                reference: source.name.clone(),
            });
        }
    }
    if !design.strata.is_empty() {
        let total: u64 = design.strata.iter().map(|s| s.count).sum();
        if total != design.cohort_size {
            return Err(ObservedRefusal::CohortCountUnreconciled {
                declared: design.cohort_size,
                strata_total: total,
            });
        }
    }
    if design.stands_for_population.is_some() && matches!(design.selection, Selection::Undeclared) {
        return Err(ObservedRefusal::UndeclaredSelection);
    }
    Ok(ObservedWorld {
        id: id.into(),
        sources,
        design,
        outcome_labels,
    })
}
