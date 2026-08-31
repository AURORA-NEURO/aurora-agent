//! Graph-first knowledge and navigation, for documents.
//!
//! Implements blueprint section 41. The thesis of this workspace is that context assembly is a
//! compiler pass rather than a retrieval heuristic; `bioprism-fiber` applies that to a world of
//! facts, and this crate applies the same argument to a corpus of documents. Given a task, it
//! compiles the smallest set of documentation modules sufficient to do it, and emits a record of
//! what was left out and whether that could have mattered.
//!
//! It is meant to be pointed at this repository: `AGENTS.md`, `docs/`, `.agents/skills/` and the
//! crate-level `//!` docs. [`fixture::repository_doc_graph`] is that corpus transcribed, and
//! [`scan`] is the path that reads the real bytes.
//!
//! # What `bioprism-graph` already discharges, and why this crate is not a second one
//!
//! `bioprism-graph` also claims section 41 — under the constraint of 43.01, which says graphs are
//! *generated projections of compiled decision regions*. It projects a compiled world into a
//! typed graph, a hypergraph, a timeline and a table. Concretely it discharges:
//!
//! - **41.01** the graph model, as the shape a projected decision region takes;
//! - **41.03** the edge vocabulary, for edges between *evidence* nodes, from
//!   `machine/edge_vocabulary.json`;
//! - part of **41.11**, the acyclicity and integrity lint over a projected graph.
//!
//! Nothing here projects a world. This crate never sees a `DecisionSection`, a factor or a fact.
//! Its nodes are files with an H1 and its edges are relations between documents, so the two
//! crates share section 41's vocabulary and nothing else. The overlap is real and is confined to
//! [`vocabulary`], which extends the same ten-member list rather than restating it: `refines`,
//! `contradicts` and `example_of` are additions this corpus needs and a world of evidence does
//! not, and the untyped `related` member that `machine/edge_vocabulary.json` carries is absent
//! from both, for the same reason.
//!
//! # The modules
//!
//! | blueprint | here |
//! |---|---|
//! | 41.01 documentation graph model, 41.02 module registry, 41.04 context cards | [`registry`] |
//! | 41.03 edge vocabulary | [`vocabulary`] |
//! | 41.05 task routes | [`route`] |
//! | 41.06 token profiles | [`tokens`] |
//! | 41.07 graph traversal policy | [`traverse`] |
//! | 41.09 context bundle compiler | [`bundle`] |
//! | 41.10 change-impact analysis | [`impact`] |
//! | 41.11 graph linting and conformance | [`lint`] |
//! | 41.12 agent reading protocol | [`protocol`] |
//! | 41.16 acceptance tests | [`fixture`] and `tests/` |
//!
//! # The three rules this crate exists to keep
//!
//! **Fail rather than truncate.** A bundle whose mandatory set does not fit its declared budget
//! returns [`BundleError::MandatorySetExceedsBudget`]. It never drops a required module and
//! returns a bundle, because a truncated mandatory set is indistinguishable at the point of use
//! from a complete one. This is `bioprism-fiber`'s protected-closure rule, applied to documents.
//!
//! **Zero influence is not unknown influence.** An omitted module is classified
//! [`InfluenceClass::Zero`](bioprism_section::InfluenceClass::Zero) only when the traversal that
//! failed to reach it was *exhaustive* — no depth cap, no filtered edge type present in the
//! graph, no policy-denied node, no dangling edge. Otherwise it is
//! [`Unknown`](bioprism_section::InfluenceClass::Unknown): nobody looked. The distinction lives
//! in [`traverse::Completeness`] and is checked in [`bundle::OmissionReason::influence`].
//!
//! **An estimate is never a measurement.** [`tokens::TokenCost`] carries its basis, there is no
//! constructor that produces a bare number, and summation takes the weaker basis. Nothing in this
//! crate can produce a [`Measured`](tokens::CostBasis::Measured) cost, because there is no
//! tokenizer here to produce one with.
//!
//! # Where section 41 under-specifies, and what was chosen
//!
//! Section 41's sixteen modules are 72.6% identical scaffolding — 608 of 837 non-blank lines
//! appear in fifteen or more of the sixteen, leaving a mean of 14.1 distinguishing lines per
//! module. The shared frame ("machine-readable JSON or JSONL representation; deterministic local
//! CLI behavior; stable module IDs and paths; exact source hashes…") is a house style, not a
//! specification, so the decisions below had to be made here and are recorded rather than
//! presented as the blueprint's:
//!
//! - **Edge semantics.** 41.03 lists nine edge names and glosses their direction. It says nothing
//!   about what a traversal should *do* with each. [`vocabulary`] supplies four mechanical
//!   answers per type; every one of them is this crate's, defended in that module's docs.
//! - **What an omission entry means.** 41.09 names an "omission list" with no schema. Mapping
//!   onto `bioprism-section`'s `InfluenceClass` exposed a genuine gap — a module examined and
//!   then dropped for budget is worse than one nobody looked at, and the shared vocabulary has no
//!   member for it. See [`bundle::OmissionReason`].
//! - **What "conservative" bounds.** 41.10 requires conservative impact results and does not say
//!   against what. Read as a floor on recall it justifies transitivity everywhere and produces a
//!   report naming the corpus; [`impact`] reads it as a floor on the *direct* neighbourhood and
//!   makes transitivity a property of the edge type.
//! - **How a card is sized.** 41.04 gives a 60–180 token band with no tokenizer. The ceiling is
//!   enforced against this crate's estimator ([`lint::CARD_TOKEN_CEILING`]) and is therefore a
//!   warning, not an error.
//! - **The `governs` direction.** The normative gloss reads "target constrains source", which
//!   inverts the English word. Reproduced rather than corrected, matching `bioprism-graph`.
//!
//! # Not implemented, deliberately
//!
//! - **41.08 the docgraph CLI.** This is a library. The workspace's `bioprism-cli` owns binaries;
//!   a second CLI here would be a second argument parser and a second output format.
//! - **41.13 human navigation and 41.14 the offline graph atlas.** Both are renderings — HTML,
//!   layout, a browsable artifact. This crate produces the data such a renderer would draw, and
//!   `bioprism-graph` already argues why the data layer should not know about colour.
//! - **41.15 documentation evolution.** Tracking how a module changed across releases needs a
//!   version store this crate does not have. [`impact`] answers "what does this change reach",
//!   which is the part that does not require history.
//! - **A Markdown parser.** [`markdown`] reads front matter, ATX headings and inline links, and
//!   says in its own docs what it cannot see. There is no Markdown crate in this workspace and
//!   none is being added.
//! - **A tokenizer.** See [`tokens`].
//! - **A filesystem walk anywhere except [`scan`].** Every other module is pure data.
//!
//! [`BundleError::MandatorySetExceedsBudget`]: error::BundleError::MandatorySetExceedsBudget

