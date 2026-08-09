//! The typed lens catalogue, and an honest map of section 42.
//!
//! Section 42 has 31 modules. This crate implements six of them as lenses, two more as
//! cross-cutting machinery, and deliberately implements none of the rest. Silence about the
//! remainder would be the section's own failure repeated: a catalogue that lists what exists and
//! not what is missing is a view whose omissions are invisible.
//!
//! [`catalogue`] returns the six declarations, so a caller can ask what each lens will and will
//! not answer *before* running anything. [`DISCHARGED_BY_GRAPH`] and [`NOT_IMPLEMENTED`] cover the
//! other twenty-five modules by name.
//!
//! # On the shape of section 42
//!
//! The 31 module files run 2356 lines, of which 2206 are byte-identical scaffolding: an identical
//! "View contract" paragraph, an identical seven-item "Required user flows" list, an identical
//! seven-object "Required API objects" list, identical performance rules, identical failure modes
//! and identical acceptance gates. Each module differs from 42.01 in exactly five lines — its
//! title, its `module_id`, its heading, its one-sentence user outcome, and the label inside a
//! Mermaid diagram. That is **93.6% boilerplate**, higher than any other section measured so far.
//!
//! The consequence for this crate is direct. Thirty-one modules do not describe thirty-one
//! things; they describe one thing — a view contract — applied to thirty-one nouns. Implementing
//! them one-to-one would produce thirty-one structurally identical types distinguished by a
//! string, which is the "instance count is not benchmark count" error from `AGENTS.md` wearing a
//! module id. So the shared contract is implemented once, in [`crate::grammar`] and
//! [`crate::nonvisual`], and only the modules whose *distinguishing sentence* implies a checkable
//! semantics get a lens.

use crate::anytime::AnytimeCurveLens;
use crate::claim::ClaimEvidenceLens;
use crate::grammar::{Lens, LensDeclaration, LensId};
use crate::leakage::CohortLeakageLens;
use crate::profile::ContextTokenProfileLens;
use crate::qc::AssayQcMissingnessLens;
use crate::transport::CausalTransportLens;

/// The six implemented lenses, in blueprint-module order.
///
/// Each declaration is validated at construction, so this function panicking would mean a lens in
/// this crate declares scope preconditions it would silently ignore. The test suite calls it.
pub fn catalogue() -> Vec<LensDeclaration> {
    vec![
        CohortLeakageLens.declaration(),
        ClaimEvidenceLens.declaration(),
        CausalTransportLens.declaration(),
        AssayQcMissingnessLens.declaration(),
        ContextTokenProfileLens.declaration(),
        AnytimeCurveLens.declaration(),
    ]
}

/// The identifiers of the implemented lenses, for building a [`crate::gate::ReleaseGate`].
pub fn catalogue_ids() -> Vec<LensId> {
    catalogue().into_iter().map(|d| d.id().clone()).collect()
}

/// Section 42 modules this crate implements directly, and where.
///
/// Six are lenses. The other four are the cross-cutting contract every lens obeys, which is why
/// they are implemented once rather than thirty-one times.
pub const IMPLEMENTED_HERE: &[(&str, &str)] = &[
    ("42.01", "crate::grammar — the lens grammar itself"),
    ("42.10", "crate::leakage — CohortLeakageLens"),
    ("42.11", "crate::claim — ClaimEvidenceLens"),
    ("42.12", "crate::transport — CausalTransportLens"),
    (
        "42.13",
        "crate::qc and crate::missingness — AssayQcMissingnessLens",
    ),
    ("42.21", "crate::profile — ContextTokenProfileLens"),
    ("42.22", "crate::anytime — AnytimeCurveLens"),
    (
        "42.27",
        "crate::nonvisual — the Witness bound that makes an unanswerable lens uncompilable",
    ),
    (
        "42.30",
        "crate::grammar::Coverage — completeness as a first-class field, in part; see \
         NOT_IMPLEMENTED for the renderer half",
    ),
    (
        "42.31",
        "crate::gate — ReleaseGate as an executable predicate",
    ),
];

