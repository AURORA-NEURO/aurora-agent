//! The diagnostic catalogue.
//!
//! Twenty-two entries, one per [`DiagnosticCode`] this crate can emit, covering all nine classes
//! of [`crate::taxonomy`]. The catalogue is the fixture [`crate::lint`](mod@crate::lint) grades,
//! the lookup table a consumer resolves a code against, and the exemplar a contributor copies when
//! adding a diagnostic.
//!
//! # Why a catalogue rather than errors at the raise site
//!
//! 11.03 requires documented exit codes, and 40.36 requires typed failure events with a stable
//! module identity. Both want a *closed list* that exists independently of the code paths that
//! raise entries from it: a consumer pins `DEVX-0014`, and a refactor that moves the raise site
//! must not silently renumber it. `bioprism-examples` takes the same shape for property claims,
//! and for the same reason — a list of everything the system can say about itself is auditable in
//! a way that scattered `format!` calls are not.
//!
//! # The lint result on this crate's own catalogue
//!
//! [`crate::lint::lint_catalogue`] reports **zero errors and two warnings**, both
//! [`RemedyIsNotAnInstruction`](crate::lint::LintRule::RemedyIsNotAnInstruction), on `DEVX-0001`
//! and `DEVX-0014`. Both are false positives of the heuristic: those two remedies open with
//! "either" because they genuinely offer a choice between two surfaces, and rewriting them to
//! satisfy a substring match would make them worse sentences for the reader they exist for. They
//! are left in, and the exact set is asserted in a test so the count cannot drift unnoticed.
//!
//! That is the honest result, not a clean one. The rule stays at warning severity precisely
//! because its author's own catalogue trips it twice.
//!
//! # Not implemented
//!
//! No localisation, no message templating and no severity field. A diagnostic's urgency is a
//! property of the [`DiagnosticClass`] and of `human_decision_required`, and a second severity
//! axis would immediately disagree with the first.

use crate::diagnostic::{
    Certainty, Diagnostic, DiagnosticCode, Discrepancy, LineSpan, Remedy, Site,
};
use crate::error::CatalogueError;
use crate::introspect::CompileRecord;
use crate::taxonomy::{ChangeRequired, DiagnosticClass};
use bioprism_sdk::{ManifestError, NegotiationError};
use bioprism_section::InfluenceClass;
use std::collections::BTreeMap;

fn code(text: &str) -> DiagnosticCode {
    DiagnosticCode::parse(text).expect("catalogue codes are well-formed")
}

fn source(document: &str) -> Site {
    Site::Source {
        document: document.to_string(),
        span: None,
    }
}

