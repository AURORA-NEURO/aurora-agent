//! Declared transports: the shared machinery behind 30.08 and 30.19.
//!
//! Both cross-modal prediction (30.08) and patient-derived models (30.19) are the same move in
//! different clothes: evidence gathered in one scope is asserted about another. `bioprism_scope`
//! already names that move [`MappingKind::Transport`] and already rejects a transport that claims
//! to have lost nothing; `bioprism_standards` already renders a genome-build lift as one. This
//! module is the neuro-oncology instance, and it does not build a second mechanism.
//!
//! A [`DeclaredTransport`] is a [`ScopeMapping`] plus the assumptions the destination scope needs.
//! Two things make it refuse:
//!
//! * an **empty loss ledger**, which `bioprism_scope::ScopeMapping::check` reports as
//!   [`bioprism_scope::MappingCheck::UndeclaredLoss`] — a cross-scope move that discarded nothing
//!   and added no uncertainty is a modelling error, not a clean result;
//! * a **missing assumption**, where the caller's declared assumption names do not cover what the
//!   consumer requires.
//!
//! Assumptions are `(name, statement)` pairs rather than an enum. The names are fixed per consumer
//! — [`crate::radiogenomics::REQUIRED_ASSUMPTIONS`],
//! [`crate::models::REQUIRED_ASSUMPTIONS`] — but the statements are the caller's own words,
//! because an assumption a reader cannot disagree with in prose is not an assumption.
//!
//! # Not implemented
//!
//! Nothing here evaluates whether an assumption is *true*. That is what the oracle mesh and the
//! five-layer review in the blueprint are for. This module makes an undeclared assumption
//! impossible to leave undeclared, which is a strictly weaker and strictly checkable thing.

use crate::error::TransportRefusal;
use bioprism_scope::{LossLedger, MappingCheck, MappingKind, ScopeKey, ScopeMapping};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A stated move of evidence from one scope to another.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredTransport {
    pub from: ScopeKey,
    pub to: ScopeKey,
    pub justification: String,
    pub loss: LossLedger,
    assumptions: BTreeMap<String, String>,
}

impl DeclaredTransport {
    pub fn new(from: ScopeKey, to: ScopeKey, justification: impl Into<String>) -> Self {
        DeclaredTransport {
            from,
            to,
            justification: justification.into(),
            loss: LossLedger::default(),
            assumptions: BTreeMap::new(),
        }
    }

    pub fn losing(mut self, what: impl Into<String>) -> Self {
        self.loss = self.loss.discarding(what);
        self
    }

    pub fn adding_uncertainty(mut self, what: impl Into<String>) -> Self {
        self.loss = self.loss.adding_uncertainty(what);
        self
    }

    /// States an assumption by name, with the caller's own words for what it says.
    pub fn assuming(mut self, name: impl Into<String>, statement: impl Into<String>) -> Self {
        self.assumptions.insert(name.into(), statement.into());
        self
    }

    pub fn assumption(&self, name: &str) -> Option<&str> {
        self.assumptions.get(name).map(String::as_str)
    }

    pub fn assumption_names(&self) -> impl Iterator<Item = &str> {
        self.assumptions.keys().map(String::as_str)
    }

    /// The same move, as a `bioprism_scope` mapping.
    pub fn to_scope_mapping(&self) -> ScopeMapping {
        ScopeMapping {
            from: self.from.clone(),
            to: self.to.clone(),
            kind: MappingKind::Transport {
                justification: self.justification.clone(),
            },
            loss: self.loss.clone(),
        }
    }

    /// Whether this transport is declared well enough to carry a claim needing `required`.
    ///
    /// The loss ledger is checked first: a transport with nothing declared has not yet said
    /// anything about itself, so complaining about a specific missing assumption would understate
    /// the problem.
    pub fn check(&self, required: &[&str]) -> Result<(), TransportRefusal> {
        if matches!(
            self.to_scope_mapping().check(),
            MappingCheck::UndeclaredLoss
        ) {
            return Err(TransportRefusal::UndeclaredLoss {
                from: describe(&self.from),
                to: describe(&self.to),
            });
        }
        for name in required {
            if !self.assumptions.contains_key(*name) {
                return Err(TransportRefusal::UnstatedAssumption {
                    assumption: (*name).to_string(),
                });
            }
        }
        Ok(())
    }
}

fn describe(key: &ScopeKey) -> String {
    let parts: Vec<String> = key
        .iter()
        .map(|(dimension, value)| format!("{dimension}={}", value.describe()))
        .collect();
    if parts.is_empty() {
        "<unscoped>".to_string()
    } else {
        parts.join(",")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes() -> (ScopeKey, ScopeKey) {
        (
            ScopeKey::new().exact("specimen", "S1"),
            ScopeKey::new().exact("patient", "PT-1"),
        )
    }

    #[test]
    fn a_transport_that_declares_no_loss_is_refused() {
        let (from, to) = scopes();
        let transport = DeclaredTransport::new(from, to, "the specimen represents the tumour");
        let refusal = transport.check(&[]).unwrap_err();
        assert!(matches!(refusal, TransportRefusal::UndeclaredLoss { .. }));
    }

    #[test]
    fn a_transport_missing_a_required_assumption_names_the_assumption() {
        let (from, to) = scopes();
        let transport = DeclaredTransport::new(from, to, "justification")
            .losing("regional variation outside the sampled fragment");
        let refusal = transport.check(&["paired timing"]).unwrap_err();
        assert_eq!(
            refusal,
            TransportRefusal::UnstatedAssumption {
                assumption: "paired timing".to_string()
            }
        );
    }

    #[test]
    fn a_declared_transport_renders_as_a_real_scope_mapping() {
        let (from, to) = scopes();
        let transport = DeclaredTransport::new(from, to, "justification")
            .adding_uncertainty("sampling variance across regions")
            .assuming("paired timing", "both artefacts predate the first resection");
        assert_eq!(transport.to_scope_mapping().check(), MappingCheck::Sound);
        assert!(transport.check(&["paired timing"]).is_ok());
    }

    #[test]
    fn the_loss_ledger_is_checked_before_individual_assumptions() {
        let (from, to) = scopes();
        let transport = DeclaredTransport::new(from, to, "justification")
            .assuming("paired timing", "stated");
        assert!(matches!(
            transport.check(&["paired timing"]).unwrap_err(),
            TransportRefusal::UndeclaredLoss { .. }
        ));
    }
}
