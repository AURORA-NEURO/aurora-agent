//! The local development loop, as a contract about what a change invalidates.
//!
//! Blueprint 11.25 (local development, documentation and optional telemetry) asks for
//! "deterministic setup" and "seeded fixtures", and 11.24 for scaffolding and conformance. Neither
//! says the thing a contributor most needs to know, which is: *I changed this — what is no longer
//! trustworthy?* This module answers that question as data.
//!
//! # The expensive lesson, stated up front
//!
//! **A change that touches canonical serialisation invalidates the cross-language parity
//! fixtures.** `bioprism-section`'s reference certificate profile exists to be byte-compatible with
//! a CPython reference runtime, and that compatibility is a property of the *bytes*, not of the
//! fields. Reordering a map, changing a float repr, or normalising a string differently produces a
//! certificate that still verifies against itself and no longer matches the other implementation.
//! Nothing in a Rust-only test run detects it. [`SURFACE_CANONICAL_SERIALISATION`] declares that
//! blast radius so a contributor learns it from the contract rather than from a red CI job on
//! another repository.
//!
//! # Unknown blast radius is not empty blast radius
//!
//! [`invalidated_by`] refuses, with [`LoopError::UnownedSubject`], when the changed file belongs to
//! no declared surface. Returning an empty invalidation set for an unrecognised path would be the
//! same error `bioprism-section` guards against with `InfluenceClass::Unknown`: silence that reads
//! as safety. A contributor adding a new surface must declare it.
//!
//! # Documentation changes are delegated, not re-walked
//!
//! `bioprism-docgraph` already implements change-impact analysis over the documentation graph
//! ([`impact_of`]), including the decision about which edge types propagate transitively and a
//! record of where the walk stopped. This module calls it and attaches the report. Writing a
//! second graph walk would produce a second answer to the same question, and the two would
//! disagree the first time either was tuned.
//!
//! # Not implemented
//!
//! No watcher, no filesystem, no process execution, no test runner. This module maps a *declared*
//! change onto a *declared* set of artefacts. It does not know whether the change happened, cannot
//! read the file, and will not run the tests it names. It also has no diff awareness — a whitespace
//! fix and a rewritten invariant in the same file produce the same answer, which
//! `bioprism-docgraph` names as a limitation of its own analysis and which is inherited here.

use crate::error::LoopError;
use bioprism_docgraph::{impact_of, DocGraph, ImpactReport, ModuleId, TaskRoute};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The surface whose blast radius is the one worth memorising.
pub const SURFACE_CANONICAL_SERIALISATION: &str = "canonical-serialisation";

/// What kind of thing stopped being trustworthy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    /// A test suite that must be re-run.
    Test,
    /// A seeded input fixture that may need regenerating.
    Fixture,
    /// A committed expected-output artefact.
    GoldenArtifact,
    /// Any certificate digest computed before the change.
    CertificateDigest,
    /// A fixture whose purpose is agreement with a non-Rust implementation. The expensive one:
    /// nothing in this workspace can re-derive it.
    CrossLanguageParityFixture,
    /// A compiled documentation context bundle.
    DocBundle,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::Test => "test",
            ArtifactKind::Fixture => "fixture",
            ArtifactKind::GoldenArtifact => "golden_artifact",
            ArtifactKind::CertificateDigest => "certificate_digest",
            ArtifactKind::CrossLanguageParityFixture => "cross_language_parity_fixture",
            ArtifactKind::DocBundle => "doc_bundle",
        }
    }

    /// Whether re-establishing this artefact needs something outside this workspace.
    ///
    /// True only for [`CrossLanguageParityFixture`](ArtifactKind::CrossLanguageParityFixture),
    /// and that is why it is a separate kind rather than a golden artefact with a note.
    pub fn needs_an_external_implementation(self) -> bool {
        self == ArtifactKind::CrossLanguageParityFixture
    }
}

/// One artefact a change invalidates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Invalidated {
    pub kind: ArtifactKind,
    pub name: String,
    /// The surface that claimed it.
    pub surface: String,
    /// Why this change reaches this artefact. Every entry carries one; an invalidation set with a
    /// bare list of names is a set nobody trusts enough to act on.
    pub because: String,
}

