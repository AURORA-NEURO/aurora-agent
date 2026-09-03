//! The declared estimand, and the label a simulator result can never shed (26.09).
//!
//! 26.09's protocol step 1 is "declare intervention, comparator, unit, outcome, and horizon" —
//! five elements, all five required. [`Estimand`] has no public constructor other than
//! [`Estimand::declare`], which returns [`EstimandError::MissingElement`] naming the first missing
//! one, so an incomplete estimand cannot exist and an analysis cannot be attached to a question
//! nobody finished asking. That single refusal covers 26.09's "wrong time zero" failure mode by
//! construction: time zero is the horizon's origin and there is no estimand without a horizon.
//!
//! # A model-conditional finding does not become a real-world one
//!
//! 26.09's design detail: "Mechanistic simulators can provide exact counterfactual oracles inside
//! synthetic worlds, but their conclusions are labeled model-conditional and never upgraded
//! automatically to real-world truth." [`Evidentiary`] is that label, and the mechanism is that
//! [`Finding::promote`] takes a [`Corroboration`] — a *named external replication* — rather than a
//! boolean. There is no `set_evidentiary`, no `From<Finding>` that drops the label, and
//! [`Finding::claim_language`] returns prose that carries the qualifier, so the failure mode
//! "simulator treated as clinical evidence" has no cheap path.
//!
//! The word *automatically* in the blueprint is doing work and is honoured: promotion is possible,
//! it just cannot happen as a side effect. A promotion records what corroborated it.
//!
//! # Association is not intervention
//!
//! 26.09 step 5 is "separate association from intervention claim" and its last failure mode is
//! "causal language from predictive feature importance". [`ClaimKind`] separates the two and
//! [`Finding::claim_language`] renders each in its own vocabulary; an association finding cannot
//! be rendered with interventional wording because the renderer branches on the variant.
//!
//! # Not implemented
//!
//! **No identification engine.** 26.09's step 2 is "validate identification assumptions" and its
//! failure modes name colliders and post-treatment adjustment. Deciding those needs a causal graph
//! and a d-separation algorithm; the section supplies neither a graph format nor the criterion,
//! and `bioprism-foundation` owns causality in this workspace. [`Identification`] therefore records
//! *what a caller claimed and what they checked*, with `NotAssessed` as a real state, and refuses
//! to infer validity. That is `bioprism-safety`'s `NotChecked` move again: modelling a control is
//! allowed, claiming one is not.
//!
//! **No estimator and no bias simulation.** "Bias under simulation" and "decision regret" are
//! metrics 26.09 names without definitions.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::EstimandError;

const MAX_ESTIMAND_TEXT_BYTES: usize = 256;
const MAX_ASSUMPTIONS: usize = 256;
const MAX_IDENTIFICATION_CHECKS: usize = 256;
const MAX_CORROBORATIONS: usize = 1024;

/// The five elements 26.09 requires before any causal analysis.
///
/// Public fields on a private-constructed type: reading them is fine, assembling one without going
/// through [`Estimand::declare`] is not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Estimand {
    intervention: String,
    comparator: String,
    unit: String,
    outcome: String,
    horizon: String,
    /// The population and setting the estimand is defined over. 26.09's "test transportability"
    /// step needs a `from`, and this is it.
    scope: String,
}

impl Estimand {
    /// Declare all five elements plus the scope they are defined in.
    ///
    /// Empty or whitespace-only strings count as missing. The check is deliberately crude: it
    /// cannot tell a real comparator from the word "control", and pretending otherwise would be
    /// worse than the honest floor of "something was written here".
    pub fn declare(
        intervention: impl Into<String>,
        comparator: impl Into<String>,
        unit: impl Into<String>,
        outcome: impl Into<String>,
        horizon: impl Into<String>,
        scope: impl Into<String>,
    ) -> Result<Self, EstimandError> {
        let estimand = Estimand {
            intervention: intervention.into(),
            comparator: comparator.into(),
            unit: unit.into(),
            outcome: outcome.into(),
            horizon: horizon.into(),
            scope: scope.into(),
        };
        for (name, value) in [
            ("intervention", &estimand.intervention),
            ("comparator", &estimand.comparator),
            ("unit", &estimand.unit),
            ("outcome", &estimand.outcome),
            ("horizon", &estimand.horizon),
        ] {
            if value.trim().is_empty() {
                return Err(EstimandError::MissingElement(name));
            }
            validate_estimand_text(name, value)?;
        }
        if estimand.scope.trim().is_empty() {
            return Err(EstimandError::MissingElement("scope"));
        }
        validate_estimand_text("scope", &estimand.scope)?;
        Ok(estimand)
    }

