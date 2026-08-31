//! Certifying an adapter against a source.
//!
//! Blueprint 40.32 makes conformance the thing an adapter advertises rather than the thing its
//! author asserts, and 40.17's verification plan asks specifically for "semantic-loss
//! assertions" alongside golden fixtures. This module is that suite, and it is the most
//! valuable code in the crate, because everything else here is a promise and this is the part
//! that checks one.
//!
//! Two checks carry the weight.
//!
//! **Determinism.** The adapter is run repeatedly on the same source and the canonical bytes
//! of the whole ingestion are compared — facts, manifest and loss report together. An adapter
//! that emits stable facts and a fluctuating loss report is not deterministic in any sense a
//! reviewer cares about, because two reviewers would reach different conclusions about the
//! same world.
//!
//! **Loss completeness.** Every field the source contains must be either mapped or declared
//! lost. The field list comes from [`crate::probe`], which reads the source without any
//! mapping policy and without consulting the adapter; that independence is the whole point.
//! If the adapter supplied both the inventory and the coverage claim, the check would collapse
//! into "the adapter agrees with itself", which the deliberately-broken adapter in this
//! crate's tests passes trivially and a correct adapter passes no more convincingly.
//!
//! What conformance cannot check is whether the mapping is *right* — whether `dose` really
//! means milligrams. Nothing mechanical can. It checks that every decision was declared, which
//! is what turns a wrong mapping into something a reviewer can find.

use crate::adapter::Adapter;
use crate::error::AdapterError;
use crate::ingestion::Ingestion;
use crate::location::{LocationSet, SourceLocation};
use crate::loss::LossKind;
use crate::probe::{field_inventory, Inventory};
use crate::source::Source;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt;

/// How many times [`certify`] repeats the ingest when checking determinism.
///
/// Three rather than two: a single repeat catches an adapter that varies on every run, but not
/// one that alternates between two outputs, which is what a `HashMap` iteration order or a
/// two-slot cache produces.
pub const DETERMINISM_TRIALS: usize = 3;

/// The properties an adapter is certified against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// The same source yields byte-identical ingestions.
    Determinism,
    /// The adapter's self-description does not depend on the source it was handed.
    ManifestStability,
    /// A loss audit was performed at all, rather than declared unaudited.
    LossAuditPerformed,
    /// Every field the source contains is either mapped or declared lost.
    LossCompleteness,
    /// The adapter emitted no loss kind its manifest failed to declare.
    DeclaredLossKinds,
    /// Every emitted fact is scoped and traceable.
    FactIntegrity,
}

impl Check {
    pub fn as_str(&self) -> &'static str {
        match self {
            Check::Determinism => "determinism",
            Check::ManifestStability => "manifest_stability",
            Check::LossAuditPerformed => "loss_audit_performed",
            Check::LossCompleteness => "loss_completeness",
            Check::DeclaredLossKinds => "declared_loss_kinds",
            Check::FactIntegrity => "fact_integrity",
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The outcome of one check.
///
/// `NotApplicable` is separate from `Pass` for the same reason `Unaudited` is separate from
/// `Lossless`: a check that could not run has not been passed, and collapsing the two would
/// let an adapter earn a clean report by being unverifiable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pass,
    Fail,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckOutcome {
    pub check: Check,
    pub status: Status,
    pub detail: String,
}

impl CheckOutcome {
    fn pass(check: Check, detail: impl Into<String>) -> Self {
        CheckOutcome {
            check,
            status: Status::Pass,
            detail: detail.into(),
        }
    }

    fn fail(check: Check, detail: impl Into<String>) -> Self {
        CheckOutcome {
            check,
            status: Status::Fail,
            detail: detail.into(),
        }
    }

    fn skip(check: Check, detail: impl Into<String>) -> Self {
        CheckOutcome {
            check,
            status: Status::NotApplicable,
            detail: detail.into(),
        }
    }
}

/// The result of certifying one adapter against one source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceReport {
    pub adapter: String,
    pub adapter_version: String,
    pub source_id: String,
    pub checks: Vec<CheckOutcome>,
}

impl ConformanceReport {
    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.adapter.trim().is_empty()
            || self.adapter_version.trim().is_empty()
            || self.source_id.trim().is_empty()
        {
            return Err(AdapterError::Conformance(
                "adapter, version, and source identity are required".into(),
            ));
        }
        let expected_checks = [
            Check::Determinism,
            Check::ManifestStability,
            Check::LossAuditPerformed,
            Check::LossCompleteness,
            Check::DeclaredLossKinds,
            Check::FactIntegrity,
        ];
        if self.checks.len() != expected_checks.len() {
            return Err(AdapterError::Conformance(format!(
                "expected {} checks, found {}",
                expected_checks.len(),
                self.checks.len()
            )));
        }
        for (outcome, expected) in self.checks.iter().zip(expected_checks) {
            if outcome.check != expected {
                return Err(AdapterError::Conformance(
                    "checks are missing, duplicated, or out of canonical order".into(),
                ));
            }
            if outcome.detail.trim().is_empty() || outcome.detail.chars().any(char::is_control) {
                return Err(AdapterError::Conformance(format!(
                    "{} check has no bounded detail",
                    outcome.check
                )));
            }
        }
        Ok(())
    }