/// A declared surface of the workspace and what depends on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Surface {
    pub id: String,
    /// Exact repository-relative paths this surface owns. No globs: a glob is a promise about
    /// files that do not exist yet.
    pub owns: Vec<String>,
    /// What a change here invalidates.
    pub invalidates: Vec<Invalidated>,
    /// Why the surface is drawn where it is drawn.
    pub rationale: String,
}

impl Surface {
    pub fn owns_path(&self, path: &str) -> bool {
        self.owns.iter().any(|owned| owned == path)
    }
}

/// The change a contributor made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "change", rename_all = "snake_case")]
pub enum ChangeUnit {
    /// A repository-relative source path.
    SourceFile { path: String },
    /// A documentation module in the `bioprism-docgraph` corpus.
    DocModule { module: String },
}

impl ChangeUnit {
    pub fn source(path: impl Into<String>) -> Self {
        ChangeUnit::SourceFile { path: path.into() }
    }

    pub fn doc(module: impl Into<String>) -> Self {
        ChangeUnit::DocModule {
            module: module.into(),
        }
    }

    pub fn subject(&self) -> &str {
        match self {
            ChangeUnit::SourceFile { path } => path,
            ChangeUnit::DocModule { module } => module,
        }
    }
}

/// Everything a change invalidates.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidationSet {
    pub change: ChangeUnit,
    /// The surfaces that claimed the change, in declaration order.
    pub surfaces: Vec<String>,
    /// The invalidated artefacts, sorted and deduplicated.
    pub entries: Vec<Invalidated>,
    /// The documentation impact report, when the change was to a documentation module. Produced by
    /// `bioprism-docgraph`, not recomputed here.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doc_impact: Option<ImpactReport>,
}

impl InvalidationSet {
    pub fn of_kind(&self, kind: ArtifactKind) -> Vec<&Invalidated> {
        self.entries.iter().filter(|e| e.kind == kind).collect()
    }

    /// Whether cross-language parity must be re-established outside this workspace.
    pub fn invalidates_cross_language_parity(&self) -> bool {
        !self
            .of_kind(ArtifactKind::CrossLanguageParityFixture)
            .is_empty()
    }

    /// Whether every certificate digest computed before the change is now wrong.
    pub fn invalidates_certificate_digests(&self) -> bool {
        !self.of_kind(ArtifactKind::CertificateDigest).is_empty()
    }

    pub fn names(&self) -> BTreeSet<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }

    /// Whether this set covers everything another set covers.
    ///
    /// Used to check that a broader surface is genuinely broader, which is the property that makes
    /// the contract worth reading: a contributor who learns the canonical-serialisation radius
    /// should not later discover a narrower surface reaching further.
    pub fn covers(&self, other: &InvalidationSet) -> bool {
        let mine = self.names();
        other.names().iter().all(|name| mine.contains(name))
    }
}

