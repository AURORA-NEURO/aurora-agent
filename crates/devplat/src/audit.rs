//! What this crate found, as a value, and the two invariants it holds across its siblings.
//!
//! Everything here is derived. There is no stored state, no clock and no filesystem access except
//! where a caller passes in a workspace, so the same source tree always produces the same
//! [`DevPlatReport`] and the same digest.
//!
//! # Findings reuse `bioprism-devx`, and stop short of its code space
//!
//! A [`Finding`] carries a devx [`Site`], a [`Certainty`] and a [`Remedy`], because those three
//! are the parts of a diagnostic that survive being read by someone who does not know this crate,
//! and defining a second `Site` enum would produce two spellings of "the record does not say".
//! What a finding does *not* carry is a `DiagnosticCode`: devx validates codes against its own
//! `DEVX-` namespace and its catalogue is the registry for them, so minting codes here would
//! squat in another crate's key space and break the join it maintains between a catalogue entry,
//! a lint finding and an exit-code audit row.
//!
//! # The two cross-catalogue invariants
//!
//! [`recipes_are_all_in_tree`] states that every crate a `bioprism-cookbook` recipe names is a
//! crate of this workspace — which is why the recipe type never needed a foreign surface, and why
//! this crate needed one. [`catalogues_are_disjoint`] states that no walkthrough here shares an
//! identifier with a recipe there. Together they are the executable form of "do not build a second
//! registry": the two catalogues are checked to be about different things rather than asserted to
//! be.
//!
//! [`Site`]: bioprism_devx::Site
//! [`Certainty`]: bioprism_devx::Certainty
//! [`Remedy`]: bioprism_devx::Remedy

use bioprism_cookbook::{Cookbook, Workspace};
use bioprism_devx::{Certainty, ChangeRequired, Remedy, Site};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::claim::Evidence;
use crate::classify::{classification, implemented_module_ids, not_implemented, verdict_counts};
use crate::error::ReportError;
use crate::surface::foreign_subjects;
use crate::walkthrough::{recheck, Standing, Walkthrough};

/// Something worth telling a developer, with what would have to change.
///
/// The remedy is not optional. `bioprism-devx` argues the rule and lints its own catalogue for it;
/// this crate adopts it rather than restating it, and the type makes the field mandatory so there
/// is nothing to lint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Finding {
    /// The rule, named as a rule.
    pub invariant: String,
    /// What was seen.
    pub observed: String,
    pub site: Site,
    pub certainty: Certainty,
    pub remedy: Remedy,
}

/// One walkthrough, flattened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalkthroughSummary {
    pub id: String,
    pub goal: String,
    pub subject: String,
    pub standing: Standing,
    pub steps: usize,
    pub narration_permille: u32,
}

/// The crate's own account of itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DevPlatReport {
    /// process, foreign artifact, covered elsewhere, implemented here.
    pub verdict_counts: [usize; 4],
    /// The module ids this crate is entitled to cite.
    pub implemented: Vec<String>,
    /// Everything else, by title and bucket. Never by id.
    pub not_implemented: Vec<(String, String)>,
    /// The subjects whose artifact is in another language and another repository.
    pub foreign_subjects: Vec<String>,
    pub walkthroughs: Vec<WalkthroughSummary>,
    /// Claims across all walkthroughs that no test here will ever guard.
    pub unguarded_claims: usize,
    /// Claims a test here does guard.
    pub guarded_claims: usize,
    pub digest: String,
}

impl DevPlatReport {
    /// Assemble the report from the classification and a set of walkthroughs.
    pub fn of(walkthroughs: &[Walkthrough]) -> Result<Self, ReportError> {
        let summaries: Vec<WalkthroughSummary> = walkthroughs
            .iter()
            .map(|walkthrough| WalkthroughSummary {
                id: walkthrough.id().as_str().to_string(),
                goal: walkthrough.goal().to_string(),
                subject: walkthrough.subject().describe(),
                standing: walkthrough.standing(),
                steps: walkthrough.steps().len(),
                narration_permille: walkthrough.narration_permille(),
            })
            .collect();
        let guarded = summaries
            .iter()
            .map(|summary| summary.standing.guarded_claims())
            .sum();
        let unguarded = summaries
            .iter()
            .map(|summary| summary.standing.unguarded_claims())
            .sum();
        let mut report = DevPlatReport {
            verdict_counts: verdict_counts(),
            implemented: implemented_module_ids()
                .into_iter()
                .map(str::to_string)
                .collect(),
            not_implemented: not_implemented()
                .into_iter()
                .map(|(title, bucket, _)| (title.to_string(), bucket.to_string()))
                .collect(),
            foreign_subjects: foreign_subjects()
                .into_iter()
                .map(|subject| format!("{}: {}", subject.title, subject.surface.describe()))
                .collect(),
            walkthroughs: summaries,
            guarded_claims: guarded,
            unguarded_claims: unguarded,
            digest: String::new(),
        };
        let value = json!({
            "verdict_counts": report.verdict_counts,
            "implemented": report.implemented,
            "not_implemented": report.not_implemented,
            "foreign_subjects": report.foreign_subjects,
            "walkthroughs": report.walkthroughs,
            "guarded_claims": report.guarded_claims,
            "unguarded_claims": report.unguarded_claims,
        });
        report.digest = ContentHash::of_value(&value)
            .map_err(|error| ReportError::NotCanonical {
                reason: error.to_string(),
            })?
            .as_str()
            .to_string();
        Ok(report)
    }