    /// True when no check failed. A report of nothing but `NotApplicable` also passes, which is
    /// why [`ConformanceReport::verified`] exists alongside it.
    pub fn passed(&self) -> bool {
        self.failures().next().is_none()
    }

    /// True when nothing failed *and* something was actually verified.
    pub fn verified(&self) -> bool {
        self.passed()
            && self
                .checks
                .iter()
                .any(|outcome| outcome.status == Status::Pass)
    }

    pub fn failures(&self) -> impl Iterator<Item = &CheckOutcome> {
        self.checks
            .iter()
            .filter(|outcome| outcome.status == Status::Fail)
    }

    pub fn status_of(&self, check: Check) -> Option<Status> {
        self.checks
            .iter()
            .find(|outcome| outcome.check == check)
            .map(|outcome| outcome.status)
    }

    /// A single-line summary for a CLI or a CI log.
    pub fn summary(&self) -> String {
        let failed = self.failures().count();
        format!(
            "{} {} on {}: {} checks, {} failed",
            self.adapter,
            self.adapter_version,
            self.source_id,
            self.checks.len(),
            failed
        )
    }
}

/// Runs the full suite. The ingestion from the first trial is returned alongside the report so
/// that a caller which certifies before publishing does not have to ingest a fifth time.
pub fn certify<A: Adapter + ?Sized>(
    adapter: &A,
    source: &Source,
) -> Result<(ConformanceReport, Ingestion), AdapterError> {
    let manifest = adapter.manifest();
    manifest.validate()?;
    if manifest.name != adapter.name() {
        return Err(AdapterError::Conformance(
            "adapter manifest name does not match the adapter identity".into(),
        ));
    }
    let first = adapter.ingest(source)?;
    let baseline = first.canonical_bytes()?;

    let checks = vec![
        check_determinism(adapter, source, &baseline)?,
        check_manifest_stability(adapter, &manifest),
        check_audit_performed(&first),
        check_loss_completeness(source, &first)?,
        check_declared_loss_kinds(&manifest.declared_loss_kinds, &first),
        check_fact_integrity(&first),
    ];

    let report = ConformanceReport {
        adapter: manifest.name,
        adapter_version: manifest.version,
        source_id: source.id.clone(),
        checks,
    };
    report.validate()?;
    Ok((report, first))
}

fn check_determinism<A: Adapter + ?Sized>(
    adapter: &A,
    source: &Source,
    baseline: &[u8],
) -> Result<CheckOutcome, AdapterError> {
    for trial in 1..DETERMINISM_TRIALS {
        let repeat = adapter.ingest(source)?;
        let bytes = repeat.canonical_bytes()?;
        if bytes != baseline {
            return Ok(CheckOutcome::fail(
                Check::Determinism,
                format!(
                    "trial {trial} produced {} canonical bytes against the first trial's {}; \
                     the first divergence is at byte {}",
                    bytes.len(),
                    baseline.len(),
                    first_divergence(baseline, &bytes)
                ),
            ));
        }
    }
    Ok(CheckOutcome::pass(
        Check::Determinism,
        format!("{DETERMINISM_TRIALS} trials produced identical canonical bytes"),
    ))
}