/// The declared surfaces of this workspace.
///
/// Hand-written and therefore incomplete: it declares the surfaces whose blast radius is both
/// non-obvious and expensive to discover, not every file in the repository. A path outside this
/// list produces [`LoopError::UnownedSubject`] rather than an empty answer.
pub fn workspace_contract() -> Vec<Surface> {
    vec![
        Surface {
            id: SURFACE_CANONICAL_SERIALISATION.to_string(),
            owns: vec![
                "crates/ids/src/canonical.rs".to_string(),
                "crates/ids/src/hash.rs".to_string(),
            ],
            rationale: "canonical bytes are the input to every content hash in the workspace, and \
                        the reference certificate profile is byte-compatible with a CPython \
                        implementation that this workspace cannot re-run"
                .to_string(),
            invalidates: vec![
                Invalidated {
                    kind: ArtifactKind::CrossLanguageParityFixture,
                    name: "fiber-context-certificate/0.1 reference-profile digests".to_string(),
                    surface: SURFACE_CANONICAL_SERIALISATION.to_string(),
                    because: "the reference profile's compatibility claim is about bytes; a \
                              different canonical encoding produces a certificate that still \
                              verifies against itself and no longer matches the other \
                              implementation, and no Rust-only test can see that"
                        .to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::CertificateDigest,
                    name: "every certificate_sha256 computed before the change".to_string(),
                    surface: SURFACE_CANONICAL_SERIALISATION.to_string(),
                    because: "ContentHash::of_value hashes canonical bytes, so every digest \
                              derived from them moves"
                        .to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::GoldenArtifact,
                    name: "every committed expected-output artefact containing a digest".to_string(),
                    surface: SURFACE_CANONICAL_SERIALISATION.to_string(),
                    because: "golden artefacts embed digests as literals".to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::Test,
                    name: "bioprism-ids canonicalisation suite".to_string(),
                    surface: SURFACE_CANONICAL_SERIALISATION.to_string(),
                    because: "it is the suite that defines the encoding".to_string(),
                },
            ],
        },
        Surface {
            id: "certificate-schema".to_string(),
            owns: vec!["crates/section/src/certificate.rs".to_string()],
            rationale: "the reference profile's field set and field order are part of the \
                        cross-language contract; the extended profile's are not"
                .to_string(),
            invalidates: vec![
                Invalidated {
                    kind: ArtifactKind::CrossLanguageParityFixture,
                    name: "fiber-context-certificate/0.1 reference-profile digests".to_string(),
                    surface: "certificate-schema".to_string(),
                    because: "the reference profile emits exactly the field set the CPython \
                              reference produces; adding, removing or reordering a field there \
                              breaks the match"
                        .to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::CertificateDigest,
                    name: "every certificate_sha256 computed before the change".to_string(),
                    surface: "certificate-schema".to_string(),
                    because: "the digest is taken over the certificate body".to_string(),
                },
            ],
        },
        Surface {
            id: "omission-manifest".to_string(),
            owns: vec!["crates/section/src/omission.rs".to_string()],
            rationale: "the omission manifest is carried only by the extended certificate \
                        profile, so its shape is not part of the cross-language contract; this is \
                        a precision claim about crates/section/src/certificate.rs, which inserts \
                        omission_manifest only under CertificateProfile::Extended, and it stops \
                        being true the moment the reference profile carries the manifest"
                .to_string(),
            invalidates: vec![
                Invalidated {
                    kind: ArtifactKind::CertificateDigest,
                    name: "fiber-context-certificate/0.2-extended digests".to_string(),
                    surface: "omission-manifest".to_string(),
                    because: "the extended body embeds the manifest and the sufficiency flag"
                        .to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::Test,
                    name: "every sufficiency assertion in the workspace".to_string(),
                    surface: "omission-manifest".to_string(),
                    because: "supports_sufficiency_claim is derived from the influence classes \
                              defined in this file"
                        .to_string(),
                },
            ],
        },
        Surface {
            id: "exit-code-registry".to_string(),
            owns: vec!["crates/cli/src/exit.rs".to_string()],
            rationale: "the shipped exit codes are transcribed into this crate, so a change there \
                        silently invalidates a transcription no compiler checks"
                .to_string(),
            invalidates: vec![
                Invalidated {
                    kind: ArtifactKind::Fixture,
                    name: "bioprism_devx::exitaudit::shipped_exit_codes".to_string(),
                    surface: "exit-code-registry".to_string(),
                    because: "bioprism-cli is not in bioprism-devx's dependency set, so the \
                              registry is a hand copy and drift is invisible to the compiler"
                        .to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::Test,
                    name: "the exit-code audit assertions".to_string(),
                    surface: "exit-code-registry".to_string(),
                    because: "the audit's finding count is asserted against the transcription"
                        .to_string(),
                },
            ],
        },
        Surface {
            id: "diagnostic-catalogue".to_string(),
            owns: vec!["crates/devx/src/catalogue.rs".to_string()],
            rationale: "the catalogue is both the data the lint grades and the fixture the lint's \
                        own result is asserted against"
                .to_string(),
            invalidates: vec![
                Invalidated {
                    kind: ArtifactKind::Test,
                    name: "the catalogue lint result assertions".to_string(),
                    surface: "diagnostic-catalogue".to_string(),
                    because: "the accepted warning count is asserted entry by entry".to_string(),
                },
                Invalidated {
                    kind: ArtifactKind::Fixture,
                    name: "any consumer pinned to a DEVX code".to_string(),
                    surface: "diagnostic-catalogue".to_string(),
                    because: "codes are the join key and are not renumbered silently".to_string(),
                },
            ],
        },
    ]
}

