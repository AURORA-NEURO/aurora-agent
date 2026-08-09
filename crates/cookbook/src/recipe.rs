//! The recipe: a task-shaped entry point that cannot be vacuous (19.09, 19.11, 19.13).
//!
//! Blueprint 19 asks for worked examples. The obvious build — a folder of sample programs — rots
//! within a week and proves nothing while it rots, because nothing in a sample program says what
//! it was supposed to demonstrate or how a reader would know it still does. `bioprism-examples`
//! answered that for *vertical slices* by making a slice carry the property it exercises and the
//! obstacle blocking the ones it cannot. This module answers the same question one level up, for
//! the reader who arrives with a goal rather than a crate name.
//!
//! # What a recipe is
//!
//! A [`Recipe`] is five things, all mandatory:
//!
//! 1. a **goal**, phrased as the sentence the reader would have said out loud;
//! 2. **ordered steps**, each naming a real crate and the real entry points it calls;
//! 3. the **claim** it demonstrates — what following the steps is evidence *for*;
//! 4. at least one **checkable property** — how a reader confirms the claim still holds;
//! 5. the **pitfall** — the thing that is easy to get wrong here, and why it is wrong.
//!
//! # Why the fields are private
//!
//! `AGENTS.md` states the house rule: "Where a rule can be made unrepresentable, make it
//! unrepresentable." The rule here is that a recipe without a checkable property is not a recipe.
//! So [`Recipe`]'s fields are private, the only constructor is [`RecipeDraft::seal`], and `seal`
//! returns [`CookbookError::NoCheckableProperty`] rather than a `Recipe`. There is no
//! `Recipe { .. }` literal a caller can write, no `Default`, and no `set_property` that could be
//! forgotten.
//!
//! Deserialisation goes through the same gate. `Recipe` is `#[serde(try_from = ...)]` over a
//! private wire struct, so a recipe whose property field was dropped in transit fails to parse
//! instead of arriving as a weaker object that still says `Recipe` in the type signature. That
//! matters more than it looks: the wire form is the form a recipe travels in.
//!
//! # What a recipe deliberately does not do
//!
//! **It does not run.** Nothing in this crate calls `bioprism_fiber::compile`, generates a world,
//! or executes a pipeline. A recipe *describes* an order of operations and *verifies its
//! references* ([`crate::verify`]); it never produces the artefact it talks about. Two reasons,
//! both structural. `bioprism-examples` already owns execution — a second executing catalogue
//! would be a second answer to "does this still work", and the two would drift. And a cookbook
//! that linked every crate it mentions would take a dependency on nearly the whole workspace,
//! which means it could not be built while any of them was mid-edit — exactly when a reader most
//! needs to know what the platform can do.
//!
//! The honest consequence is stated rather than hidden: **a green cookbook proves that every
//! crate, entry point and enforcing test a recipe names exists, and that its documentation route
//! compiles within its budget. It does not prove the recipe produces the result it claims.** The
//! test named in each [`Check::EnforcedByTest`] is what proves that, and it lives in the crate
//! that owns the behaviour.

use crate::error::CookbookError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A stable recipe identifier, used verbatim as the suffix of its documentation-graph module id.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RecipeId(String);