    fn validate(&self) -> Result<(), EstimandError> {
        for (name, value) in [
            ("intervention", &self.intervention),
            ("comparator", &self.comparator),
            ("unit", &self.unit),
            ("outcome", &self.outcome),
            ("horizon", &self.horizon),
            ("scope", &self.scope),
        ] {
            if value.trim().is_empty() {
                return Err(EstimandError::MissingElement(name));
            }
            validate_estimand_text(name, value)?;
        }
        Ok(())
    }

    /// The intervention being evaluated.
    pub fn intervention(&self) -> &str {
        &self.intervention
    }

    /// What it is being compared against.
    pub fn comparator(&self) -> &str {
        &self.comparator
    }

    /// The unit of analysis.
    pub fn unit(&self) -> &str {
        &self.unit
    }

    /// The outcome measured.
    pub fn outcome(&self) -> &str {
        &self.outcome
    }

    /// The horizon, which fixes time zero.
    pub fn horizon(&self) -> &str {
        &self.horizon
    }

    /// The population and setting.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// Refuse a transport out of scope.
    ///
    /// There is no transport *rule* here: `bioprism-scope` owns scope mappings and their loss
    /// ledgers. This is the gate that says a transport was never declared, which is 26.09's "scope
    /// violation rate" made into a refusal rather than a counter.
    pub fn transport_to(
        &self,
        target: &str,
        declared: &BTreeSet<String>,
    ) -> Result<(), EstimandError> {
        validate_estimand_text("target", target)?;
        if declared.contains(target) {
            Ok(())
        } else {
            Err(EstimandError::OutOfScope {
                from: self.scope.clone(),
                to: target.to_string(),
            })
        }
    }
}

fn validate_estimand_text(field: &str, value: &str) -> Result<(), EstimandError> {
    if value != value.trim() {
        return Err(EstimandError::InvalidField {
            field: field.to_string(),
            detail: "leading or trailing whitespace is not allowed".into(),
        });
    }
    if value.len() > MAX_ESTIMAND_TEXT_BYTES {
        return Err(EstimandError::InvalidField {
            field: field.to_string(),
            detail: format!("text exceeds {MAX_ESTIMAND_TEXT_BYTES} bytes"),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(EstimandError::InvalidField {
            field: field.to_string(),
            detail: "control characters are not allowed".into(),
        });
    }
    Ok(())
}

/// What a finding is evidence about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "evidentiary")]
pub enum Evidentiary {
    /// Produced inside a mechanistic simulator or digital twin. True of the model, and of nothing
    /// else until something outside the model says otherwise.
    ModelConditional { model: String },
    /// Produced from observational data in a real population.
    Observational { dataset: String },
    /// Produced from a real experiment or perturbation.
    Experimental { study: String },
}

impl Evidentiary {
    /// Whether this finding's truth is contingent on a model being right.
    pub fn is_model_conditional(&self) -> bool {
        matches!(self, Evidentiary::ModelConditional { .. })
    }

    fn validate(&self) -> Result<(), EstimandError> {
        let (field, value) = match self {
            Evidentiary::ModelConditional { model } => ("model", model),
            Evidentiary::Observational { dataset } => ("dataset", dataset),
            Evidentiary::Experimental { study } => ("study", study),
        };
        if value.trim().is_empty() {
            return Err(EstimandError::InvalidField {
                field: field.into(),
                detail: "value must not be empty".into(),
            });
        }
        validate_estimand_text(field, value)
    }
}

/// Whether a finding is about association or about intervention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimKind {
    /// Two things move together in the observed data.
    Association,
    /// Setting the intervention changes the outcome.
    Intervention,
}

/// What was done about identification, which is not the same as whether it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "identification")]
pub enum Identification {
    /// Nobody assessed it. The default, and a real state.
    NotAssessed,
    /// A strategy was named and the assumptions it needs were listed. Naming is not verifying, and
    /// this variant does not claim the assumptions hold.
    Declared {
        strategy: String,
        assumptions: Vec<String>,
    },
    /// Negative controls or sensitivity analyses were run, with their outcomes recorded.
    Probed {
        strategy: String,
        assumptions: Vec<String>,
        checks: Vec<IdentificationCheck>,
    },
}

impl Identification {
    /// The assumptions named, if any were.
    pub fn assumptions(&self) -> &[String] {
        match self {
            Identification::NotAssessed => &[],
            Identification::Declared { assumptions, .. }
            | Identification::Probed { assumptions, .. } => assumptions,
        }
    }

    /// Whether any check was actually run.
    pub fn was_probed(&self) -> bool {
        matches!(self, Identification::Probed { .. })
    }

    fn validate(&self) -> Result<(), EstimandError> {
        let (strategy, assumptions, checks) = match self {
            Identification::NotAssessed => return Ok(()),
            Identification::Declared {
                strategy,
                assumptions,
            } => (strategy, assumptions, None),
            Identification::Probed {
                strategy,
                assumptions,
                checks,
            } => (strategy, assumptions, Some(checks)),
        };

        validate_identification_text("strategy", strategy)?;
        if assumptions.is_empty() {
            return Err(EstimandError::InvalidIdentification {
                detail: "at least one identification assumption is required".into(),
            });
        }
        validate_identification_list("assumptions", assumptions, MAX_ASSUMPTIONS)?;
        if let Some(checks) = checks {
            if checks.is_empty() {
                return Err(EstimandError::InvalidIdentification {
                    detail: "probed identification requires at least one check".into(),
                });
            }
            if checks.len() > MAX_IDENTIFICATION_CHECKS {
                return Err(EstimandError::InvalidIdentification {
                    detail: format!(
                        "checks exceed the {MAX_IDENTIFICATION_CHECKS}-item limit"
                    ),
                });
            }
            let mut names = BTreeSet::new();
            for check in checks {
                validate_identification_text("check name", &check.name)?;
                validate_identification_text("check detail", &check.detail)?;
                if !names.insert(&check.name) {
                    return Err(EstimandError::InvalidIdentification {
                        detail: format!("check `{}` appears more than once", check.name),
                    });
                }
            }
        }
        Ok(())
    }
}

/// One negative control or sensitivity analysis and what it showed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentificationCheck {
    pub name: String,
    /// Whether the check came out the way the strategy predicts. A failed negative control is a
    /// finding about the analysis, so it is retained rather than dropped.
    pub passed: bool,
    pub detail: String,
}

