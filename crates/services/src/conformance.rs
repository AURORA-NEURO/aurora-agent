//! Holding an implementation against a contract.
//!
//! A [`ServiceReport`] is a factual description of what a piece of code actually offers: the inputs
//! it takes, the outputs it returns, the error classes it can express as types, the effects it has,
//! and which of the contract's numbered invariants it enforces. [`ServiceContract::check`] compares
//! the two and returns every way they differ.
//!
//! # Why a report and not a trait
//!
//! A trait would make conformance a compile-time property of code this crate can see. Most of what
//! §40 asks about is not visible that way — whether a failure mode has a typed representation,
//! whether an invariant is enforced or merely stated, whether an effect happens inside the service
//! or is left to its caller. Those are answered by reading, and the honest way to record a reading
//! is as evidence a reviewer can contradict. Every field of a report is such a reading, and
//! [`ServiceReport::evidence`] is where the citation goes.
//!
//! The reports themselves live in [`crate::implementations`], separately from the checker, so that
//! disagreeing with a verdict means editing a fact rather than editing the logic that consumes it.
//!
//! # Three outcomes, not two
//!
//! [`Conformance::NotImplemented`] is distinct from [`Conformance::Diverges`] for the reason
//! `bioprism-conformance` separates *unsupported* from *failed*: collapsing them punishes an
//! implementation for being honest about its scope, and the repair is completely different. One is
//! a bug list; the other is a ticket.

use crate::contract::{ContractId, Delivery, Effect, Idempotency, ServiceContract};
use crate::error::ErrorClass;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// What an implementation actually offers, as read from its source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceReport {
    pub contract: ContractId,
    /// The workspace crates that between them implement the contract. Empty means nothing does.
    pub crates: Vec<String>,
    /// The function or type that is the operation, named so a reviewer can go and look.
    pub entry_point: String,
    /// Request fields the entry point accepts, by the contract's field names.
    pub accepts: BTreeSet<String>,
    /// Response fields the entry point returns, by the contract's field names.
    pub produces: BTreeSet<String>,
    /// Error classes the implementation can express as a type. A failure mode whose class is
    /// absent here has no typed representation, whatever the prose says.
    pub error_classes: BTreeSet<ErrorClass>,
    pub effects: BTreeSet<Effect>,
    pub delivery: Delivery,
    pub idempotency: Idempotency,
    /// One-based indices into the contract's invariant list, for the invariants the implementation
    /// actually enforces rather than merely satisfies by having no code that could break them.
    pub enforces: BTreeSet<usize>,
    /// Citations and caveats. Prose, and deliberately so: this is the part a reviewer argues with.
    pub evidence: Vec<String>,
}

impl ServiceReport {
    pub fn new(contract: ContractId, entry_point: impl Into<String>) -> Self {
        ServiceReport {
            contract,
            crates: Vec::new(),
            entry_point: entry_point.into(),
            accepts: BTreeSet::new(),
            produces: BTreeSet::new(),
            error_classes: BTreeSet::new(),
            effects: BTreeSet::new(),
            delivery: Delivery::Immediate,
            idempotency: Idempotency::Pure,
            enforces: BTreeSet::new(),
            evidence: Vec::new(),
        }
    }

    pub fn in_crates(mut self, crates: &[&str]) -> Self {
        self.crates = crates.iter().map(|name| (*name).to_string()).collect();
        self
    }

    pub fn accepting(mut self, fields: &[&str]) -> Self {
        self.accepts = fields.iter().map(|name| (*name).to_string()).collect();
        self
    }

    pub fn producing(mut self, fields: &[&str]) -> Self {
        self.produces = fields.iter().map(|name| (*name).to_string()).collect();
        self
    }

    pub fn raising(mut self, classes: &[ErrorClass]) -> Self {
        self.error_classes = classes.iter().copied().collect();
        self
    }

    pub fn touching(mut self, effects: &[Effect]) -> Self {
        self.effects = effects.iter().copied().collect();
        self
    }