    /// Whether the classification accounts for every module in scope.
    pub fn modules_classified(&self) -> usize {
        self.verdict_counts.iter().sum()
    }
}

/// Findings a reader should act on, derived from the walkthroughs and a working tree.
///
/// Two kinds. A refuted claim is a document that has gone stale against the code and is fixable
/// today. An entirely-outside document is not a defect, and it produces a finding anyway, because
/// a green test run over a catalogue containing one says less than it appears to.
pub fn findings(walkthroughs: &[Walkthrough], workspace: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for walkthrough in walkthroughs {
        let id = walkthrough.id().as_str().to_string();
        for (api, evidence) in recheck(walkthrough, workspace) {
            if let Evidence::AbsentFromTree = evidence {
                findings.push(Finding {
                    invariant: "a walkthrough step that names an in-tree API resolves against the \
                                working tree"
                        .to_string(),
                    observed: format!("`{api}` is named by walkthrough `{id}` and is not present"),
                    site: Site::Source {
                        document: format!("walkthrough:{id}"),
                        span: None,
                    },
                    certainty: Certainty::Observed,
                    remedy: Remedy::new(
                        "restore the symbol or rewrite the step to name what replaced it",
                        Site::Source {
                            document: format!("walkthrough:{id}"),
                            span: None,
                        },
                        "the step resolves again against the file it names",
                        ChangeRequired::Payload,
                        Certainty::Observed,
                    ),
                });
            }
        }
        if walkthrough.documents_absent_artifact() {
            findings.push(Finding {
                invariant:
                    "a document whose every claim is outside this repository is labelled as \
                            such rather than counted as verified"
                        .to_string(),
                observed: format!(
                    "walkthrough `{id}` has {} claims and none of them can be checked here",
                    walkthrough.standing().unguarded_claims()
                ),
                site: Site::Unlocated {
                    because: "the artifact the document is about has no file in this checkout"
                        .to_string(),
                },
                certainty: Certainty::Observed,
                remedy: Remedy::new(
                    "publish the document with the foreign artifact it documents, and keep the \
                     standing visible wherever it is listed",
                    Site::Unlocated {
                        because: "the fix is in the repository that owns the artifact".to_string(),
                    },
                    "a reader of the catalogue can tell this document apart from one a test guards",
                    ChangeRequired::Environment,
                    Certainty::Observed,
                ),
            });
        }
    }
    findings
}

/// Every crate a cookbook recipe names is a crate of this workspace.
///
/// Returns the offending names, empty when it holds. The point is not to police the cookbook —
/// it holds trivially, and it holds because the recipe type can only express in-tree references.
/// The point is that this crate's [`Surface`](crate::surface::Surface) exists for the documents
/// that fall outside that guarantee, and the guarantee should be checked rather than assumed.
pub fn recipes_are_all_in_tree(cookbook: &Cookbook, workspace: &Workspace) -> Vec<String> {
    cookbook
        .crates()
        .into_iter()
        .filter(|name| !workspace.contains_package(name))
        .map(|name| name.as_str().to_string())
        .collect()
}

/// No walkthrough here shares an identifier with a cookbook recipe.
///
/// Returns the collisions, empty when it holds.
pub fn catalogues_are_disjoint(cookbook: &Cookbook, walkthroughs: &[Walkthrough]) -> Vec<String> {
    let recipe_ids: Vec<String> = cookbook
        .recipes()
        .iter()
        .map(|recipe| recipe.id().as_str().to_string())
        .collect();
    walkthroughs
        .iter()
        .map(|walkthrough| walkthrough.id().as_str().to_string())
        .filter(|id| recipe_ids.contains(id))
        .collect()
}

/// Titles of everything in scope this crate did not implement, for a caller writing a status line.
///
/// Sixteen of twenty. Returned from [`classification`] rather than from a second list, so the
/// number cannot drift from the classification it summarises.
pub fn unimplemented_titles() -> Vec<&'static str> {
    classification()
        .into_iter()
        .filter(|row| !row.verdict.is_citable_here())
        .map(|row| row.title)
        .collect()
}
