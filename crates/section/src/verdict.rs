//! Oracle verdicts and leakage witnesses.
//!
//! Blueprint 43.41 specifies the split-integrity oracle as returning a set of *witnesses* plus a
//! status in `{valid, invalid, underdetermined}`. A witness is a concrete, checkable object —
//! "alias ALT-77 appears in subjects S001 and S003 which land in different splits" — not a
//! score. That is what makes the first vertical slice objective.
//!
//! The `fiber-*/0.1` reference emits only `valid` and `invalid`;
//! [`OracleStatus::Underdetermined`] exists because 43.28 requires abstention to be
//! representable, and is reported through [`crate::certificate::CertificateProfile::Extended`].

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleStatus {
    Valid,
    Invalid,
    Underdetermined,
}

impl OracleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            OracleStatus::Valid => "valid",
            OracleStatus::Invalid => "invalid",
            OracleStatus::Underdetermined => "underdetermined",
        }
    }
}

/// A concrete, checkable violation found by an oracle.
///
/// The name predates the open variant: the four reference mechanisms are all *leakage*
/// witnesses, and their wire bytes are pinned by the CPython parity contract, so the enum keeps
/// the name they gave it. [`LeakageWitness::DomainCheck`] carries any other domain's declared
/// mechanism; it is additive, so the four reference variants serialise exactly as before.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LeakageWitness {
    /// One alias resolves to several subjects that were assigned to different splits.
    IdentityLeakage {
        alias: String,
        subjects: Vec<String>,
        splits: Vec<String>,
    },
    /// Each split draws from exactly one site, and the sites differ, so site is confounded
    /// with the split and any external-generalisation claim is unsupported.
    SiteLeakage {
        site_by_split: BTreeMap<String, Vec<String>>,
    },
    /// A label was derived from evidence that did not exist at the training decision time.
    TemporalLeakage {
        decision_time: String,
        future_label_sources: BTreeMap<String, String>,
    },
    /// Preprocessing was fit across subjects before the split was drawn.
    PreprocessingLeakage { detail: String },
    /// A domain-declared check fired.
    ///
    /// `check` names the declared rule, `observed` carries the variable bindings the rule read,
    /// each rendered as a canonical JSON string so a human can re-run the check by hand, and
    /// `detail` is the declared sentence naming the mechanism. This is still a witness in the
    /// 43.41 sense — a checkable object, never a score.
    DomainCheck {
        check: String,
        observed: BTreeMap<String, String>,
        detail: String,
    },
}

impl LeakageWitness {
    pub fn kind(&self) -> &'static str {
        match self {
            LeakageWitness::IdentityLeakage { .. } => "identity_leakage",
            LeakageWitness::SiteLeakage { .. } => "site_leakage",
            LeakageWitness::TemporalLeakage { .. } => "temporal_leakage",
            LeakageWitness::PreprocessingLeakage { .. } => "preprocessing_leakage",
            LeakageWitness::DomainCheck { .. } => "domain_check",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleVerdict {
    pub status: OracleStatus,
    pub witnesses: Vec<LeakageWitness>,
    pub oracle_kind: String,
}

impl OracleVerdict {
    pub fn new(oracle_kind: impl Into<String>, witnesses: Vec<LeakageWitness>) -> Self {
        let status = if witnesses.is_empty() {
            OracleStatus::Valid
        } else {
            OracleStatus::Invalid
        };
        OracleVerdict {
            status,
            witnesses,
            oracle_kind: oracle_kind.into(),
        }
    }

    pub fn abstain(oracle_kind: impl Into<String>, witnesses: Vec<LeakageWitness>) -> Self {
        OracleVerdict {
            status: OracleStatus::Underdetermined,
            witnesses,
            oracle_kind: oracle_kind.into(),
        }
    }

    pub fn witness_kinds(&self) -> Vec<&'static str> {
        self.witnesses.iter().map(LeakageWitness::kind).collect()
    }
}