/// Every diagnostic this crate can emit.
///
/// Ordered by code. The bodies are exemplars: real values are substituted at the raise site, and
/// the shape — which fields are populated, how the remedy is phrased — is the contract.
pub fn catalogue() -> Vec<Diagnostic> {
    vec![
        Diagnostic::new(
            code("DEVX-0001"),
            DiagnosticClass::ContractViolation,
            "every fact in the protected closure is present in the compiled selection",
            "3 protected facts were withheld to fit a budget of 12 facts",
            Site::Artifact {
                node_kind: "query".into(),
                id: "fiber-query/0.1".into(),
            },
        )
        .with_discrepancy(Discrepancy::new("15 facts", "12 facts"))
        .with_context("protected_tags", "split_assignment, training_decision_time")
        .with_remedy(Remedy::new(
            "either raise budget_facts to at least 15 or drop a protected tag from the query",
            source("query.json"),
            "the compile trace carries an empty dropped_protected list",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("43.13")
        .citing("43.25"),
        Diagnostic::new(
            code("DEVX-0002"),
            DiagnosticClass::ContractViolation,
            "a context is presented as sufficient only when every omission group is zero-influence or explicitly bounded",
            "the manifest carries 3 facts in an unknown-influence group and the context is labelled sufficient",
            Site::Artifact {
                node_kind: "omission_manifest".into(),
                id: "group:not-analysed".into(),
            },
        )
        .with_context("influence", InfluenceClass::Unknown.as_str())
        .with_remedy(Remedy::new(
            "run the influence analysis over the unknown group, or withdraw the sufficiency label",
            Site::Artifact {
                node_kind: "pass".into(),
                id: "influence_analysis".into(),
            },
            "OmissionManifest::supports_sufficiency_claim returns true",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("43.26"),
        Diagnostic::new(
            code("DEVX-0003"),
            DiagnosticClass::InvalidInput,
            "every protected tag names at least one fact in the world",
            "the tag `preprocess_fit_scope` matched no fact and was reported as a satisfied empty closure",
            source("query.json"),
        )
        .with_remedy(Remedy::new(
            "rename the tag to one the world carries, or remove it from protected_tags",
            source("query.json"),
            "the compile trace lists no unmatched_protected_tags",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("43.13"),
        Diagnostic::new(
            code("DEVX-0004"),
            DiagnosticClass::Stale,
            "a certificate's embedded digest equals the digest recomputed over its body",
            "the recomputed digest differs from certificate_sha256, so the document was edited or was produced by a different canonical encoding",
            Site::Digest {
                digest: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            },
        )
        .with_remedy(Remedy::new(
            "re-read the certificate from its source and re-verify",
            Site::Digest {
                digest: "0000000000000000000000000000000000000000000000000000000000000000".into(),
            },
            "ContextCertificate::verify returns Valid",
            ChangeRequired::Precondition,
            Certainty::Observed,
        ))
        .citing("43.26")
        .citing("40.05"),
        Diagnostic::new(
            code("DEVX-0005"),
            DiagnosticClass::Internal,
            "a pass that did not run names what is missing",
            "the pass `obstruction_tests` is listed as deferred with an empty reason",
            Site::Artifact {
                node_kind: "pass".into(),
                id: "obstruction_tests".into(),
            },
        )
        .with_remedy(Remedy::new(
            "report this with the compile record attached; a deferred pass with no reason is a defect in the compiler, not in the input",
            source("crates/fiber/src"),
            "the deferred-pass list carries a reason for every entry",
            ChangeRequired::Environment,
            Certainty::Observed,
        ))
        .citing("43.16")
        .citing("43.37"),
        Diagnostic::new(
            code("DEVX-0006"),
            DiagnosticClass::ContractViolation,
            "a host and a plugin share at least one schema version before the plugin is used",
            "the host and the plugin declare disjoint schema version sets",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_discrepancy(Discrepancy::new("<host versions>", "<plugin versions>"))
        .with_remedy(Remedy::new(
            "pin the plugin to a version on a line the host speaks, or widen the host's declared version set",
            source("plugin manifest"),
            "bioprism_sdk::negotiate returns a Negotiated value",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("40.16")
        .citing("43.35"),
        Diagnostic::new(
            code("DEVX-0007"),
            DiagnosticClass::InvalidInput,
            "a schema version parses as family/major.minor[-qualifier]",
            "the declared version text does not parse",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "replace the version text with one in family/major.minor[-qualifier] form",
            source("plugin manifest"),
            "SchemaVersion::parse accepts it",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16"),
        Diagnostic::new(
            code("DEVX-0008"),
            DiagnosticClass::ContractViolation,
            "a host declares at least one supported schema version before negotiating",
            "the host declares no schema versions, so nothing can be negotiated with any plugin",
            Site::Artifact {
                node_kind: "host".into(),
                id: "version_set".into(),
            },
        )
        .with_remedy(Remedy::new(
            "declare the host's supported schema versions before offering any plugin",
            source("host configuration"),
            "the host's VersionSet is non-empty",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("40.16"),
        Diagnostic::new(
            code("DEVX-0009"),
            DiagnosticClass::InvalidInput,
            "a plugin declares the schema versions it speaks",
            "the plugin declares no schema versions",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "add a schema_versions entry to the plugin manifest",
            source("plugin manifest"),
            "the plugin's VersionSet is non-empty",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16"),
        Diagnostic::new(
            code("DEVX-0010"),
            DiagnosticClass::InvalidInput,
            "an adapter capability carries a semantic-loss declaration",
            "the plugin provides an adapter capability and declares nothing about what it drops",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_semantic_loss("provenance_unavailable")
        .with_remedy(Remedy::new(
            "declare the loss kinds the adapter drops, or declare it lossless and mean it",
            source("plugin manifest"),
            "PluginManifest::validate accepts the manifest",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16")
        .citing("11.14"),
        Diagnostic::new(
            code("DEVX-0011"),
            DiagnosticClass::InvalidInput,
            "a manifest does not claim lossless ingestion while declaring loss kinds",
            "the plugin claims lossless ingestion and declares loss kinds; one of the two statements is wrong",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_semantic_loss("unmapped_column")
        .with_remedy(Remedy::new(
            "remove the lossless claim, or remove the declared loss kinds",
            source("plugin manifest"),
            "PluginManifest::validate accepts the manifest",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16"),
        Diagnostic::new(
            code("DEVX-0012"),
            DiagnosticClass::ContractViolation,
            "an exit code preserves the retry decision of the failure it reports",
            "a class whose retryability is retryable_as_is maps to an exit code documented as terminal",
            source("crates/cli/src/exit.rs"),
        )
        .with_remedy(Remedy::new(
            "give the class a code whose is_retryable agrees with its retryability, or carry the retry decision in the JSON envelope",
            source("crates/cli/src/exit.rs"),
            "bioprism_devx::exitaudit::audit reports no retryability_inverted row",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .needing_human_decision()
        .citing("40.36")
        .citing("11.03"),
        Diagnostic::new(
            code("DEVX-0013"),
            DiagnosticClass::Indeterminate,
            "every changed path belongs to a declared surface of the local-loop contract",
            "the changed path belongs to no declared surface, so what it invalidates is unknown rather than nothing",
            source("<changed path>"),
        )
        .with_remedy(Remedy::new(
            "declare a surface owning the path, stating what a change there invalidates and why",
            source("crates/devx/src/devloop.rs"),
            "invalidated_by returns a set instead of UnownedSubject",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("11.25")
        .citing("41.10"),
        Diagnostic::new(
            code("DEVX-0014"),
            DiagnosticClass::ContractViolation,
            "a change to canonical serialisation is accompanied by a refresh of the cross-language parity fixtures",
            "canonical serialisation changed and the reference-profile parity fixtures were not regenerated",
            Site::Source {
                document: "crates/ids/src/canonical.rs".into(),
                span: Some(LineSpan::at(1)),
            },
        )
        .with_context("surface", crate::devloop::SURFACE_CANONICAL_SERIALISATION)
        .with_remedy(Remedy::new(
            "either regenerate the reference-profile fixtures against the CPython reference or revert the encoding change; no Rust-only test can detect the divergence",
            source("crates/ids/src/canonical.rs"),
            "the reference-profile digests match the other implementation's",
            ChangeRequired::Environment,
            Certainty::Observed,
        ))
        .needing_human_decision()
        .citing("11.25")
        .citing("43.26"),
        Diagnostic::new(
            code("DEVX-0015"),
            DiagnosticClass::Usage,
            "an invoked subcommand exists in the command tree",
            "the subcommand is not in the command tree",
            Site::Invocation {
                argument: "<subcommand>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "replace the subcommand with one the command tree declares",
            Site::Invocation {
                argument: "<subcommand>".into(),
            },
            "the command runs and exits 0 or 1",
            ChangeRequired::Invocation,
            Certainty::Observed,
        ))
        .citing("11.02")
        .citing("11.03"),
        Diagnostic::new(
            code("DEVX-0016"),
            DiagnosticClass::Conflict,
            "an identity is bound to one content digest for its lifetime",
            "the identity is already bound to a different digest, so accepting this request would rewrite a published result",
            Site::Digest {
                digest: "<incumbent digest>".into(),
            },
        )
        .with_discrepancy(Discrepancy::new("<incumbent digest>", "<offered digest>"))
        .with_remedy(Remedy::new(
            "rename the new content to a fresh identity; a published binding is never rewritten in place",
            source("<request payload>"),
            "the registry accepts the binding and the incumbent is untouched",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.36")
        .citing("11.01"),
        Diagnostic::new(
            code("DEVX-0017"),
            DiagnosticClass::PolicyDenied,
            "evidence is read only where policy and consent permit it",
            "the requested evidence is governed by a policy that withholds it from this caller",
            Site::Artifact {
                node_kind: "fact".into(),
                id: "<withheld fact>".into(),
            },
        )
        .with_context("influence", InfluenceClass::InaccessibleByPolicy.as_str())
        .with_remedy(Remedy::new(
            "obtain the access grant, or re-scope the query away from the withheld variables and record the gap in the decision",
            source("<access request>"),
            "the compile emits no inaccessible_by_policy omission group for these variables",
            ChangeRequired::Environment,
            Certainty::Observed,
        ))
        .needing_human_decision()
        .citing("39.05")
        .citing("43.26"),
        Diagnostic::new(
            code("DEVX-0018"),
            DiagnosticClass::Indeterminate,
            "an oracle returns a determined verdict when the evidence determines one",
            "the oracle ran to completion and the evidence does not determine a verdict",
            Site::Artifact {
                node_kind: "oracle".into(),
                id: "<oracle kind>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "widen the evidence the query admits, or accept the abstention as the answer; this is a scientific unknown and not an execution error",
            source("query.json"),
            "the oracle returns a status other than underdetermined",
            ChangeRequired::Contract,
            Certainty::Observed,
        ))
        .citing("40.36")
        .citing("43.28"),
        Diagnostic::new(
            code("DEVX-0019"),
            DiagnosticClass::Unavailable,
            "every declared dependency is reachable at the point of use",
            "a declared dependency could not be read",
            source("<dependency path>"),
        )
        .with_remedy(Remedy::new(
            "re-run once the dependency is reachable; the identical request is safe to re-send",
            source("<dependency path>"),
            "the dependency reads and the run proceeds",
            ChangeRequired::Environment,
            Certainty::Observed,
        ))
        .citing("40.36"),
        Diagnostic::new(
            code("DEVX-0020"),
            DiagnosticClass::Internal,
            "every failure is emitted under a class in the taxonomy",
            "a pass failed with no classification, so the unknown-failure rate is measurable rather than hidden in a neighbouring class",
            Site::Unlocated {
                because: "the failure escaped before a site was attached to it".into(),
            },
        )
        .with_remedy(Remedy::new(
            "report this with the compile record attached, so the failure can be given a class",
            source("crates/devx/src/catalogue.rs"),
            "the failure is raised under a specific DEVX code on the next run",
            ChangeRequired::Environment,
            Certainty::Observed,
        ))
        .citing("40.36"),
        Diagnostic::new(
            code("DEVX-0021"),
            DiagnosticClass::InvalidInput,
            "a plugin manifest is internally consistent before any registry sees it",
            "the manifest contradicts itself",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "set the contradicted field to a value consistent with the rest of the manifest",
            source("plugin manifest"),
            "PluginManifest::validate accepts the manifest",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16")
        .citing("11.13"),
        Diagnostic::new(
            code("DEVX-0022"),
            DiagnosticClass::InvalidInput,
            "a plugin claiming determinism declares no effect that varies between runs",
            "the plugin claims determinism while declaring an effect that is not reproducible across runs",
            Site::Artifact {
                node_kind: "plugin".into(),
                id: "<plugin>".into(),
            },
        )
        .with_remedy(Remedy::new(
            "drop the determinism claim, or remove the effect that contradicts it",
            source("plugin manifest"),
            "PluginManifest::validate accepts the manifest",
            ChangeRequired::Payload,
            Certainty::Observed,
        ))
        .citing("40.16"),
    ]
}

/// The catalogue as a lookup table.
pub fn index() -> Result<BTreeMap<String, Diagnostic>, CatalogueError> {
    let mut map: BTreeMap<String, Diagnostic> = BTreeMap::new();
    for entry in catalogue() {
        let key = entry.code.as_str().to_string();
        if let Some(incumbent) = map.get(&key) {
            return Err(CatalogueError::DuplicateCode {
                code: key,
                incumbent: incumbent.invariant.clone(),
                challenger: entry.invariant,
            });
        }
        map.insert(key, entry);
    }
    Ok(map)
}

/// Look one up.
pub fn lookup(code: &str) -> Result<Diagnostic, CatalogueError> {
    index()?
        .remove(code)
        .ok_or_else(|| CatalogueError::UnknownCode {
            code: code.to_string(),
        })
}

fn exemplar(code_text: &str) -> Diagnostic {
    lookup(code_text).expect("every code used by a converter is in the catalogue")
}

/// The catalogue code a version-negotiation failure is raised under.
pub fn code_for_negotiation_error(error: &NegotiationError) -> &'static str {
    match error {
        NegotiationError::MalformedVersion { .. } => "DEVX-0007",
        NegotiationError::HostSpeaksNothing => "DEVX-0008",
        NegotiationError::PluginSpeaksNothing { .. } => "DEVX-0009",
        NegotiationError::NoCommonVersion { .. } => "DEVX-0006",
    }
}

/// Turn a `bioprism-sdk` negotiation failure into a diagnostic.
///
/// The exemplar supplies the invariant, the class and the remedy shape; the error supplies the
/// observation and, for [`NegotiationError::NoCommonVersion`], the discrepancy. The near-miss the
/// SDK computes is folded into a second remedy, because "these two versions are on the same
/// profile line" is precisely a statement of what would have to change.
pub fn from_negotiation_error(error: &NegotiationError, plugin: &str) -> Diagnostic {
    let mut diagnostic = exemplar(code_for_negotiation_error(error));
    diagnostic.observed = error.to_string();
    diagnostic.site = Site::Artifact {
        node_kind: "plugin".to_string(),
        id: plugin.to_string(),
    };
    if let NegotiationError::NoCommonVersion {
        host_supported,
        plugin_supported,
        near_miss,
    } = error
    {
        diagnostic.discrepancy = Some(Discrepancy::new(host_supported, plugin_supported));
        if let Some(near) = near_miss {
            diagnostic = diagnostic.with_remedy(Remedy::new(
                format!("pin the plugin to the near-miss version pair: {near}"),
                source("plugin manifest"),
                "bioprism_sdk::negotiate returns a Negotiated value",
                ChangeRequired::Contract,
                Certainty::Inferred,
            ));
        }
    }
    diagnostic
}

/// The catalogue code a manifest failure is raised under.
pub fn code_for_manifest_error(error: &ManifestError) -> &'static str {
    match error {
        ManifestError::AdapterWithoutLossDeclaration { .. } => "DEVX-0010",
        ManifestError::ContradictoryLossDeclaration { .. } => "DEVX-0011",
        ManifestError::DeterminismContradictedByEffect { .. } => "DEVX-0022",
        ManifestError::EmptyName
        | ManifestError::EmptyVersion { .. }
        | ManifestError::NoCapabilities { .. }
        | ManifestError::DuplicateCapabilityKind { .. }
        | ManifestError::EvidenceForUndeclaredCapability { .. }
        | ManifestError::NotDigestible { .. } => "DEVX-0021",
    }
}

/// Turn a `bioprism-sdk` manifest failure into a diagnostic.
pub fn from_manifest_error(error: &ManifestError, plugin: &str) -> Diagnostic {
    let mut diagnostic = exemplar(code_for_manifest_error(error));
    diagnostic.observed = error.to_string();
    diagnostic.site = Site::Artifact {
        node_kind: "plugin".to_string(),
        id: plugin.to_string(),
    };
    diagnostic
}

/// Diagnostics implied by a compile record.
///
/// The developer view of [`crate::introspect`] made actionable: an unknown-influence group and a
/// reasonless deferred pass are both things a reader of the record should be told about, and both
/// have a stated next move.
pub fn diagnose_record(record: &CompileRecord) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    for entry in record.blocking_omissions() {
        if entry.influence == InfluenceClass::Unknown {
            let mut diagnostic = exemplar("DEVX-0002");
            diagnostic.observed = format!(
                "the manifest carries {} subjects in the group {:?} with unknown influence",
                entry.count, entry.reason
            );
            diagnostic.site = record.site();
            out.push(diagnostic);
        }
        if entry.influence == InfluenceClass::InaccessibleByPolicy {
            let mut diagnostic = exemplar("DEVX-0017");
            diagnostic.observed = format!(
                "{} subjects were withheld by policy under the group {:?}",
                entry.count, entry.reason
            );
            diagnostic.site = record.site();
            out.push(diagnostic);
        }
    }

    for pass in record.did_not_run() {
        let reason = pass.outcome.absence_reason().unwrap_or("");
        if reason.trim().is_empty() {
            let mut diagnostic = exemplar("DEVX-0005");
            diagnostic.observed = format!(
                "the pass {:?} is recorded as {} with an empty reason",
                pass.name,
                pass.outcome.as_str()
            );
            diagnostic.site = Site::Artifact {
                node_kind: "pass".to_string(),
                id: pass.name.clone(),
            };
            out.push(diagnostic);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::introspect::{PassOutcome, PassRecord};
    use crate::lint::{lint, lint_catalogue, LintRule};
    use bioprism_section::{OmissionGroup, OmissionManifest};
    use std::collections::BTreeSet;

    #[test]
    fn the_catalogue_has_no_duplicate_codes_and_is_sorted() {
        let entries = catalogue();
        let codes: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
        let unique: BTreeSet<&&str> = codes.iter().collect();
        assert_eq!(unique.len(), codes.len());
        assert!(index().is_ok());
    }

    #[test]
    fn the_catalogue_exercises_every_diagnostic_class() {
        let used: BTreeSet<DiagnosticClass> = catalogue().iter().map(|e| e.class).collect();
        for class in DiagnosticClass::ALL {
            assert!(used.contains(&class), "no catalogue entry for {class}");
        }
    }

    #[test]
    fn the_catalogue_has_zero_lint_errors() {
        let report = lint_catalogue();
        assert!(
            report.is_clean(),
            "catalogue lint errors: {:?}",
            report.errors()
        );
        assert_eq!(report.checked, 22);
    }

    #[test]
    fn the_catalogue_trips_exactly_two_heuristic_warnings_and_they_are_named() {
        let report = lint_catalogue();
        let warnings = report.warnings();
        let codes: Vec<&str> = warnings.iter().map(|f| f.code.as_str()).collect();
        assert_eq!(
            codes,
            vec!["DEVX-0001", "DEVX-0014"],
            "the accepted warning set changed: {warnings:?}"
        );
        assert!(warnings
            .iter()
            .all(|f| f.rule == LintRule::RemedyIsNotAnInstruction));
    }

    #[test]
    fn every_entry_states_a_remedy_with_a_way_to_verify_it() {
        for entry in catalogue() {
            assert!(!entry.remedies.is_empty(), "{} has no remedy", entry.code);
            for remedy in &entry.remedies {
                assert!(
                    !remedy.verified_by.trim().is_empty(),
                    "{} has an unverifiable remedy",
                    entry.code
                );
                assert_ne!(remedy.change_required, ChangeRequired::Unknown);
            }
        }
    }

    #[test]
    fn every_entry_cites_at_least_one_blueprint_module() {
        for entry in catalogue() {
            assert!(
                !entry.blueprint_modules.is_empty(),
                "{} cites nothing",
                entry.code
            );
        }
    }

    #[test]
    fn only_entries_needing_a_person_are_flagged_for_a_human_decision() {
        let entries = catalogue();
        let flagged: Vec<&str> = entries
            .iter()
            .filter(|e| e.human_decision_required)
            .map(|e| e.code.as_str())
            .collect();
        assert_eq!(flagged, vec!["DEVX-0012", "DEVX-0014", "DEVX-0017"]);
    }

    #[test]
    fn every_negotiation_error_maps_to_a_catalogued_code() {
        let errors = [
            NegotiationError::MalformedVersion {
                text: "nope".into(),
            },
            NegotiationError::HostSpeaksNothing,
            NegotiationError::PluginSpeaksNothing {
                host_supported: "fiber-world/0.1".into(),
            },
            NegotiationError::NoCommonVersion {
                host_supported: "fiber-world/0.1".into(),
                plugin_supported: "fiber-world/0.2".into(),
                near_miss: Some("fiber-world/0.1 vs fiber-world/0.2".into()),
            },
        ];
        for error in &errors {
            let diagnostic = from_negotiation_error(error, "acme-adapter");
            assert!(lookup(diagnostic.code.as_str()).is_ok());
            assert_eq!(diagnostic.observed, error.to_string());
        }
    }

    #[test]
    fn a_near_miss_becomes_a_second_remedy_asserted_only_as_inferred() {
        let error = NegotiationError::NoCommonVersion {
            host_supported: "fiber-world/0.1".into(),
            plugin_supported: "fiber-world/0.2".into(),
            near_miss: Some("fiber-world/0.1 and fiber-world/0.2 share a profile line".into()),
        };
        let diagnostic = from_negotiation_error(&error, "acme-adapter");
        assert_eq!(diagnostic.remedies.len(), 2);
        assert_eq!(diagnostic.remedies[1].confidence, Certainty::Inferred);
        assert!(diagnostic.discrepancy.is_some());
    }

    #[test]
    fn every_manifest_error_maps_to_a_catalogued_code() {
        let errors = [
            ManifestError::EmptyName,
            ManifestError::EmptyVersion {
                plugin: "p".into(),
            },
            ManifestError::NoCapabilities {
                plugin: "p".into(),
            },
            ManifestError::DuplicateCapabilityKind {
                plugin: "p".into(),
                kind: "adapter".into(),
            },
            ManifestError::AdapterWithoutLossDeclaration {
                plugin: "p".into(),
            },
            ManifestError::ContradictoryLossDeclaration {
                plugin: "p".into(),
                kinds: "unmapped_column".into(),
            },
            ManifestError::DeterminismContradictedByEffect {
                plugin: "p".into(),
                effect: "network".into(),
            },
            ManifestError::EvidenceForUndeclaredCapability {
                plugin: "p".into(),
                kind: "oracle".into(),
            },
            ManifestError::NotDigestible {
                plugin: "p".into(),
                detail: "nan".into(),
            },
        ];
        for error in &errors {
            let diagnostic = from_manifest_error(error, "p");
            assert!(lookup(diagnostic.code.as_str()).is_ok());
        }
    }

    #[test]
    fn diagnostics_derived_from_sdk_errors_pass_the_same_lint_as_the_catalogue() {
        let diagnostics = vec![
            from_negotiation_error(&NegotiationError::HostSpeaksNothing, "acme"),
            from_manifest_error(&ManifestError::EmptyName, "acme"),
        ];
        assert!(lint(&diagnostics).is_clean());
    }

    #[test]
    fn an_unknown_influence_group_produces_the_sufficiency_diagnostic() {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: "not analysed".into(),
            influence: InfluenceClass::Unknown,
            count: 4,
            bound: None,
            examples: Vec::new(),
        });
        let record = CompileRecord::new("w", "q").with_manifest(manifest);
        let produced = diagnose_record(&record);
        assert_eq!(produced.len(), 1);
        assert_eq!(produced[0].code.as_str(), "DEVX-0002");
        assert!(produced[0].observed.contains("4 subjects"));
    }

    #[test]
    fn a_zero_influence_group_produces_no_diagnostic_at_all() {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: "no dependency path reaches the target".into(),
            influence: InfluenceClass::Zero,
            count: 40,
            bound: None,
            examples: Vec::new(),
        });
        let record = CompileRecord::new("w", "q").with_manifest(manifest);
        assert!(diagnose_record(&record).is_empty());
    }

    #[test]
    fn a_deferred_pass_with_an_empty_reason_produces_the_internal_diagnostic() {
        let record = CompileRecord::new("w", "q").with_pass(PassRecord::new(
            "obstruction_tests",
            PassOutcome::Deferred {
                reason: "   ".into(),
            },
            "nothing to do",
        ));
        let produced = diagnose_record(&record);
        assert_eq!(produced.len(), 1);
        assert_eq!(produced[0].code.as_str(), "DEVX-0005");
    }

    #[test]
    fn record_diagnostics_are_lint_clean() {
        let mut manifest = OmissionManifest::default();
        manifest.push(OmissionGroup {
            reason: "withheld by consent".into(),
            influence: InfluenceClass::InaccessibleByPolicy,
            count: 2,
            bound: None,
            examples: Vec::new(),
        });
        let record = CompileRecord::new("w", "q")
            .with_manifest(manifest)
            .bound_by("a".repeat(64));
        let produced = diagnose_record(&record);
        assert_eq!(produced.len(), 1);
        assert!(lint(&produced).is_clean());
    }

    #[test]
    fn an_unknown_code_is_a_typed_refusal() {
        assert!(matches!(
            lookup("DEVX-9999"),
            Err(CatalogueError::UnknownCode { .. })
        ));
    }
}