impl RecipeId {
    pub fn parse(value: impl Into<String>) -> Result<Self, CookbookError> {
        let value = value.into();
        if value.is_empty() {
            return Err(CookbookError::MalformedId(value, "empty"));
        }
        if value.chars().any(char::is_whitespace) {
            return Err(CookbookError::MalformedId(value, "contains whitespace"));
        }
        if !value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(CookbookError::MalformedId(
                value,
                "recipe ids are lowercase ascii, digits and hyphens",
            ));
        }
        Ok(RecipeId(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RecipeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<RecipeId> for String {
    fn from(value: RecipeId) -> Self {
        value.0
    }
}

impl TryFrom<String> for RecipeId {
    type Error = CookbookError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        RecipeId::parse(value)
    }
}

/// A workspace package name, as it appears in a crate's `Cargo.toml`: `bioprism-fiber`.
///
/// A separate type from [`EntryPoint`] because the two are spelled differently — a package name
/// hyphenates where a Rust path underscores — and a call site that took two strings would let an
/// author pass either in either position and only find out when the verifier could not resolve it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CrateName(String);

impl CrateName {
    pub fn parse(value: impl Into<String>) -> Result<Self, CookbookError> {
        let value = value.into();
        if value.is_empty() || value.chars().any(char::is_whitespace) {
            return Err(CookbookError::MalformedId(
                value,
                "crate names are non-empty and whitespace-free",
            ));
        }
        Ok(CrateName(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The crate as a Rust path segment: `bioprism-fiber` becomes `bioprism_fiber`.
    pub fn as_path_segment(&self) -> String {
        self.0.replace('-', "_")
    }
}

impl fmt::Display for CrateName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CrateName> for String {
    fn from(value: CrateName) -> Self {
        value.0
    }
}

impl TryFrom<String> for CrateName {
    type Error = CookbookError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        CrateName::parse(value)
    }
}

/// A Rust path rooted at a crate: `bioprism_fiber::compile`, `bioprism_docgraph::route::TaskRoute`.
///
/// Rooted rather than bare because [`crate::verify`] has to know which `lib.rs` to open, and a
/// bare item name says nothing about that. Segments are validated as identifiers so that a
/// mistyped path is a construction error rather than a verification finding — the two need
/// different fixes, and reporting a typo as "this symbol no longer exists" would send a reader to
/// the wrong crate.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct EntryPoint(String);

impl EntryPoint {
    pub fn parse(value: impl Into<String>) -> Result<Self, CookbookError> {
        let value = value.into();
        let segments: Vec<&str> = value.split("::").collect();
        if segments.len() < 2 {
            return Err(CookbookError::MalformedEntryPoint(
                value,
                "an entry point is a path rooted at a crate, such as `bioprism_fiber::compile`",
            ));
        }
        for segment in &segments {
            if segment.is_empty() {
                return Err(CookbookError::MalformedEntryPoint(
                    value.clone(),
                    "empty path segment",
                ));
            }
            if !segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                return Err(CookbookError::MalformedEntryPoint(
                    value.clone(),
                    "path segments are ascii identifiers",
                ));
            }
        }
        Ok(EntryPoint(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The leading segment, as a package name: `bioprism_fiber::compile` roots at `bioprism-fiber`.
    pub fn crate_name(&self) -> CrateName {
        let root = self.0.split("::").next().unwrap_or_default();
        CrateName(root.replace('_', "-"))
    }

    /// The path segments after the crate root, in order.
    pub fn item_path(&self) -> Vec<&str> {
        self.0.split("::").skip(1).collect()
    }
}

impl fmt::Display for EntryPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<EntryPoint> for String {
    fn from(value: EntryPoint) -> Self {
        value.0
    }
}

impl TryFrom<String> for EntryPoint {
    type Error = CookbookError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        EntryPoint::parse(value)
    }
}

/// One operation in a recipe, naming the crate that performs it and the entry points it calls.
///
/// The crate is stored separately from the entry points rather than derived from them, because a
/// step can be an instruction to *run* a crate's tests, which names a package but no symbol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Step {
    /// What this step does, in the imperative.
    pub action: String,
    pub krate: CrateName,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entry_points: Vec<EntryPoint>,
}

impl Step {
    pub fn new(krate: CrateName, action: impl Into<String>) -> Self {
        Step {
            action: action.into(),
            krate,
            entry_points: Vec::new(),
        }
    }

    pub fn calling(mut self, entry: EntryPoint) -> Self {
        self.entry_points.push(entry);
        self
    }
}

/// A test that already exists in this workspace and already enforces something.
///
/// The path is workspace-relative so [`crate::verify`] can open it from a repository root without
/// this crate ever knowing where the repository lives. `name` is the test function's name, which
/// is why `AGENTS.md` requires tests to state their claim in the name: a reference to
/// `test_budget_2` would tell a reader nothing, and this type would still accept it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceTest {
    pub krate: CrateName,
    /// Workspace-relative, forward-slashed: `crates/mutation/tests/metamorphic.rs`.
    pub path: String,
    pub name: String,
}

impl WorkspaceTest {
    pub fn new(krate: CrateName, path: impl Into<String>, name: impl Into<String>) -> Self {
        WorkspaceTest {
            krate,
            path: path.into(),
            name: name.into(),
        }
    }
}

/// How a reader confirms a property.
///
/// The two variants are not interchangeable and the distinction is the point.
/// [`Check::EnforcedByTest`] is a *standing* guarantee: someone else's CI fails when it stops
/// being true. [`Check::Observable`] is an instruction to the reader — it holds only when they
/// actually look. A cookbook that presented both as "checked" would be claiming continuous
/// verification for things nothing continuously verifies.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "check")]
pub enum Check {
    /// A named test in the workspace fails if the property stops holding.
    EnforcedByTest(WorkspaceTest),
    /// The reader inspects a stated observable and compares it against a stated expectation.
    Observable { observe: String, expect: String },
}

impl Check {
    /// Whether something other than the reader will notice when this stops being true.
    pub fn is_continuously_enforced(&self) -> bool {
        matches!(self, Check::EnforcedByTest(_))
    }

    pub fn test(&self) -> Option<&WorkspaceTest> {
        match self {
            Check::EnforcedByTest(test) => Some(test),
            Check::Observable { .. } => None,
        }
    }
}

/// A property of the recipe's outcome that a reader can confirm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckableProperty {
    /// The property, phrased so it can be true or false.
    pub statement: String,
    pub check: Check,
}

impl CheckableProperty {
    pub fn new(statement: impl Into<String>, check: Check) -> Self {
        CheckableProperty {
            statement: statement.into(),
            check,
        }
    }
}

/// What following the recipe is evidence for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claim {
    pub statement: String,
    /// Blueprint module ids that assert the claim, cited as text rather than as a typed reference
    /// because the blueprint is not a build input and a typed reference would imply it is.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blueprint_modules: Vec<String>,
}

impl Claim {
    pub fn new(statement: impl Into<String>) -> Self {
        Claim {
            statement: statement.into(),
            blueprint_modules: Vec::new(),
        }
    }