/// Validate a contract before trusting it.
pub fn validate_contract(surfaces: &[Surface]) -> Result<(), LoopError> {
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for surface in surfaces {
        if !seen.insert(surface.id.as_str()) {
            return Err(LoopError::DuplicateSurface {
                surface: surface.id.clone(),
            });
        }
        if surface.invalidates.is_empty() {
            return Err(LoopError::EmptyInvalidation {
                surface: surface.id.clone(),
            });
        }
    }
    Ok(())
}

/// What a change invalidates.
///
/// A [`ChangeUnit::DocModule`] is answered by `bioprism-docgraph`'s
/// [`impact_of`]; a [`ChangeUnit::SourceFile`] is answered from the declared surfaces. A source
/// path no surface owns is a refusal, not an empty set.
pub fn invalidated_by(
    surfaces: &[Surface],
    change: &ChangeUnit,
    graph: &DocGraph,
    routes: &[TaskRoute],
) -> Result<InvalidationSet, LoopError> {
    validate_contract(surfaces)?;

    match change {
        ChangeUnit::SourceFile { path } => {
            let owning: Vec<&Surface> = surfaces
                .iter()
                .filter(|surface| surface.owns_path(path))
                .collect();
            if owning.is_empty() {
                return Err(LoopError::UnownedSubject {
                    subject: path.clone(),
                });
            }
            let mut entries: Vec<Invalidated> = owning
                .iter()
                .flat_map(|surface| surface.invalidates.iter().cloned())
                .collect();
            entries.sort();
            entries.dedup();
            Ok(InvalidationSet {
                change: change.clone(),
                surfaces: owning.iter().map(|s| s.id.clone()).collect(),
                entries,
                doc_impact: None,
            })
        }
        ChangeUnit::DocModule { module } => {
            let id = ModuleId::parse(module.clone()).map_err(|_| LoopError::UnownedSubject {
                subject: module.clone(),
            })?;
            if !graph.contains(&id) {
                return Err(LoopError::UnownedSubject {
                    subject: module.clone(),
                });
            }
            let report = impact_of(graph, &id, routes);
            let mut entries: Vec<Invalidated> = report
                .affected
                .iter()
                .map(|hop| Invalidated {
                    kind: ArtifactKind::DocBundle,
                    name: hop.module.as_str().to_string(),
                    surface: "doc-corpus".to_string(),
                    because: format!(
                        "reached from {} via {} at depth {}",
                        hop.from.as_str(),
                        hop.via.as_str(),
                        hop.depth
                    ),
                })
                .collect();
            entries.extend(report.affected_routes.iter().map(|route| Invalidated {
                kind: ArtifactKind::DocBundle,
                name: format!("route:{}", route.as_str()),
                surface: "doc-corpus".to_string(),
                because: "the route declares a module in the impact closure".to_string(),
            }));
            entries.sort();
            entries.dedup();
            Ok(InvalidationSet {
                change: change.clone(),
                surfaces: vec!["doc-corpus".to_string()],
                entries,
                doc_impact: Some(report),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_docgraph::fixture::{repository_doc_graph, repository_routes};

    fn empty_graph() -> DocGraph {
        DocGraph::new()
    }

    #[test]
    fn a_change_to_canonical_serialisation_invalidates_the_cross_language_parity_fixtures() {
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::source("crates/ids/src/canonical.rs"),
            &empty_graph(),
            &[],
        )
        .expect("the surface is declared");
        assert!(set.invalidates_cross_language_parity());
        assert!(set.invalidates_certificate_digests());
        let parity = set.of_kind(ArtifactKind::CrossLanguageParityFixture);
        assert_eq!(parity.len(), 1);
        assert!(parity[0].kind.needs_an_external_implementation());
    }

    #[test]
    fn the_canonical_serialisation_radius_covers_the_certificate_schema_radius() {
        let contract = workspace_contract();
        let wide = invalidated_by(
            &contract,
            &ChangeUnit::source("crates/ids/src/canonical.rs"),
            &empty_graph(),
            &[],
        )
        .expect("declared");
        let narrow = invalidated_by(
            &contract,
            &ChangeUnit::source("crates/section/src/certificate.rs"),
            &empty_graph(),
            &[],
        )
        .expect("declared");
        assert!(
            wide.covers(&narrow),
            "a contributor who learned the wide radius would be surprised by the narrow one"
        );
    }

    #[test]
    fn a_change_to_the_omission_manifest_spares_the_reference_profile_parity_fixtures() {
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::source("crates/section/src/omission.rs"),
            &empty_graph(),
            &[],
        )
        .expect("declared");
        assert!(!set.invalidates_cross_language_parity());
        assert!(set.invalidates_certificate_digests());
        assert!(set.entries.iter().any(|e| e.name.contains("0.2-extended")));
    }

    #[test]
    fn a_file_no_surface_owns_is_a_refusal_and_never_an_empty_set() {
        let error = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::source("crates/onco/src/lib.rs"),
            &empty_graph(),
            &[],
        )
        .expect_err("no surface declares this path");
        assert!(matches!(error, LoopError::UnownedSubject { .. }));
    }

    #[test]
    fn every_invalidated_entry_states_why_it_is_invalidated() {
        for surface in workspace_contract() {
            for entry in &surface.invalidates {
                assert!(
                    entry.because.len() > 20,
                    "{}/{} carries no usable reason",
                    surface.id,
                    entry.name
                );
                assert_eq!(entry.surface, surface.id);
            }
        }
    }

    #[test]
    fn a_surface_that_invalidates_nothing_is_rejected() {
        let surfaces = vec![Surface {
            id: "empty".into(),
            owns: vec!["a.rs".into()],
            invalidates: Vec::new(),
            rationale: "none".into(),
        }];
        assert!(matches!(
            validate_contract(&surfaces),
            Err(LoopError::EmptyInvalidation { .. })
        ));
    }

    #[test]
    fn a_duplicate_surface_id_is_rejected() {
        let mut surfaces = workspace_contract();
        let first = surfaces[0].clone();
        surfaces.push(first);
        assert!(matches!(
            validate_contract(&surfaces),
            Err(LoopError::DuplicateSurface { .. })
        ));
    }

    #[test]
    fn a_documentation_change_delegates_to_docgraphs_impact_analysis() {
        let graph = repository_doc_graph();
        let routes = repository_routes();
        let seed = graph
            .node_ids()
            .next()
            .expect("the repository fixture has modules")
            .clone();
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::doc(seed.as_str()),
            &graph,
            &routes,
        )
        .expect("the module is in the corpus");
        let report = set.doc_impact.as_ref().expect("impact report attached");
        assert_eq!(report.changed, seed);
        assert_eq!(
            set.entries.len(),
            report.affected.len() + report.affected_routes.len()
        );
    }

    #[test]
    fn a_documentation_module_outside_the_corpus_is_a_refusal() {
        let error = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::doc("not-a-module"),
            &repository_doc_graph(),
            &[],
        )
        .expect_err("the module is not in the corpus");
        assert!(matches!(error, LoopError::UnownedSubject { .. }));
    }

    #[test]
    fn an_invalidation_set_round_trips_through_json() {
        let set = invalidated_by(
            &workspace_contract(),
            &ChangeUnit::source("crates/cli/src/exit.rs"),
            &empty_graph(),
            &[],
        )
        .expect("declared");
        let encoded = serde_json::to_string(&set).expect("serialises");
        let decoded: InvalidationSet = serde_json::from_str(&encoded).expect("parses back");
        assert_eq!(set, decoded);
    }
}