/// A named external result that corroborated a model-conditional finding.
///
/// Required to promote. Its existence is the mechanism: promotion needs an argument, and the
/// argument is stored with the promoted finding forever.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Corroboration {
    pub source: String,
    pub kind: ClaimKind,
    pub detail: String,
}

impl Corroboration {
    fn validate(&self) -> Result<(), EstimandError> {
        validate_identification_text("source", &self.source)?;
        validate_identification_text("detail", &self.detail)?;
        Ok(())
    }
}

/// A causal finding: an estimand, what kind of claim it makes, and what it rests on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub estimand: Estimand,
    pub kind: ClaimKind,
    pub basis: Evidentiary,
    pub identification: Identification,
    /// Empty until something outside the model corroborated this. Append-only through
    /// [`Finding::promote`].
    corroborations: Vec<Corroboration>,
}

impl Finding {
    /// A finding whose identification nobody has assessed.
    pub fn new(estimand: Estimand, kind: ClaimKind, basis: Evidentiary) -> Self {
        Finding {
            estimand,
            kind,
            basis,
            identification: Identification::NotAssessed,
            corroborations: Vec::new(),
        }
    }

    /// Record what was done about identification.
    pub fn identified_by(
        mut self,
        identification: Identification,
    ) -> Result<Self, EstimandError> {
        identification.validate()?;
        self.identification = identification;
        Ok(self)
    }

    /// Promote a model-conditional finding, naming what corroborated it.
    ///
    /// Refuses when the corroboration is itself model-conditional in the sense that matters: a
    /// simulator corroborating a simulator is not external evidence, so a promotion whose
    /// corroborating source is the same model is refused.
    pub fn promote(&mut self, corroboration: Corroboration) -> Result<(), EstimandError> {
        self.validate()?;
        corroboration.validate()?;
        if corroboration.kind != self.kind {
            return Err(EstimandError::InvalidCorroboration {
                source_id: corroboration.source,
                detail: "claim kind does not match the finding".into(),
            });
        }
        if self.corroborations.len() >= MAX_CORROBORATIONS {
            return Err(EstimandError::InvalidCorroboration {
                source_id: corroboration.source,
                detail: format!("corroborations exceed the {MAX_CORROBORATIONS}-item limit"),
            });
        }
        if self
            .corroborations
            .iter()
            .any(|existing| existing.source == corroboration.source)
        {
            return Err(EstimandError::DuplicateCorroboration {
                source_id: corroboration.source,
            });
        }
        if let Evidentiary::ModelConditional { model } = &self.basis {
            if corroboration.source == *model {
                return Err(EstimandError::NoAutomaticPromotion {
                    target: corroboration.source,
                });
            }
        }
        self.corroborations.push(corroboration);
        Ok(())
    }

