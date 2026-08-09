//! The catalogue as an enumerable object.
//!
//! `bioprism-examples` made the same move for vertical slices and the argument carries over: a
//! catalogue is only useful if something can say what is in it, resolve all of it at once, and
//! return the result as one artefact. A cookbook that could only be read one recipe at a time
//! would have no way to answer "is any of this still true", which is the question that matters
//! after a refactor.
//!
//! # Ids are unique because a report has to name exactly one entry
//!
//! [`Cookbook::from_parts`] rejects a duplicate id rather than shadowing the earlier entry, and it
//! rejects a recipe and an anti-recipe sharing an id even though they live in different lists —
//! they share a namespace in [`crate::graph`], where each becomes a documentation module, and a
//! collision there would silently drop one of them from every bundle.

use crate::antirecipe::AntiRecipe;
use crate::catalog;
use crate::error::CookbookError;
use crate::quotes::PinnedQuote;
use crate::recipe::{CrateName, Recipe, WorkspaceTest};
use crate::verify::{verify_cookbook, VerificationReport, Workspace};
use std::collections::BTreeSet;

/// Every recipe and anti-recipe under one roof.
#[derive(Debug, Clone)]
pub struct Cookbook {
    recipes: Vec<Recipe>,
    anti_recipes: Vec<AntiRecipe>,
}

impl Cookbook {
    /// The catalogue this crate ships.
    pub fn standard() -> Result<Self, CookbookError> {
        Cookbook::from_parts(catalog::recipes()?, catalog::anti_recipes()?)
    }

    pub fn from_parts(
        recipes: Vec<Recipe>,
        anti_recipes: Vec<AntiRecipe>,
    ) -> Result<Self, CookbookError> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for id in recipes
            .iter()
            .map(|recipe| recipe.id().to_string())
            .chain(anti_recipes.iter().map(|anti| anti.id().to_string()))
        {
            if !seen.insert(id.clone()) {
                return Err(CookbookError::DuplicateRecipe(id));
            }
        }
        Ok(Cookbook {
            recipes,
            anti_recipes,
        })
    }

    pub fn recipes(&self) -> &[Recipe] {
        &self.recipes
    }

    pub fn anti_recipes(&self) -> &[AntiRecipe] {
        &self.anti_recipes
    }

    pub fn recipe(&self, id: &str) -> Result<&Recipe, CookbookError> {
        self.recipes
            .iter()
            .find(|recipe| recipe.id().as_str() == id)
            .ok_or_else(|| CookbookError::UnknownRecipe(id.to_string()))
    }

    pub fn anti_recipe(&self, id: &str) -> Result<&AntiRecipe, CookbookError> {
        self.anti_recipes
            .iter()
            .find(|anti| anti.id().as_str() == id)
            .ok_or_else(|| CookbookError::UnknownRecipe(id.to_string()))
    }

    /// Every crate any entry names, deduplicated and ordered.
    pub fn crates(&self) -> Vec<CrateName> {
        let mut out: BTreeSet<CrateName> = BTreeSet::new();
        for recipe in &self.recipes {
            out.extend(recipe.crates());
        }
        for anti in &self.anti_recipes {
            for test in anti.enforced_by() {
                out.insert(test.krate.clone());
            }
        }
        out.into_iter().collect()
    }

    /// Every workspace test the catalogue leans on, deduplicated by path and name.
    pub fn enforcing_tests(&self) -> Vec<WorkspaceTest> {
        let mut out: Vec<WorkspaceTest> = Vec::new();
        for test in self
            .recipes
            .iter()
            .flat_map(Recipe::enforcing_tests)
            .cloned()
            .chain(
                self.anti_recipes
                    .iter()
                    .flat_map(|anti| anti.enforced_by().iter().cloned()),
            )
        {
            if !out.iter().any(|seen| seen == &test) {
                out.push(test);
            }
        }
        out
    }

    /// Every quotation the catalogue attributes to a file it does not own.
    pub fn quotes(&self) -> Vec<PinnedQuote> {
        let mut out: Vec<PinnedQuote> = crate::quotes::all();
        for anti in &self.anti_recipes {
            if let Some(rule) = anti.house_rule() {
                if !out.contains(rule) {
                    out.push(rule.clone());
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// Resolve every reference the catalogue makes against a workspace.
    pub fn verify(&self, workspace: &Workspace) -> VerificationReport {
        verify_cookbook(workspace, &self.recipes, &self.anti_recipes, &self.quotes())
    }
}

/// The shipped catalogue.
pub fn standard_cookbook() -> Result<Cookbook, CookbookError> {
    Cookbook::standard()
}