    pub fn delivered(mut self, delivery: Delivery, idempotency: Idempotency) -> Self {
        self.delivery = delivery;
        self.idempotency = idempotency;
        self
    }

    pub fn enforcing(mut self, invariants: &[usize]) -> Self {
        self.enforces = invariants.iter().copied().collect();
        self
    }

    pub fn noting(mut self, evidence: &str) -> Self {
        self.evidence.push(evidence.to_string());
        self
    }

    /// Whether anything at all implements the contract.
    pub fn is_absent(&self) -> bool {
        self.crates.is_empty() || self.produces.is_empty()
    }
}

/// The verdict for one contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conformance {
    /// Every declared input is accepted, every declared output produced, every failure mode has a
    /// typed representation, no undeclared effect, every invariant enforced.
    Satisfied,
    /// Implemented, and differs from the contract in at least one named way.
    Diverges,
    /// Nothing in the workspace answers this contract.
    NotImplemented,
}

impl Conformance {
    pub fn as_str(self) -> &'static str {
        match self {
            Conformance::Satisfied => "satisfied",
            Conformance::Diverges => "diverges",
            Conformance::NotImplemented => "not implemented",
        }
    }
}

impl fmt::Display for Conformance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Every way one implementation differs from its contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceCheck {
    pub contract: ContractId,
    /// Required request fields the entry point does not take.
    pub missing_inputs: Vec<String>,
    /// Declared response fields the entry point does not return.
    pub missing_outputs: Vec<String>,
    /// Fields the implementation offers that the contract does not declare. Not a defect on its
    /// own; a contract that has fallen behind its implementation looks exactly like this.
    pub extra_outputs: Vec<String>,
    /// Declared failure modes whose error class the implementation cannot express as a type.
    pub untyped_failures: Vec<String>,
    /// Effects the implementation has and the contract does not permit. The least-authority clause
    /// every §40 module carries is this list being empty.
    pub undeclared_effects: Vec<Effect>,
    /// Effects the contract requires that the implementation does not perform. Often a sign the
    /// contract put the effect in the wrong place rather than that the code is missing something.
    pub unperformed_effects: Vec<Effect>,
    /// Contract invariants the implementation does not enforce, quoted.
    pub unenforced_invariants: Vec<String>,
    /// Contract delivery mode, then the implementation's.
    pub delivery_mismatch: Option<(Delivery, Delivery)>,
    /// Contract idempotency class, then the implementation's.
    pub idempotency_mismatch: Option<(Idempotency, Idempotency)>,
    absent: bool,
}

impl ConformanceCheck {
    pub fn verdict(&self) -> Conformance {
        if self.absent {
            return Conformance::NotImplemented;
        }
        if self.is_clean() {
            Conformance::Satisfied
        } else {
            Conformance::Diverges
        }
    }

    /// Whether the implementation matches the contract in every checked respect.
    ///
    /// `extra_outputs` is not counted: offering more than was promised breaks nobody, and counting
    /// it would make every contract that lags its implementation look like a failure.
    pub fn is_clean(&self) -> bool {
        self.missing_inputs.is_empty()
            && self.missing_outputs.is_empty()
            && self.untyped_failures.is_empty()
            && self.undeclared_effects.is_empty()
            && self.unperformed_effects.is_empty()
            && self.unenforced_invariants.is_empty()
            && self.delivery_mismatch.is_none()
            && self.idempotency_mismatch.is_none()
    }

    /// The number of distinct divergences, for a headline.
    pub fn divergence_count(&self) -> usize {
        self.missing_inputs.len()
            + self.missing_outputs.len()
            + self.untyped_failures.len()
            + self.undeclared_effects.len()
            + self.unperformed_effects.len()
            + self.unenforced_invariants.len()
            + usize::from(self.delivery_mismatch.is_some())
            + usize::from(self.idempotency_mismatch.is_some())
    }