/// Section 42 modules whose content is already discharged by `bioprism-graph`, with what that
/// crate actually provides.
///
/// `bioprism-graph` claims sections 41 and 42 "under the constraint of 43.01": it renders a
/// compiled Decision Section into four generated projections and refuses to seal one that has
/// dropped an obligation or an oracle witness. That is the *projection* half of section 42. This
/// crate is the *question* half. Neither subsumes the other, and both are needed: a projection
/// that carries every obstruction still does not tell you what question it answers, and a lens
/// declaration does not draw anything.
pub const DISCHARGED_BY_GRAPH: &[(&str, &str)] = &[
    (
        "42.01",
        "the view side of the lens grammar: `Projection`, `View` with sealed provenance, and the \
         refusal to emit a view that has lost an obstruction. This crate adds the question, the \
         requirements, the preconditions and the refusals.",
    ),
    (
        "42.27",
        "`TableProjection` — the accessible fallback of a *graph*, flattened to text columns. \
         This crate covers the accessible form of an *answer*, which is a different object: a \
         table of the graph does not tell a non-visual reader whether the leakage check ran.",
    ),
    (
        "42.03",
        "`GraphProjection` and `vocabulary` are the BioWorld explorer's node and edge inventory.",
    ),
    (
        "42.04",
        "`TimelineProjection`, which keeps event time separate from availability time and records \
         clock anomalies rather than merging them.",
    ),
    (
        "42.15",
        "`ProjectionSource` and `ProvenanceCheck` bind a view to the section and certificate \
         digests it came from, which is the inspector's provenance requirement.",
    ),
];

