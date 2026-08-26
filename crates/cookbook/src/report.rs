//! The catalogue's own report, including the half that is missing.
//!
//! A cookbook that printed only its recipes would read as completeness, and the tasks nobody could
//! write a recipe for are exactly the ones a newcomer assumes are supported. `bioprism-examples`
//! makes the same argument about properties and answers it with a coverage report that enumerates
//! the unexercised claims with their obstacles. [`CookbookReport`] is that argument applied to
//! tasks: it carries the recipes, and it carries [`CookbookReport::unwritten`] — the recipes this
//! crate wanted to write and could not, each naming the capability that is missing.
//!
//! It also reports something narrower and easy to miss. A recipe whose only
//! [`Check`](crate::recipe::Check) is an
//! [`Observable`](crate::recipe::Check::Observable) is not continuously verified: it holds when a
//! reader looks and nothing notices when it stops. [`CookbookReport::not_continuously_enforced`]
//! lists those separately from the ones a test guards, because presenting the two as equally
//! "checked" would claim standing verification for something nothing stands over.
//!
//! # The digest
//!
//! The report content-addresses itself over canonical bytes, the same way `bioprism-examples` and
//! `bioprism-bioworlds` do, so a stored report can be checked against a recomputation without
//! rerunning anything. There is no clock in it: the digest is a function of the catalogue alone.

use crate::book::Cookbook;
use crate::error::CookbookError;
use crate::quotes::{examples_blockers, PinnedQuote};
use bioprism_ids::{CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A task somebody will want to do that this crate could not write a recipe for.
///
/// `blocker` names the concrete missing capability, not a vague absence, so a later contributor
/// knows what to change. Where the obstacle is one another crate has already recorded, `evidence`
/// pins that crate's own wording rather than paraphrasing it — see [`crate::quotes`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UnwrittenRecipe {
    /// The goal, phrased the way the reader would have phrased it.
    pub goal: String,
    /// Why no recipe exists. Concrete and checkable.
    pub blocker: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blueprint_modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<PinnedQuote>,
}

impl UnwrittenRecipe {
    fn new(goal: &str, blocker: &str, modules: &[&str]) -> Self {
        UnwrittenRecipe {
            goal: goal.to_string(),
            blocker: blocker.to_string(),
            blueprint_modules: modules.iter().map(|m| (*m).to_string()).collect(),
            evidence: None,
        }
    }

    fn with_evidence(mut self, quote: PinnedQuote) -> Self {
        self.evidence = Some(quote);
        self
    }
}

/// The recipes that could not be written, and the capability each one is waiting on.
///
/// Six entries. Four are quoted from `bioprism-examples`, which already recorded the obstacle
/// against the corresponding property and has the right to be the source of that wording; two are
/// this crate's own findings about the workspace.
pub fn unwritten_recipes() -> Vec<UnwrittenRecipe> {
    vec![
        UnwrittenRecipe::new(
            "Make the compiler abstain instead of forcing a verdict, when the evidence genuinely \
             underdetermines the decision.",
            "OracleStatus and OracleVerdict in bioprism-section can express abstention, and no path \
             in bioprism-fiber constructs one: the v0.1 oracle derives status solely from whether \
             the witness list is empty. bioprism-bioworlds ships an underdetermination slice, but it \
             declares which hypotheses it leaves live rather than making the compiler abstain.",
            &["43.28", "43.41", "19.04"],
        )
        .with_evidence(examples_blockers::abstention()),
        UnwrittenRecipe::new(
            "Choose a query backend from the portfolio, and have the compiler decline to compress \
             when compression would not pay.",
            "bioprism-backends implements the portfolio — Candidate, Portfolio, Declined, a phase \
             diagram — and bioprism-fiber never consults it. The gap is not a missing capability but \
             a missing call: the compiler hard-codes one backend and leaves the fallback empty.",
            &["43.36", "43.37", "19.14"],
        )
        .with_evidence(examples_blockers::backend_portfolio()),
        UnwrittenRecipe::new(
            "Run all six mutation families the specification names, not the four the generator can \
             produce.",
            "WorldSpec::LeakageMechanism has four members. Prevalence shift, segmentation \
             perturbation and assay uncertainty have no generator knob, so a recipe for them would \
             be a recipe for code that does not exist.",
            &["38.01", "19.05"],
        )
        .with_evidence(examples_blockers::mutation_families()),
        UnwrittenRecipe::new(
            "Trade context size against a stated bound on decision loss, and report the smallest \
             context that stays inside it.",
            "the fiber-query/0.1 wire schema carries neither permitted_actions nor decision_loss, so \
             there is no loss to trade distortion against and no set of permitted actions to quotient \
             by. Query::missing_contract_fields reports both as absent on every compile.",
            &["43.10", "43.12"],
        )
        .with_evidence(examples_blockers::decision_loss()),
        UnwrittenRecipe::new(
            "Drive a whole compile from a local CLI session, the way blueprint 19.19 shows it.",
            "crates/cli has no lib.rs — it is main.rs plus private modules — so there is no entry \
             point a recipe could name and no symbol this cookbook's verifier could resolve. A \
             recipe would have to quote a command line, which nothing in this workspace checks.",
            &["19.19", "11.11"],
        ),
        UnwrittenRecipe::new(
            "Publish a benchmark pack to a federated registry and consume it from another site.",
            "nothing in the workspace performs a cross-site exchange. bioprism-registry holds packs \
             and trust tiers and bioprism-hubapi serves an air-gapped site; neither implements the \
             push, pull and reconciliation that 19.20's flow is about, so every step of the recipe \
             would name something imagined.",
            &["19.20", "19.10"],
        ),
    ]
}