    pub fn citing(mut self, module: impl Into<String>) -> Self {
        self.blueprint_modules.push(module.into());
        self
    }
}

/// The thing that is easy to get wrong at this task, and the reason it is wrong.
///
/// `why` is mandatory and separate from `mistake` because "do not do X" is an instruction and
/// "X is wrong because Y" is a reason, and only the second survives contact with a reader who
/// thinks their situation is different.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pitfall {
    pub mistake: String,
    pub why: String,
}

impl Pitfall {
    pub fn new(mistake: impl Into<String>, why: impl Into<String>) -> Self {
        Pitfall {
            mistake: mistake.into(),
            why: why.into(),
        }
    }
}

/// A worked recipe. Constructed only through [`Recipe::draft`] and [`RecipeDraft::seal`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RecipeWire", into = "RecipeWire")]
pub struct Recipe {
    id: RecipeId,
    goal: String,
    steps: Vec<Step>,
    claim: Claim,
    properties: Vec<CheckableProperty>,
    pitfall: Pitfall,
    reading: Vec<String>,
    budget: Option<u32>,
}

impl Recipe {
    /// Begin a recipe. The result is not a `Recipe` and cannot be used as one.
    pub fn draft(id: RecipeId, goal: impl Into<String>) -> RecipeDraft {
        RecipeDraft {
            id,
            goal: goal.into(),
            steps: Vec::new(),
            claim: None,
            properties: Vec::new(),
            pitfall: None,
            reading: Vec::new(),
            budget: None,
        }
    }

    pub fn id(&self) -> &RecipeId {
        &self.id
    }

    pub fn goal(&self) -> &str {
        &self.goal
    }

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn claim(&self) -> &Claim {
        &self.claim
    }

    /// At least one, always. The type has no state in which this is empty.
    pub fn properties(&self) -> &[CheckableProperty] {
        &self.properties
    }

    pub fn pitfall(&self) -> &Pitfall {
        &self.pitfall
    }

    /// Documentation-graph module ids this recipe wants read alongside it, beyond the crates its
    /// steps name. Optional in the route sense: they are dropped for budget before anything
    /// mandatory is touched.
    pub fn reading(&self) -> &[String] {
        &self.reading
    }