    fn validate(&self) -> Result<(), EstimandError> {
        self.estimand.validate()?;
        self.basis.validate()?;
        self.identification.validate()?;
        if self.corroborations.len() > MAX_CORROBORATIONS {
            return Err(EstimandError::InvalidCorroboration {
                source_id: "<collection>".into(),
                detail: format!("corroborations exceed the {MAX_CORROBORATIONS}-item limit"),
            });
        }
        let mut sources = BTreeSet::new();
        for corroboration in &self.corroborations {
            corroboration.validate()?;
            if corroboration.kind != self.kind {
                return Err(EstimandError::InvalidCorroboration {
                    source_id: corroboration.source.clone(),
                    detail: "claim kind does not match the finding".into(),
                });
            }
            if !sources.insert(&corroboration.source) {
                return Err(EstimandError::DuplicateCorroboration {
                    source_id: corroboration.source.clone(),
                });
            }
            if let Evidentiary::ModelConditional { model } = &self.basis {
                if corroboration.source == *model {
                    return Err(EstimandError::NoAutomaticPromotion {
                        target: corroboration.source.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The corroborations recorded, in order.
    pub fn corroborations(&self) -> &[Corroboration] {
        &self.corroborations
    }

    /// Whether this finding still carries the model-conditional qualifier.
    pub fn still_model_conditional(&self) -> bool {
        self.basis.is_model_conditional() && self.corroborations.is_empty()
    }

    /// The sentence this finding licenses, with every qualifier it must carry.
    ///
    /// Rendering rather than a boolean, because the failure this guards against is linguistic:
    /// 26.09's "causal language from predictive feature importance" is a report that says "causes"
    /// where the analysis supports "is associated with". The renderer cannot produce the wrong
    /// verb for the wrong claim kind, because the verb is chosen by the variant.
    pub fn claim_language(&self) -> String {
        let verb = match self.kind {
            ClaimKind::Association => "is associated with",
            ClaimKind::Intervention => "changes",
        };
        let core = format!(
            "in {}, {} versus {} {} {} at {} (unit: {})",
            self.estimand.scope(),
            self.estimand.intervention(),
            self.estimand.comparator(),
            verb,
            self.estimand.outcome(),
            self.estimand.horizon(),
            self.estimand.unit(),
        );
        let mut qualifiers = Vec::new();
        if self.still_model_conditional() {
            if let Evidentiary::ModelConditional { model } = &self.basis {
                qualifiers.push(format!("model-conditional on {model}"));
            }
        }
        if matches!(self.identification, Identification::NotAssessed)
            && self.kind == ClaimKind::Intervention
        {
            qualifiers.push("identification not assessed".to_string());
        }
        if qualifiers.is_empty() {
            core
        } else {
            format!("{core} [{}]", qualifiers.join("; "))
        }
    }
}

fn validate_identification_text(field: &str, value: &str) -> Result<(), EstimandError> {
    if value.trim().is_empty() {
        return Err(EstimandError::InvalidIdentification {
            detail: format!("{field} must not be empty"),
        });
    }
    if value != value.trim() {
        return Err(EstimandError::InvalidIdentification {
            detail: format!("{field} must not have leading or trailing whitespace"),
        });
    }
    if value.len() > MAX_ESTIMAND_TEXT_BYTES {
        return Err(EstimandError::InvalidIdentification {
            detail: format!("{field} exceeds {MAX_ESTIMAND_TEXT_BYTES} bytes"),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(EstimandError::InvalidIdentification {
            detail: format!("{field} contains a control character"),
        });
    }
    Ok(())
}

fn validate_identification_list(
    field: &str,
    values: &[String],
    limit: usize,
) -> Result<(), EstimandError> {
    if values.len() > limit {
        return Err(EstimandError::InvalidIdentification {
            detail: format!("{field} exceed the {limit}-item limit"),
        });
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_identification_text(field, value)?;
        if !unique.insert(value) {
            return Err(EstimandError::InvalidIdentification {
                detail: format!("{field} contain a duplicate `{}`", value),
            });
        }
    }
    Ok(())
}