/// Section 42 modules this crate deliberately does not implement, and why.
///
/// Read this as a specification of scope, not a to-do list. Several of these are *refusals*: the
/// blueprint under-specifies them to the point where an implementation would be an invention with
/// a module id stamped on it.
pub const NOT_IMPLEMENTED: &[(&str, &str)] = &[
    (
        "42.02",
        "Home and question router. Routing a natural-language goal to a lens needs an intent \
         model; the blueprint specifies no grammar for the goal, no ranking, and no behaviour when \
         two lenses match. A router that guesses is worse than a catalogue a caller reads.",
    ),
    (
        "42.05",
        "Lesion and region correspondence. The one genuinely novel requirement — representing \
         uncertain many-to-many mapping instead of forcing identity — needs the scope mapping \
         taxonomy of 43.05, which `bioprism-scope` already types. A lens here would restate it.",
    ),
    (
        "42.06 / 42.07 / 42.08 / 42.09",
        "Specimen lineage, imaging coordinate lineage, molecular reference, single-cell and \
         spatial. All four are inventory lenses over domain data models that do not exist in this \
         workspace. Their distinguishing sentences name entities, not checks.",
    ),
    (
        "42.14",
        "Workflow and reproducibility. Executable lineage over containers and data versions is a \
         real check, and it is a check over an execution substrate this crate cannot see.",
    ),
    (
        "42.16 / 42.17",
        "Fork comparison lab and oracle mesh inspector. Both are `bioprism-prism` and \
         `bioprism-oracle` territory: comparing matched continuations requires decision cells, and \
         keeping oracle disagreement unflattened is already `bioprism-atlas`'s label distribution.",
    ),
    (
        "42.18 / 42.19 / 42.20",
        "Benchmark genealogy, capability atlas, failure atlas. `bioprism-atlas` implements the \
         capability ontology, the failure taxonomy and the coverage report these lenses would view.",
    ),
    (
        "42.23 / 42.24 / 42.26",
        "Cross-modal alignment, research tumour board, saved views and stories. Interaction \
         surfaces: shared selection state, annotation, and ordered replay steps. There is no \
         interaction model in this workspace to attach them to.",
    ),
    (
        "42.25",
        "Graph search and command palette. The blueprint's one durable requirement — that the \
         compiled query be inspectable — belongs to the query compiler, not to a palette.",
    ),
    (
        "42.28 / 42.29",
        "Local-first workspace and public/private/federated modes. Deployment topology. The \
         epistemic content is that a federated answer must disclose which partitions it could not \
         read, and that is already expressible as a partial `Coverage` with named pending regions.",
    ),
    (
        "42.30",
        "Implemented only in part. Completeness as a first-class field is in `Coverage`; \
         aggregation, viewport queries, workers, streaming and layout caching are renderer \
         concerns with no renderer here, and a first-useful-view latency budget would need a clock \
         this crate does not have.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_declaration_in_the_catalogue_is_well_formed() {
        assert_eq!(catalogue().len(), 6);
    }

    #[test]
    fn every_lens_has_a_distinct_id_and_a_distinct_blueprint_module() {
        let ids: BTreeSet<String> = catalogue()
            .iter()
            .map(|d| d.id().as_str().to_string())
            .collect();
        let modules: BTreeSet<&str> = catalogue().iter().map(|d| d.blueprint_module()).collect();
        assert_eq!(ids.len(), 6);
        assert_eq!(modules.len(), 6);
    }

    #[test]
    fn every_lens_states_a_question_and_names_the_evidence_it_needs() {
        for declaration in catalogue() {
            assert!(
                declaration.question().len() > 20,
                "{} has a question too short to be one",
                declaration.id()
            );
            assert!(
                !declaration.requires().is_empty(),
                "{} names no required evidence, so absence would be undetectable",
                declaration.id()
            );
        }
    }

    #[test]
    fn every_lens_that_declares_a_precondition_declares_the_matching_refusal() {
        for declaration in catalogue() {
            if !declaration.preconditions().is_empty() {
                assert!(
                    declaration.declares_refusal(crate::RefusalReason::ScopePreconditionUnmet),
                    "{} would ignore its own precondition",
                    declaration.id()
                );
            }
        }
    }

    #[test]
    fn the_catalogue_covers_the_six_load_bearing_modules_of_section_42() {
        let modules: BTreeSet<&str> = catalogue().iter().map(|d| d.blueprint_module()).collect();
        for expected in ["42.10", "42.11", "42.12", "42.13", "42.21", "42.22"] {
            assert!(modules.contains(expected), "{expected} is not implemented");
        }
    }

    #[test]
    fn the_unimplemented_list_is_not_empty_and_gives_a_reason_for_each_entry() {
        assert!(NOT_IMPLEMENTED.len() >= 10);
        for (module, reason) in NOT_IMPLEMENTED {
            assert!(!module.is_empty());
            assert!(
                reason.len() > 60,
                "{module} is dismissed without a reason worth reading"
            );
        }
    }

    #[test]
    fn the_graph_discharge_list_names_what_that_crate_provides_rather_than_only_a_module_id() {
        assert!(!DISCHARGED_BY_GRAPH.is_empty());
        for (module, what) in DISCHARGED_BY_GRAPH {
            assert!(!module.is_empty());
            assert!(what.len() > 60);
        }
    }

    #[test]
    fn every_module_of_section_42_is_accounted_for_somewhere() {
        let mut named: BTreeSet<String> = BTreeSet::new();
        for (modules, _) in IMPLEMENTED_HERE
            .iter()
            .chain(DISCHARGED_BY_GRAPH)
            .chain(NOT_IMPLEMENTED)
        {
            for token in modules.split('/') {
                named.insert(token.trim().to_string());
            }
        }
        let missing: Vec<String> = (1..=crate::SECTION_42_MODULE_COUNT)
            .map(|n| format!("42.{n:02}"))
            .filter(|id| !named.contains(id))
            .collect();
        assert!(
            missing.is_empty(),
            "section 42 modules named nowhere in this crate's account: {missing:?}"
        );
    }

    #[test]
    fn every_lens_in_the_catalogue_appears_in_the_implemented_list() {
        let implemented: BTreeSet<&str> = IMPLEMENTED_HERE.iter().map(|(m, _)| *m).collect();
        for declaration in catalogue() {
            assert!(
                implemented.contains(declaration.blueprint_module()),
                "{} implements {} but the account does not say so",
                declaration.id(),
                declaration.blueprint_module()
            );
        }
    }

    #[test]
    fn catalogue_ids_match_the_declarations() {
        let ids = catalogue_ids();
        assert_eq!(ids.len(), 6);
        for (id, declaration) in ids.iter().zip(catalogue()) {
            assert_eq!(id, declaration.id());
        }
    }
}