    /// A one-line summary naming the kinds of divergence, for a report table.
    pub fn headline(&self) -> String {
        if self.absent {
            return "nothing in the workspace answers this contract".to_string();
        }
        if self.is_clean() {
            return "satisfied as written".to_string();
        }
        let mut parts = Vec::new();
        if !self.missing_inputs.is_empty() {
            parts.push(format!(
                "{} input(s) not accepted",
                self.missing_inputs.len()
            ));
        }
        if !self.missing_outputs.is_empty() {
            parts.push(format!(
                "{} output(s) not produced",
                self.missing_outputs.len()
            ));
        }
        if !self.untyped_failures.is_empty() {
            parts.push(format!(
                "{} failure mode(s) with no typed error",
                self.untyped_failures.len()
            ));
        }
        if !self.undeclared_effects.is_empty() {
            parts.push(format!(
                "{} undeclared effect(s)",
                self.undeclared_effects.len()
            ));
        }
        if !self.unperformed_effects.is_empty() {
            parts.push(format!(
                "{} declared effect(s) not performed",
                self.unperformed_effects.len()
            ));
        }
        if !self.unenforced_invariants.is_empty() {
            parts.push(format!(
                "{} invariant(s) not enforced",
                self.unenforced_invariants.len()
            ));
        }
        if self.delivery_mismatch.is_some() {
            parts.push("delivery mode differs".to_string());
        }
        if self.idempotency_mismatch.is_some() {
            parts.push("idempotency class differs".to_string());
        }
        parts.join("; ")
    }
}