/// One recipe, flattened for the report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeSummary {
    pub id: String,
    pub goal: String,
    pub claim: String,
    pub steps: usize,
    pub crates: Vec<String>,
    pub entry_points: usize,
    /// Properties guarded by a named test rather than by the reader's attention.
    pub enforced_checks: usize,
    pub observable_checks: usize,
    pub pitfall: String,
}

/// What the catalogue contains, and what it does not.
///
/// Every struct the report is made of refuses a field it does not declare, and that is what makes
/// [`CookbookReport::digest_is_intact`] mean anything. The digest is recomputed by re-serialising
/// the *parsed* report, so a field a reader discarded is a field outside the seal: the
/// recomputation never sees it, the claimed digest still agrees, and a report with content nobody
/// hashed reads as intact. Refusing the unknown field is the only place that difference can be
/// caught, because by the time the struct exists the evidence is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CookbookReport {
    pub recipes: Vec<RecipeSummary>,
    pub anti_recipes: Vec<String>,
    /// Recipes whose only checks are observables. Empty is the goal; non-empty is honest.
    pub not_continuously_enforced: Vec<String>,
    pub unwritten: Vec<UnwrittenRecipe>,
    pub crates_named: Vec<String>,
    pub tests_leaned_on: usize,
    pub digest: String,
}

impl CookbookReport {
    /// Build the report from a catalogue. No filesystem access and no clock: the same catalogue
    /// always produces the same digest.
    pub fn of(cookbook: &Cookbook) -> Result<Self, CookbookError> {
        let recipes: Vec<RecipeSummary> = cookbook
            .recipes()
            .iter()
            .map(|recipe| RecipeSummary {
                id: recipe.id().to_string(),
                goal: recipe.goal().to_string(),
                claim: recipe.claim().statement.clone(),
                steps: recipe.steps().len(),
                crates: recipe.crates().iter().map(ToString::to_string).collect(),
                entry_points: recipe.entry_points().len(),
                enforced_checks: recipe.enforcing_tests().len(),
                observable_checks: recipe.properties().len() - recipe.enforcing_tests().len(),
                pitfall: recipe.pitfall().mistake.clone(),
            })
            .collect();

        let mut report = CookbookReport {
            not_continuously_enforced: cookbook
                .recipes()
                .iter()
                .filter(|recipe| !recipe.is_continuously_enforced())
                .map(|recipe| recipe.id().to_string())
                .collect(),
            anti_recipes: cookbook
                .anti_recipes()
                .iter()
                .map(|anti| anti.id().to_string())
                .collect(),
            recipes,
            unwritten: unwritten_recipes(),
            crates_named: cookbook.crates().iter().map(ToString::to_string).collect(),
            tests_leaned_on: cookbook.enforcing_tests().len(),
            digest: String::new(),
        };
        report.digest = report
            .recompute_digest()
            .map_err(|_| CookbookError::EmptyField {
                recipe: "<report>".to_string(),
                field: "digest",
            })?;
        Ok(report)
    }

    /// The report without its own digest field, which is what the digest is taken over.
    pub fn body(&self) -> Value {
        let mut value = serde_json::to_value(self).expect("the report is serialisable");
        if let Some(map) = value.as_object_mut() {
            map.remove("digest");
        }
        value
    }

    pub fn recompute_digest(&self) -> Result<String, CanonicalError> {
        ContentHash::of_value(&self.body()).map(|hash| hash.as_str().to_string())
    }

    pub fn digest_is_intact(&self) -> bool {
        self.recompute_digest()
            .is_ok_and(|recomputed| recomputed == self.digest)
    }

    /// A summary a newcomer can read without opening the JSON. Both halves, unwritten first —
    /// putting the gaps after the contents is how a gap list ends up unread.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("cookbook digest: {}\n\n", self.digest));
        out.push_str(&format!(
            "{} recipes, {} anti-recipes, {} crates named, {} workspace tests leaned on\n\n",
            self.recipes.len(),
            self.anti_recipes.len(),
            self.crates_named.len(),
            self.tests_leaned_on
        ));

        out.push_str("WANTED BUT NOT WRITTEN\n");
        for entry in &self.unwritten {
            out.push_str(&format!(
                "  [{}]\n    {}\n    blocked by: {}\n",
                entry.blueprint_modules.join(", "),
                entry.goal,
                entry.blocker
            ));
        }

        out.push_str("\nRECIPES\n");
        for recipe in &self.recipes {
            out.push_str(&format!(
                "  {}\n    {}\n    {} steps, {} entry points, {} enforced / {} observable checks\n    \
                 easy to get wrong: {}\n",
                recipe.id,
                recipe.goal,
                recipe.steps,
                recipe.entry_points,
                recipe.enforced_checks,
                recipe.observable_checks,
                recipe.pitfall
            ));
        }

        out.push_str("\nANTI-RECIPES\n");
        for id in &self.anti_recipes {
            out.push_str(&format!("  {id}\n"));
        }

        if !self.not_continuously_enforced.is_empty() {
            out.push_str("\nCHECKED ONLY WHEN SOMEBODY LOOKS\n");
            for id in &self.not_continuously_enforced {
                out.push_str(&format!("  {id}\n"));
            }
        }
        out
    }
}