fn check_manifest_stability<A: Adapter + ?Sized>(
    adapter: &A,
    baseline: &crate::adapter::AdapterManifest,
) -> CheckOutcome {
    if &adapter.manifest() == baseline {
        CheckOutcome::pass(
            Check::ManifestStability,
            "the manifest is identical across calls",
        )
    } else {
        CheckOutcome::fail(
            Check::ManifestStability,
            "the manifest changed between calls, so it describes an ingest rather than an adapter",
        )
    }
}

fn check_audit_performed(ingestion: &Ingestion) -> CheckOutcome {
    match ingestion.loss() {
        crate::loss::SemanticLoss::Unaudited { reason } => CheckOutcome::fail(
            Check::LossAuditPerformed,
            format!("the adapter returned facts without auditing loss: {reason}"),
        ),
        _ => CheckOutcome::pass(
            Check::LossAuditPerformed,
            "the adapter performed a loss audit",
        ),
    }
}

/// The check the whole crate is built around.
///
/// Accounting is by *exact* field identity, in both directions, and both strictnesses matter.
///
/// A claim coarser than a field does not discharge it. Otherwise a single source-wide entry —
/// every adapter here emits one, [`LossKind::ProvenanceUnavailable`] — would discharge every
/// column in the file at once, and the check would pass for an adapter that read one column
/// and ignored the rest. A loss about the source is not a loss about a field.
///
/// A claim finer than a field does not discharge it either. "Row 4 of this column lost
/// precision" says nothing about rows 1 to 3, and an adapter that drops a column has to say so
/// about the column.
fn check_loss_completeness(
    source: &Source,
    ingestion: &Ingestion,
) -> Result<CheckOutcome, AdapterError> {
    let declared = match field_inventory(source)? {
        Inventory::Unverifiable { reason } => {
            return Ok(CheckOutcome::skip(
                Check::LossCompleteness,
                format!("the source cannot be probed independently: {reason}"),
            ))
        }
        Inventory::Known(locations) => locations,
    };

    let mapped = ingestion.loss().mapped();
    let lost: LocationSet = ingestion
        .loss()
        .entries()
        .iter()
        .map(|entry| entry.location.clone())
        .collect();

    let unaccounted: Vec<&SourceLocation> = declared
        .iter()
        .filter(|field| !mapped.contains(*field) && !lost.contains(*field))
        .collect();
    if !unaccounted.is_empty() {
        return Ok(CheckOutcome::fail(
            Check::LossCompleteness,
            format!(
                "{} source field(s) were neither mapped nor declared lost: {}",
                unaccounted.len(),
                render(unaccounted.into_iter())
            ),
        ));
    }

    let fabricated: Vec<&SourceLocation> = mapped
        .iter()
        .filter(|claimed| !declared.contains(*claimed))
        .collect();
    if !fabricated.is_empty() {
        return Ok(CheckOutcome::fail(
            Check::LossCompleteness,
            format!(
                "the adapter claims to have mapped {} field(s) the source does not contain: {}",
                fabricated.len(),
                render(fabricated.into_iter())
            ),
        ));
    }

    Ok(CheckOutcome::pass(
        Check::LossCompleteness,
        format!(
            "all {} source field(s) are accounted for: {} mapped, {} loss entries",
            declared.len(),
            mapped.len(),
            ingestion.loss().entries().len()
        ),
    ))
}