    /// Estimated-token ceiling for the recipe's documentation route. `None` is unbounded, matching
    /// [`bioprism_docgraph::TaskRoute::budget`]; it does not mean "pick a default", because a
    /// default budget is a truncation policy nobody wrote down.
    pub fn budget(&self) -> Option<u32> {
        self.budget
    }

    /// Every crate the recipe names, deduplicated, in first-mention order.
    pub fn crates(&self) -> Vec<CrateName> {
        let mut out: Vec<CrateName> = Vec::new();
        for step in &self.steps {
            if !out.contains(&step.krate) {
                out.push(step.krate.clone());
            }
            for entry in &step.entry_points {
                let owner = entry.crate_name();
                if !out.contains(&owner) {
                    out.push(owner);
                }
            }
        }
        for property in &self.properties {
            if let Some(test) = property.check.test() {
                if !out.contains(&test.krate) {
                    out.push(test.krate.clone());
                }
            }
        }
        out
    }

    /// Every entry point the recipe names, in step order.
    pub fn entry_points(&self) -> Vec<&EntryPoint> {
        self.steps
            .iter()
            .flat_map(|step| step.entry_points.iter())
            .collect()
    }

    /// Every workspace test the recipe leans on.
    pub fn enforcing_tests(&self) -> Vec<&WorkspaceTest> {
        self.properties
            .iter()
            .filter_map(|property| property.check.test())
            .collect()
    }

    /// Whether at least one property is guarded by a test rather than by the reader's attention.
    ///
    /// Not an invariant: a recipe whose only check is an observable is legitimate and is what an
    /// honest catalogue looks like when nothing yet enforces the claim. It is reported rather than
    /// forbidden, by [`crate::report::CookbookReport`].
    pub fn is_continuously_enforced(&self) -> bool {
        self.properties
            .iter()
            .any(|property| property.check.is_continuously_enforced())
    }

    /// The recipe as an agent reads it. Deterministic, and the text that gets costed when the
    /// recipe becomes a documentation-graph node.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# recipe {}\n\n", self.id));
        out.push_str(&format!("goal: {}\n\n", self.goal));
        out.push_str("steps:\n");
        for (index, step) in self.steps.iter().enumerate() {
            out.push_str(&format!("  {}. [{}] {}\n", index + 1, step.krate, step.action));
            for entry in &step.entry_points {
                out.push_str(&format!("     calls {entry}\n"));
            }
        }
        out.push_str(&format!("\nclaim: {}\n", self.claim.statement));
        if !self.claim.blueprint_modules.is_empty() {
            out.push_str(&format!(
                "blueprint: {}\n",
                self.claim.blueprint_modules.join(", ")
            ));
        }
        out.push_str("\nchecks:\n");
        for property in &self.properties {
            out.push_str(&format!("  - {}\n", property.statement));
            match &property.check {
                Check::EnforcedByTest(test) => {
                    out.push_str(&format!("    enforced by {}::{}\n", test.path, test.name));
                }
                Check::Observable { observe, expect } => {
                    out.push_str(&format!("    observe {observe}; expect {expect}\n"));
                }
            }
        }
        out.push_str(&format!(
            "\neasy to get wrong: {}\n  because {}\n",
            self.pitfall.mistake, self.pitfall.why
        ));
        out
    }
}

impl fmt::Display for Recipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render())
    }
}

/// A recipe under construction. Cannot be passed anywhere a [`Recipe`] is expected.
#[derive(Debug, Clone)]
pub struct RecipeDraft {
    id: RecipeId,
    goal: String,
    steps: Vec<Step>,
    claim: Option<Claim>,
    properties: Vec<CheckableProperty>,
    pitfall: Option<Pitfall>,
    reading: Vec<String>,
    budget: Option<u32>,
}

impl RecipeDraft {
    pub fn step(mut self, step: Step) -> Self {
        self.steps.push(step);
        self
    }

    pub fn demonstrating(mut self, claim: Claim) -> Self {
        self.claim = Some(claim);
        self
    }

    pub fn checked_by(mut self, property: CheckableProperty) -> Self {
        self.properties.push(property);
        self
    }