impl ServiceContract {
    /// Hold an implementation against this contract and name every difference.
    pub fn check(&self, report: &ServiceReport) -> ConformanceCheck {
        let missing_inputs = self
            .required_inputs()
            .into_iter()
            .filter(|path| !report.accepts.contains(*path))
            .map(str::to_string)
            .collect();

        let declared: BTreeSet<&str> = self.declared_outputs().into_iter().collect();
        let missing_outputs = declared
            .iter()
            .filter(|path| !report.produces.contains(**path))
            .map(|path| (*path).to_string())
            .collect();
        let extra_outputs = report
            .produces
            .iter()
            .filter(|path| !declared.contains(path.as_str()))
            .cloned()
            .collect();

        let untyped_failures = self
            .failures
            .iter()
            .filter(|failure| !report.error_classes.contains(&failure.class))
            .map(|failure| failure.label.clone())
            .collect();

        let undeclared_effects = report
            .effects
            .difference(&self.effects)
            .copied()
            .collect::<Vec<Effect>>();
        let unperformed_effects = self
            .effects
            .difference(&report.effects)
            .copied()
            .collect::<Vec<Effect>>();

        let unenforced_invariants = self
            .invariants
            .iter()
            .enumerate()
            .filter(|(index, _)| !report.enforces.contains(&(index + 1)))
            .map(|(_, text)| text.clone())
            .collect();

        let delivery_mismatch =
            (self.delivery != report.delivery).then_some((self.delivery, report.delivery));
        let idempotency_mismatch = (self.idempotency != report.idempotency)
            .then_some((self.idempotency, report.idempotency));

        ConformanceCheck {
            contract: self.id,
            missing_inputs,
            missing_outputs,
            extra_outputs,
            untyped_failures,
            undeclared_effects,
            unperformed_effects,
            unenforced_invariants,
            delivery_mismatch,
            idempotency_mismatch,
            absent: report.is_absent(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog;

    fn perfect(contract: &ServiceContract) -> ServiceReport {
        let inputs: Vec<&str> = contract.required_inputs();
        let outputs: Vec<&str> = contract.declared_outputs();
        let classes: Vec<ErrorClass> = contract.error_classes().into_iter().collect();
        let effects: Vec<Effect> = contract.effects.iter().copied().collect();
        ServiceReport::new(contract.id, "hypothetical")
            .in_crates(&["hypothetical"])
            .accepting(&inputs)
            .producing(&outputs)
            .raising(&classes)
            .touching(&effects)
            .delivered(contract.delivery, contract.idempotency)
            .enforcing(&(1..=contract.invariants.len()).collect::<Vec<_>>())
    }

    #[test]
    fn an_implementation_matching_the_contract_in_every_respect_is_satisfied() {
        for contract in catalog::all() {
            let check = contract.check(&perfect(&contract));
            assert_eq!(
                check.verdict(),
                Conformance::Satisfied,
                "{}: {}",
                contract.id,
                check.headline()
            );
            assert_eq!(check.divergence_count(), 0);
        }
    }

    #[test]
    fn an_implementation_producing_none_of_the_outputs_is_not_implemented_rather_than_diverging() {
        let contract = catalog::context_compiler().expect("transcribes");
        let report = ServiceReport::new(contract.id, "nothing").in_crates(&["nowhere"]);
        assert_eq!(
            contract.check(&report).verdict(),
            Conformance::NotImplemented
        );
    }

    #[test]
    fn a_missing_output_is_named_in_the_check() {
        let contract = catalog::context_compiler().expect("transcribes");
        let mut report = perfect(&contract);
        report.produces.remove("expansion_api");
        let check = contract.check(&report);
        assert_eq!(check.missing_outputs, ["expansion_api"]);
        assert_eq!(check.verdict(), Conformance::Diverges);
    }

    #[test]
    fn a_failure_mode_whose_class_the_implementation_cannot_express_is_untyped() {
        let contract = catalog::context_compiler().expect("transcribes");
        let mut report = perfect(&contract);
        report.error_classes.remove(&ErrorClass::PolicyDenied);
        let check = contract.check(&report);
        assert_eq!(check.untyped_failures, ["policy conflict"]);
    }

    #[test]
    fn an_effect_the_contract_does_not_permit_is_reported_as_undeclared() {
        let contract = catalog::adaptive_scheduler().expect("transcribes");
        let mut report = perfect(&contract);
        report.effects.insert(Effect::WritesArtifactStore);
        let check = contract.check(&report);
        assert_eq!(check.undeclared_effects, [Effect::WritesArtifactStore]);
    }

    #[test]
    fn an_effect_the_contract_requires_and_the_code_leaves_to_its_caller_is_reported_separately() {
        let contract = catalog::context_compiler().expect("transcribes");
        let mut report = perfect(&contract);
        report.effects.remove(&Effect::WritesArtifactStore);
        let check = contract.check(&report);
        assert_eq!(check.unperformed_effects, [Effect::WritesArtifactStore]);
        assert!(check.undeclared_effects.is_empty());
    }

    #[test]
    fn producing_more_than_the_contract_declares_is_not_a_divergence() {
        let contract = catalog::registry_backend().expect("transcribes");
        let mut report = perfect(&contract);
        report.produces.insert("download_count".to_string());
        let check = contract.check(&report);
        assert_eq!(check.extra_outputs, ["download_count"]);
        assert_eq!(check.verdict(), Conformance::Satisfied);
    }

    #[test]
    fn an_unenforced_invariant_is_quoted_rather_than_numbered() {
        let contract = catalog::world_builder().expect("transcribes");
        let mut report = perfect(&contract);
        report.enforces.remove(&2);
        let check = contract.check(&report);
        assert_eq!(check.unenforced_invariants, ["World release is immutable."]);
    }

    #[test]
    fn a_headline_names_every_kind_of_divergence_present() {
        let contract = catalog::world_builder().expect("transcribes");
        let mut report = perfect(&contract);
        report.produces.remove("world_card");
        report.enforces.remove(&2);
        report.delivery = Delivery::Immediate;
        let headline = contract.check(&report).headline();
        assert!(headline.contains("1 output(s) not produced"), "{headline}");
        assert!(
            headline.contains("1 invariant(s) not enforced"),
            "{headline}"
        );
        assert!(headline.contains("delivery mode differs"), "{headline}");
    }

    #[test]
    fn a_report_round_trips_through_json() {
        let contract = catalog::mutation_runtime().expect("transcribes");
        let report = perfect(&contract).noting("evidence");
        let value = serde_json::to_value(&report).expect("serialises");
        let back: ServiceReport = serde_json::from_value(value).expect("deserialises");
        assert_eq!(back, report);
    }
}