fn check_declared_loss_kinds(declared: &BTreeSet<LossKind>, ingestion: &Ingestion) -> CheckOutcome {
    let emitted = ingestion.loss().kinds();
    let undeclared: Vec<LossKind> = emitted
        .into_iter()
        .filter(|kind| !declared.contains(kind))
        .collect();
    if undeclared.is_empty() {
        CheckOutcome::pass(
            Check::DeclaredLossKinds,
            "every emitted loss kind appears in the adapter manifest",
        )
    } else {
        let names: Vec<&str> = undeclared.iter().map(LossKind::as_str).collect();
        CheckOutcome::fail(
            Check::DeclaredLossKinds,
            format!(
                "the adapter emitted loss kinds its manifest does not declare: {}",
                names.join(", ")
            ),
        )
    }
}

/// Every fact must be scoped and traceable.
///
/// An empty scope is a claim of unconditional validity, which 43.04 says no local section
/// makes; empty provenance is a fact nobody can trace back to a byte. Both are cheap to check
/// and both make a world unreviewable if they slip through.
fn check_fact_integrity(ingestion: &Ingestion) -> CheckOutcome {
    let mut offenders = Vec::new();
    for fact in ingestion.facts() {
        let id = fact
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or("<no id>")
            .to_string();
        let unscoped = fact
            .get("scope")
            .and_then(Value::as_object)
            .is_none_or(serde_json::Map::is_empty);
        let untraceable = fact
            .get("provenance")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty);
        if unscoped {
            offenders.push(format!("{id} has an empty scope"));
        }
        if untraceable {
            offenders.push(format!("{id} has no provenance"));
        }
    }

    if offenders.is_empty() {
        CheckOutcome::pass(
            Check::FactIntegrity,
            format!(
                "all {} emitted fact(s) are scoped and traceable",
                ingestion.fact_count()
            ),
        )
    } else {
        CheckOutcome::fail(Check::FactIntegrity, offenders.join("; "))
    }
}

fn render<'a, I: Iterator<Item = &'a SourceLocation>>(locations: I) -> String {
    let rendered: Vec<String> = locations.map(SourceLocation::locator).collect();
    rendered.join(", ")
}

fn first_divergence(left: &[u8], right: &[u8]) -> usize {
    left.iter()
        .zip(right.iter())
        .position(|(a, b)| a != b)
        .unwrap_or_else(|| left.len().min(right.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_report_of_only_skipped_checks_passes_but_is_not_verified() {
        let report = ConformanceReport {
            adapter: "t".into(),
            adapter_version: "0.1.0".into(),
            source_id: "s".into(),
            checks: vec![CheckOutcome::skip(Check::LossCompleteness, "unprobeable")],
        };
        assert!(report.passed());
        assert!(!report.verified());
        assert!(report.validate().is_err());
    }

    #[test]
    fn the_first_divergence_offset_points_at_the_differing_byte() {
        assert_eq!(first_divergence(b"abcd", b"abXd"), 2);
        assert_eq!(first_divergence(b"abc", b"abcd"), 3);
    }

    #[test]
    fn a_complete_report_requires_canonical_check_order() {
        let checks = [
            Check::Determinism,
            Check::ManifestStability,
            Check::LossAuditPerformed,
            Check::LossCompleteness,
            Check::DeclaredLossKinds,
            Check::FactIntegrity,
        ]
        .into_iter()
        .map(|check| CheckOutcome::pass(check, "verified"))
        .collect::<Vec<_>>();
        let report = ConformanceReport {
            adapter: "adapter".into(),
            adapter_version: "0.1.0".into(),
            source_id: "source".into(),
            checks,
        };
        report.validate().unwrap();
    }

    #[test]
    fn a_report_with_a_repeated_check_is_rejected() {
        let mut checks = [
            Check::Determinism,
            Check::ManifestStability,
            Check::LossAuditPerformed,
            Check::LossCompleteness,
            Check::DeclaredLossKinds,
            Check::FactIntegrity,
        ]
        .into_iter()
        .map(|check| CheckOutcome::pass(check, "verified"))
        .collect::<Vec<_>>();
        checks[5].check = Check::Determinism;
        let report = ConformanceReport {
            adapter: "adapter".into(),
            adapter_version: "0.1.0".into(),
            source_id: "source".into(),
            checks,
        };
        assert!(report.validate().is_err());
    }
}