    pub fn easy_to_get_wrong(mut self, pitfall: Pitfall) -> Self {
        self.pitfall = Some(pitfall);
        self
    }

    pub fn also_reading(mut self, module: impl Into<String>) -> Self {
        self.reading.push(module.into());
        self
    }

    pub fn with_budget(mut self, budget: u32) -> Self {
        self.budget = Some(budget);
        self
    }

    /// The only way to obtain a [`Recipe`].
    ///
    /// Every branch here is a recipe an author could otherwise have shipped: a title with no
    /// steps, steps with nothing to prove, a claim nobody can check, a happy path with no warning.
    /// Refusing at construction is what keeps the catalogue's promise — that every entry in it is
    /// checkable — a property of the *type* rather than of a review process.
    pub fn seal(self) -> Result<Recipe, CookbookError> {
        let id = self.id.to_string();
        if self.goal.trim().is_empty() {
            return Err(CookbookError::EmptyField {
                recipe: id,
                field: "goal",
            });
        }
        if self.steps.is_empty() {
            return Err(CookbookError::NoSteps(id));
        }
        for step in &self.steps {
            if step.action.trim().is_empty() {
                return Err(CookbookError::EmptyField {
                    recipe: id,
                    field: "step action",
                });
            }
        }
        let Some(claim) = self.claim else {
            return Err(CookbookError::NoClaim(id));
        };
        if claim.statement.trim().is_empty() {
            return Err(CookbookError::EmptyField {
                recipe: id,
                field: "claim statement",
            });
        }
        if self.properties.is_empty() {
            return Err(CookbookError::NoCheckableProperty(id));
        }
        for property in &self.properties {
            if property.statement.trim().is_empty() {
                return Err(CookbookError::EmptyField {
                    recipe: id,
                    field: "property statement",
                });
            }
            if let Check::Observable { observe, expect } = &property.check {
                if observe.trim().is_empty() || expect.trim().is_empty() {
                    return Err(CookbookError::EmptyField {
                        recipe: id,
                        field: "observable check",
                    });
                }
            }
        }
        let Some(pitfall) = self.pitfall else {
            return Err(CookbookError::NoPitfall(id));
        };
        if pitfall.mistake.trim().is_empty() || pitfall.why.trim().is_empty() {
            return Err(CookbookError::EmptyField {
                recipe: id,
                field: "pitfall",
            });
        }
        Ok(Recipe {
            id: self.id,
            goal: self.goal,
            steps: self.steps,
            claim,
            properties: self.properties,
            pitfall,
            reading: self.reading,
            budget: self.budget,
        })
    }
}

/// The serialisation form. Private: it exists only so `try_from` can route deserialisation
/// through [`RecipeDraft::seal`], and exposing it would hand callers the unchecked constructor
/// that the private fields exist to withhold.
#[derive(Serialize, Deserialize)]
struct RecipeWire {
    id: RecipeId,
    goal: String,
    #[serde(default)]
    steps: Vec<Step>,
    #[serde(default)]
    claim: Option<Claim>,
    #[serde(default)]
    properties: Vec<CheckableProperty>,
    #[serde(default)]
    pitfall: Option<Pitfall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    reading: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    budget: Option<u32>,
}

impl TryFrom<RecipeWire> for Recipe {
    type Error = CookbookError;

    fn try_from(wire: RecipeWire) -> Result<Self, Self::Error> {
        let mut draft = Recipe::draft(wire.id, wire.goal);
        draft.steps = wire.steps;
        draft.claim = wire.claim;
        draft.properties = wire.properties;
        draft.pitfall = wire.pitfall;
        draft.reading = wire.reading;
        draft.budget = wire.budget;
        draft.seal()
    }
}

impl From<Recipe> for RecipeWire {
    fn from(recipe: Recipe) -> Self {
        RecipeWire {
            id: recipe.id,
            goal: recipe.goal,
            steps: recipe.steps,
            claim: Some(recipe.claim),
            properties: recipe.properties,
            pitfall: Some(recipe.pitfall),
            reading: recipe.reading,
            budget: recipe.budget,
        }
    }
}