pub mod bundle;
pub mod error;
pub mod fixture;
pub mod impact;
pub mod instrument_action_contract;
pub mod knowledge_gateway;
pub mod lint;
pub mod markdown;
pub mod protocol;
pub mod registry;
pub mod route;
pub mod scan;
pub mod tokens;
pub mod traverse;
pub mod vocabulary;
pub mod document_graph_integrity_support;
pub mod local_document_graph_integrity_inference;
pub mod multimodal_document_graph_integrity_inference;
pub mod throughput_document_graph_integrity_inference;
pub mod federated_continual_document_graph_integrity_inference;
pub mod local_document_graph_integrity_contract_model;
pub mod multimodal_document_graph_integrity_contract_model;
pub mod throughput_document_graph_integrity_contract_model;
pub mod federated_continual_document_graph_integrity_contract_model;
pub mod local_document_graph_integrity_research_copilot;
pub mod multimodal_document_graph_integrity_research_copilot;
pub mod throughput_document_graph_integrity_research_copilot;
pub mod federated_continual_document_graph_integrity_research_copilot;
pub mod local_document_graph_integrity_workflow_fabric;
pub mod multimodal_document_graph_integrity_workflow_fabric;
pub mod throughput_document_graph_integrity_workflow_fabric;
pub mod federated_continual_document_graph_integrity_workflow_fabric;

pub use document_graph_integrity_support::{
    DocumentGraphIntegrityArtifact4, DocumentGraphIntegrityCard7, DocumentGraphIntegrityError,
    DocumentGraphIntegrityRequest4, DocumentModule4,
};

pub use instrument_action_contract::{
    capability_manifest as instrument_action_contract_manifest, validate_instrument_actions,
    InstrumentAction4, InstrumentActionContractError, InstrumentActionReceipt2,
    InstrumentActionRequest4, CONTENT_TYPE as INSTRUMENT_ACTION_CONTENT_TYPE,
    CONTRACT_VERSION as INSTRUMENT_ACTION_CONTRACT_VERSION,
    FEATURE_ID as INSTRUMENT_ACTION_FEATURE_ID, INPUT_SCHEMA as INSTRUMENT_ACTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as INSTRUMENT_ACTION_OUTPUT_SCHEMA,
};

pub use bundle::{
    compile_bundle, BundleEntry, BundleOmission, ContextBundle, InclusionReason, OmissionReason,
    Sufficiency,
};
pub use error::{BundleError, DocGraphError};
pub use impact::{impact_of, ImpactHop, ImpactReport, PropagationStop};
pub use knowledge_gateway::{
    operate_knowledge_gateway, ClaimState, KnowledgeGatewayDisposition, KnowledgeGatewayError,
    KnowledgeGatewayReceipt, ScopedResearchClaim, ScopedResearchClaims, TypedKnowledgeWorld,
    CONTRACT_VERSION as KNOWLEDGE_GATEWAY_CONTRACT_VERSION,
    FEATURE_ID as KNOWLEDGE_GATEWAY_FEATURE_ID,
};
pub use lint::{lint, LintFinding, LintReport, LintSeverity};
pub use markdown::{first_h1, headings, link_targets, parse_document, FrontMatter, Heading};
pub use protocol::{check_receipt, Citation, Claim, ClaimKind, ProtocolViolation, ReadingReceipt};
pub use registry::{
    ContextCard, DocGraph, ModuleBody, ModuleId, ModuleNode, NodeStatus, ProtectedClass,
};
pub use route::{RouteDefect, RouteId, TaskRoute};
pub use scan::{scan_markdown_tree, ScanError, ScanOptions, ScanReport};
pub use tokens::{estimate_tokens, CostBasis, ProfileLevel, TokenCost, ESTIMATOR_ID};
pub use traverse::{traverse, Completeness, PartialReason, Traversal, TraversalPolicy};
pub use vocabulary::{CompanionRequirement, DocEdge, DocEdgeType, ImpactPropagation};
