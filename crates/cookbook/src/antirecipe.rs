//! Anti-recipes: the things a reader will try that do not work, and why.
//!
//! Blueprint 19 does not ask for these. It is a catalogue of examples, and every example in it
//! succeeds. That is the failure mode of every cookbook ever written: a reader who has only seen
//! working code has no way to recognise the shape of the mistake they are about to make, and the
//! mistakes available in this workspace are unusually seductive — each of them produces an artifact
//! that *looks* like a result. A smaller context. A correct verdict. A larger benchmark. Nothing in
//! the output says the answer was reached illegitimately, which is precisely why `AGENTS.md` spends
//! its first section on the distinctions and why the workspace has tests that enforce them.
//!
//! # An anti-recipe cannot be constructed without the test that already refutes it
//!
//! [`AntiRecipe::new`] takes a [`WorkspaceTest`] by value. Not an `Option`, not a list that may be
//! empty: the signature makes an unrefuted warning unrepresentable. The reason is that a warning
//! with no enforcement is an opinion, and an opinion in a reference document is worse than
//! silence — it will be believed, and nothing will notice when it stops being true. If a mistake
//! is real, something in this workspace already fails when someone makes it; if nothing does, the
//! honest artifact is a gap in [`crate::report::CookbookReport`], not a paragraph here.
//!
//! [`crate::verify`] then checks the named test still exists, by reading the file. A cookbook that
//! cited a deleted test would be making the same mistake it is warning about.
//!
//! # These sharpen `AGENTS.md` rather than restating it
//!
//! Each anti-recipe carries the sentence in `AGENTS.md` it descends from, as a
//! [`PinnedQuote`]. `AGENTS.md` states the rule; the anti-recipe states
//! the concrete thing a reader does at a keyboard that violates it, and the specific test that
//! catches them. A rule and its instance are different objects and the reader needs both — which
//! is why these are `refines` edges in [`crate::graph`], not copies.

use crate::error::CookbookError;
use crate::quotes::PinnedQuote;
use crate::recipe::{RecipeId, WorkspaceTest};
use serde::{Deserialize, Serialize};

/// A mistake, the reason it is a mistake, and the test that catches it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AntiRecipeWire", into = "AntiRecipeWire")]
pub struct AntiRecipe {
    id: RecipeId,
    attempt: String,
    why_it_fails: String,
    instead: String,
    enforced_by: Vec<WorkspaceTest>,
    house_rule: Option<PinnedQuote>,
}

impl AntiRecipe {
    /// The only constructor. The mandatory [`WorkspaceTest`] is the whole point of the signature.
    pub fn new(
        id: RecipeId,
        attempt: impl Into<String>,
        why_it_fails: impl Into<String>,
        instead: impl Into<String>,
        enforced_by: WorkspaceTest,
    ) -> Result<Self, CookbookError> {
        let anti = AntiRecipe {
            id,
            attempt: attempt.into(),
            why_it_fails: why_it_fails.into(),
            instead: instead.into(),
            enforced_by: vec![enforced_by],
            house_rule: None,
        };
        anti.validate()?;
        Ok(anti)
    }

    /// A second (or third) test that catches the same mistake from a different angle.
    pub fn also_enforced_by(mut self, test: WorkspaceTest) -> Self {
        self.enforced_by.push(test);
        self
    }

    /// The `AGENTS.md` sentence this instance descends from.
    pub fn sharpening(mut self, rule: PinnedQuote) -> Self {
        self.house_rule = Some(rule);
        self
    }

    fn validate(&self) -> Result<(), CookbookError> {
        let id = self.id.to_string();
        for (field, value) in [
            ("attempt", &self.attempt),
            ("why_it_fails", &self.why_it_fails),
            ("instead", &self.instead),
        ] {
            if value.trim().is_empty() {
                return Err(CookbookError::EmptyField { recipe: id, field });
            }
        }
        if self.enforced_by.is_empty() {
            return Err(CookbookError::NoCheckableProperty(id));
        }
        Ok(())
    }

    pub fn id(&self) -> &RecipeId {
        &self.id
    }

    /// What the reader is about to do.
    pub fn attempt(&self) -> &str {
        &self.attempt
    }

    pub fn why_it_fails(&self) -> &str {
        &self.why_it_fails
    }

    /// The move that does work. An anti-recipe with no alternative is a scolding.
    pub fn instead(&self) -> &str {
        &self.instead
    }

    /// Never empty.
    pub fn enforced_by(&self) -> &[WorkspaceTest] {
        &self.enforced_by
    }

    pub fn house_rule(&self) -> Option<&PinnedQuote> {
        self.house_rule.as_ref()
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# anti-recipe {}\n\n", self.id));
        out.push_str(&format!("attempt: {}\n", self.attempt));
        out.push_str(&format!("why it fails: {}\n", self.why_it_fails));
        out.push_str(&format!("instead: {}\n", self.instead));
        out.push_str("already enforced by:\n");
        for test in &self.enforced_by {
            out.push_str(&format!("  - {}::{}\n", test.path, test.name));
        }
        if let Some(rule) = &self.house_rule {
            out.push_str(&format!("sharpens {}: {}\n", rule.source, rule.needle));
        }
        out
    }
}

/// The serialisation form, private for the same reason [`crate::recipe::Recipe`]'s is: it is the
/// only way `try_from` can route deserialisation through [`AntiRecipe::validate`], and publishing
/// it would hand back the unchecked constructor.
#[derive(Serialize, Deserialize)]
struct AntiRecipeWire {
    id: RecipeId,
    attempt: String,
    why_it_fails: String,
    instead: String,
    #[serde(default)]
    enforced_by: Vec<WorkspaceTest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    house_rule: Option<PinnedQuote>,
}

impl TryFrom<AntiRecipeWire> for AntiRecipe {
    type Error = CookbookError;

    fn try_from(wire: AntiRecipeWire) -> Result<Self, Self::Error> {
        let anti = AntiRecipe {
            id: wire.id,
            attempt: wire.attempt,
            why_it_fails: wire.why_it_fails,
            instead: wire.instead,
            enforced_by: wire.enforced_by,
            house_rule: wire.house_rule,
        };
        anti.validate()?;
        Ok(anti)
    }
}

impl From<AntiRecipe> for AntiRecipeWire {
    fn from(anti: AntiRecipe) -> Self {
        AntiRecipeWire {
            id: anti.id,
            attempt: anti.attempt,
            why_it_fails: anti.why_it_fails,
            instead: anti.instead,
            enforced_by: anti.enforced_by,
            house_rule: anti.house_rule,
        }
    }
}
