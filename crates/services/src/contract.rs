//! What a service contract *is*, as a value.
//!
//! Implements the shared shape behind blueprint 40.18, 40.20, 40.22, 40.23, 40.24, 40.25 and
//! 40.27, under the conventions of 40.11 (API Conventions and Idempotency) and the taxonomy of
//! 40.36. Every one of those modules states the same five things — inputs, outputs, non-negotiable
//! invariants, failure semantics, and the effects it is permitted — and states them as prose
//! bullets. A bullet cannot be checked. [`ServiceContract`] is those five things as types.
//!
//! # There is no server here
//!
//! Nothing in this module opens a socket, routes a path, or serialises a response to a client.
//! 40.11 names REST, OpenAPI and server-sent events; none of them appear. A contract is the
//! agreement two pieces of code are held to, and 40.03 is explicit that the same application
//! services run in-process locally and behind a boundary when hosted. If the contract were a
//! description of HTTP it would be false for the in-process case, which is the case the alpha must
//! support.
//!
//! # Request and response are governed schemas, not new types
//!
//! Both shapes are [`SchemaDescriptor`]s from `bioprism-governance`. This is not convenience. The
//! platform's central guarantee is that a Context Certificate digest means the same thing to three
//! independent implementations, and `bioprism-governance` already owns the rule that any change
//! moving a digest is breaking however innocent it looks field by field. A second classifier
//! living here would eventually disagree with the first, and the disagreement would surface as a
//! contract change waved through as compatible. So [`ContractChange`] delegates: it computes two
//! [`governance::diff`]s and takes the maximum class, and adds nothing of its own except the rule
//! that a response change is at least as severe as the same change on a request.
//!
//! # Idempotency is declared, not inferred
//!
//! 40.11's first invariant is that write retries cannot create duplicate semantic objects. That is
//! a property of the operation, and the only honest way to hold a service to it is to make the
//! service say which kind of operation it is ([`Idempotency`]) and then check the claim against
//! what it actually does ([`Effect`]). A contract that declares itself [`Idempotency::Pure`] and
//! also declares a write effect is refused at construction.

use crate::error::{ErrorClass, FailureMode, ServicesError};
use bioprism_governance::{
    classify, diff, Classification, CompatibilityClass, CompatibilityMode, FieldSpec, FieldType,
    SchemaDescriptor, SchemaId, VersionBump,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The nine §40 modules this crate holds the workspace against.
///
/// Seven are service contracts with a request and a response. Two — [`ContractId::ServiceGraph`]
/// and [`ContractId::DomainBoundaries`] — describe the arrangement of the other seven rather than
/// an operation, and are checked structurally in [`crate::graph`] instead. They are in the same
/// enum because the conformance audit has to report on all nine, and an audit that silently
/// dropped the two hardest ones would be flattering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContractId {
    ServiceGraph,
    DomainBoundaries,
    WorldBuilder,
    DecisionCompiler,
    MutationRuntime,
    MatchedEvaluator,
    AdaptiveScheduler,
    ContextCompiler,
    RegistryBackend,
}

impl ContractId {
    pub const ALL: [ContractId; 9] = [
        ContractId::ServiceGraph,
        ContractId::DomainBoundaries,
        ContractId::WorldBuilder,
        ContractId::DecisionCompiler,
        ContractId::MutationRuntime,
        ContractId::MatchedEvaluator,
        ContractId::AdaptiveScheduler,
        ContractId::ContextCompiler,
        ContractId::RegistryBackend,
    ];

    /// The blueprint module this contract is read from.
    pub fn module_id(self) -> &'static str {
        match self {
            ContractId::ServiceGraph => "40.03",
            ContractId::DomainBoundaries => "40.04",
            ContractId::WorldBuilder => "40.18",
            ContractId::DecisionCompiler => "40.20",
            ContractId::MutationRuntime => "40.22",
            ContractId::MatchedEvaluator => "40.23",
            ContractId::AdaptiveScheduler => "40.24",
            ContractId::ContextCompiler => "40.25",
            ContractId::RegistryBackend => "40.27",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            ContractId::ServiceGraph => "service-graph",
            ContractId::DomainBoundaries => "domain-boundaries",
            ContractId::WorldBuilder => "world-builder",
            ContractId::DecisionCompiler => "decision-compiler",
            ContractId::MutationRuntime => "mutation-runtime",
            ContractId::MatchedEvaluator => "matched-evaluator",
            ContractId::AdaptiveScheduler => "adaptive-scheduler",
            ContractId::ContextCompiler => "context-compiler",
            ContractId::RegistryBackend => "registry-backend",
        }
    }

    /// The blueprint's own title for the module.
    pub fn title(self) -> &'static str {
        match self {
            ContractId::ServiceGraph => "Service and Process Graph",
            ContractId::DomainBoundaries => "Domain Boundaries and Ownership",
            ContractId::WorldBuilder => "BioWorld Builder Service",
            ContractId::DecisionCompiler => "BioDecision Compiler Service",
            ContractId::MutationRuntime => "Biological Mutation Runtime and Lineage",
            ContractId::MatchedEvaluator => "Matched Evaluator Runtime",
            ContractId::AdaptiveScheduler => "Adaptive Scheduler and Suite Selection",
            ContractId::ContextCompiler => "Biological Context Compiler Service Contract",
            ContractId::RegistryBackend => "Registry and Hub Backend",
        }
    }

    /// Whether the module specifies an operation with a request and a response.
    pub fn is_operation(self) -> bool {
        !matches!(
            self,
            ContractId::ServiceGraph | ContractId::DomainBoundaries
        )
    }
}

impl fmt::Display for ContractId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.slug(), self.module_id())
    }
}

/// How a result reaches the caller.
///
/// 40.11 invariant 2: long-running work returns a job resource. The distinction is contractual
/// because it changes what a caller must do with the response, and it is the same distinction
/// whether the boundary is a process or a function call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    /// The response is the result.
    Immediate,
    /// The response is a handle; the result arrives through it later.
    Job,
}

/// The retry-safety property of a write, per 40.11 invariant 1.
///
/// Distinct from [`crate::error::Retryability`], which is a property of a failure. This is a
/// property of the operation: whether re-sending it is safe at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Idempotency {
    /// A function of its inputs with no effects. Retry is free.
    Pure,
    /// Writes, but the same content lands at the same address. Retry converges.
    ContentAddressed,
    /// Writes, and duplicate suppression needs a caller-supplied key.
    KeyedWrite,
    /// Appends to a log. Retry duplicates the entry unless the caller deduplicates.
    AppendOnly,
    /// Retry produces a second semantic object. 40.11 forbids this for anything reachable by a
    /// write retry, so a contract declaring it is declaring a defect.
    NotIdempotent,
}

impl Idempotency {
    /// Whether a contract in this class may declare no write effects at all.
    pub fn is_pure(self) -> bool {
        matches!(self, Idempotency::Pure)
    }

    /// Whether re-sending the identical request is safe without further coordination.
    pub fn retry_is_safe(self) -> bool {
        matches!(
            self,
            Idempotency::Pure | Idempotency::ContentAddressed | Idempotency::KeyedWrite
        )
    }
}

/// An effect a contract is permitted to have.
///
/// Every §40 module carries the clause *apply least authority and deny undeclared effects*. That
/// clause is only enforceable against a declaration, which is what this set is.
/// [`Effect::ReadsEvaluatorState`] is separated from ordinary graph reads because the same clause
/// forbids exposing evaluator holdouts to evaluated agents, and a contract that reads them is
/// exactly the one a reviewer must look at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Effect {
    ReadsGraph,
    ReadsEvaluatorState,
    WritesArtifactStore,
    AppendsEventLedger,
    ExecutesSandbox,
    WritesSearchIndex,
}

impl Effect {
    pub fn is_write(self) -> bool {
        matches!(
            self,
            Effect::WritesArtifactStore | Effect::AppendsEventLedger | Effect::WritesSearchIndex
        )
    }
}

/// A §40 service contract: what goes in, what comes out, what must hold, how it fails, what it may
/// touch.
#[derive(Debug, Clone, PartialEq)]
pub struct ServiceContract {
    pub id: ContractId,
    /// The request shape, as a governed schema so that changes to it classify under the same rule
    /// as any other wire change.
    pub request: SchemaDescriptor,
    pub response: SchemaDescriptor,
    /// The module's "Non-negotiable invariants", one string each, in blueprint order.
    pub invariants: Vec<String>,
    /// The module's "Failure semantics" bullets, each bound to exactly one error class.
    pub failures: Vec<FailureMode>,
    pub delivery: Delivery,
    pub idempotency: Idempotency,
    pub effects: BTreeSet<Effect>,
}

/// A contract under construction.
///
/// A transcription has eight parts and a positional constructor for eight parts is a constructor
/// nobody reads correctly. Each method here names what it is setting, so a transcription can be
/// checked against the blueprint section it came from without counting commas.
#[derive(Debug, Clone)]
pub struct ContractDraft {
    id: ContractId,
    request: SchemaDescriptor,
    response: SchemaDescriptor,
    invariants: Vec<String>,
    failures: Vec<FailureMode>,
    delivery: Delivery,
    idempotency: Idempotency,
    effects: BTreeSet<Effect>,
}

impl ContractDraft {
    /// The module's "Non-negotiable invariants", verbatim and in order.
    pub fn invariants(mut self, sentences: &[&str]) -> Self {
        self.invariants = sentences.iter().map(|text| (*text).to_string()).collect();
        self
    }

    /// The module's "Failure semantics" bullets, each bound to one error class.
    pub fn failing(mut self, failures: Vec<FailureMode>) -> Self {
        self.failures = failures;
        self
    }

    pub fn delivering(mut self, delivery: Delivery, idempotency: Idempotency) -> Self {
        self.delivery = delivery;
        self.idempotency = idempotency;
        self
    }

    /// The effects the module's Execution path requires.
    pub fn touching(mut self, effects: &[Effect]) -> Self {
        self.effects = effects.iter().copied().collect();
        self
    }

    /// Finish, refusing the two ways a contract can contradict itself.
    ///
    /// A failure label bound to two different classes would make the taxonomy a relation rather
    /// than a function, and a pure contract that also writes is claiming an authority it says it
    /// does not use.
    pub fn build(self) -> Result<ServiceContract, ServicesError> {
        let mut seen: BTreeMap<&str, ErrorClass> = BTreeMap::new();
        for failure in &self.failures {
            if let Some(&first) = seen.get(failure.label.as_str()) {
                if first != failure.class {
                    return Err(ServicesError::ContradictoryFailureMode {
                        contract: self.id.slug().to_string(),
                        label: failure.label.clone(),
                        first,
                        second: failure.class,
                    });
                }
            }
            seen.insert(failure.label.as_str(), failure.class);
        }

        if self.idempotency.is_pure() && self.effects.iter().any(|effect| effect.is_write()) {
            return Err(ServicesError::MalformedDescriptor {
                contract: self.id.slug().to_string(),
                part: "effects",
                detail: "declared pure while also declaring a write effect".to_string(),
            });
        }

        Ok(ServiceContract {
            id: self.id,
            request: self.request,
            response: self.response,
            invariants: self.invariants,
            failures: self.failures,
            delivery: self.delivery,
            idempotency: self.idempotency,
            effects: self.effects,
        })
    }
}

impl ServiceContract {
    /// Start a transcription from the module's Inputs and Outputs.
    pub fn describing(
        id: ContractId,
        request: SchemaDescriptor,
        response: SchemaDescriptor,
    ) -> ContractDraft {
        ContractDraft {
            id,
            request,
            response,
            invariants: Vec::new(),
            failures: Vec::new(),
            delivery: Delivery::Immediate,
            idempotency: Idempotency::Pure,
            effects: BTreeSet::new(),
        }
    }

    pub fn module_id(&self) -> &'static str {
        self.id.module_id()
    }

    /// The classes a caller of this contract must be prepared to handle.
    pub fn error_classes(&self) -> BTreeSet<ErrorClass> {
        self.failures.iter().map(|failure| failure.class).collect()
    }

    /// The declared failure modes that carry a given class.
    pub fn failures_in(&self, class: ErrorClass) -> Vec<&FailureMode> {
        self.failures
            .iter()
            .filter(|failure| failure.class == class)
            .collect()
    }

    /// The outputs the blueprint says this contract produces.
    pub fn declared_outputs(&self) -> Vec<&str> {
        self.response
            .paths()
            .into_iter()
            .filter(|path| *path != VERSION_FIELD)
            .collect()
    }

    /// The inputs the blueprint says this contract requires.
    pub fn required_inputs(&self) -> Vec<&str> {
        self.request
            .fields()
            .iter()
            .filter(|field| field.presence.is_required() && field.path != VERSION_FIELD)
            .map(|field| field.path.as_str())
            .collect()
    }

    /// Classify a proposed successor of this contract, using the workspace's schema classifier.
    ///
    /// The class is the maximum over the request and response diffs. Nothing here decides what is
    /// breaking; `bioprism-governance` does, including the rule that any change touching hashed
    /// bytes is breaking whatever it looks like field by field.
    pub fn change_to(&self, next: &ServiceContract) -> Result<ContractChange, ServicesError> {
        if self.id != next.id {
            return Err(ServicesError::Incomparable {
                contract: self.id.slug().to_string(),
                other: next.id.slug().to_string(),
                detail: "two different contracts are not two versions of one".to_string(),
            });
        }
        let request = diff(&self.request, &next.request)
            .map(|change| classify(&change))
            .map_err(|error| ServicesError::Incomparable {
                contract: self.id.slug().to_string(),
                other: next.id.slug().to_string(),
                detail: format!("request: {error}"),
            })?;
        let response = diff(&self.response, &next.response)
            .map(|change| classify(&change))
            .map_err(|error| ServicesError::Incomparable {
                contract: self.id.slug().to_string(),
                other: next.id.slug().to_string(),
                detail: format!("response: {error}"),
            })?;
        Ok(ContractChange {
            id: self.id,
            request,
            response,
        })
    }
}

/// The classification of one contract revision against its predecessor.
#[derive(Debug, Clone, PartialEq)]
pub struct ContractChange {
    pub id: ContractId,
    pub request: Classification,
    pub response: Classification,
}

impl ContractChange {
    /// The severity of the change: the worse of the two sides.
    pub fn class(&self) -> CompatibilityClass {
        self.request.class.max(self.response.class)
    }

    /// Whether any part of the change moves the bytes an artifact is identified by.
    ///
    /// A response field is part of a published artifact; a request field is not. Both sides are
    /// asked anyway, because a request descriptor here also describes the manifest a build is
    /// pinned to, and that manifest is hashed.
    pub fn moves_a_digest(&self) -> bool {
        !self.request.digest_affecting().is_empty() || !self.response.digest_affecting().is_empty()
    }

    /// The version bump the change requires.
    pub fn required_bump(&self) -> VersionBump {
        self.request.required_bump.max(self.response.required_bump)
    }

    /// Refuse a claim of compatibility that the classifier does not support.
    ///
    /// Delegates to [`Classification::assert_class`] on both sides so that the error a caller sees
    /// is the same `DigestBreach` the schema gate raises, with the same evidence.
    pub fn assert_class(
        &self,
        claimed: CompatibilityClass,
    ) -> Result<(), bioprism_governance::GovernanceError> {
        self.request.assert_class(claimed)?;
        self.response.assert_class(claimed)
    }

    /// Whether the version strings moved far enough for what changed, on both sides.
    pub fn version_gate(&self) -> Result<(), bioprism_governance::CompatibilityError> {
        self.request.version_gate()?;
        self.response.version_gate()
    }
}

/// The field every contract document carries its own version in.
pub const VERSION_FIELD: &str = "schema_version";

/// A descriptor whose first field is its own version marker.
///
/// Every governed format in this workspace puts `schema_version` inside the hashed bytes; a
/// contract document that did not would be a document whose identity does not include which
/// contract it is.
pub fn descriptor(
    id: &str,
    mode: CompatibilityMode,
    fields: Vec<FieldSpec>,
) -> Result<SchemaDescriptor, ServicesError> {
    let parsed = SchemaId::parse(id).map_err(|error| ServicesError::MalformedDescriptor {
        contract: id.to_string(),
        part: "schema id",
        detail: error.to_string(),
    })?;
    let mut all = vec![FieldSpec::required(
        VERSION_FIELD,
        FieldType::Const(id.to_string()),
    )];
    all.extend(fields);
    SchemaDescriptor::new(parsed, mode, all).map_err(|error| ServicesError::MalformedDescriptor {
        contract: id.to_string(),
        part: "fields",
        detail: error.to_string(),
    })
}

/// A required field of unspecified interior structure.
pub fn required(path: &str) -> FieldSpec {
    FieldSpec::required(path, FieldType::Object)
}

/// A required list.
pub fn required_list(path: &str) -> FieldSpec {
    FieldSpec::required(path, FieldType::Array)
}

/// An optional field. Used where the blueprint lists an input a caller may omit.
pub fn optional(path: &str) -> FieldSpec {
    FieldSpec::optional(path, FieldType::Object)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Invalidates;

    fn shape(id: &str, field: &str) -> SchemaDescriptor {
        descriptor(
            id,
            CompatibilityMode::PreserveAndForward,
            vec![required(field)],
        )
        .expect("a test descriptor is well formed")
    }

    fn contract(
        idempotency: Idempotency,
        effects: &[Effect],
    ) -> Result<ServiceContract, ServicesError> {
        ServiceContract::describing(
            ContractId::WorldBuilder,
            shape("bioprism-test-request/0.1", "manifest"),
            shape("bioprism-test-response/0.1", "world"),
        )
        .invariants(&["Build is deterministic under pinned inputs."])
        .failing(vec![FailureMode::result(
            "dependency not pinned",
            ErrorClass::InvalidInput,
        )])
        .delivering(Delivery::Immediate, idempotency)
        .touching(effects)
        .build()
    }

    #[test]
    fn every_contract_id_names_a_distinct_blueprint_module() {
        let modules: BTreeSet<&str> = ContractId::ALL
            .into_iter()
            .map(ContractId::module_id)
            .collect();
        assert_eq!(modules.len(), ContractId::ALL.len());
        let slugs: BTreeSet<&str> = ContractId::ALL.into_iter().map(ContractId::slug).collect();
        assert_eq!(slugs.len(), ContractId::ALL.len());
    }

    #[test]
    fn exactly_two_of_the_nine_modules_describe_an_arrangement_rather_than_an_operation() {
        let structural: Vec<ContractId> = ContractId::ALL
            .into_iter()
            .filter(|id| !id.is_operation())
            .collect();
        assert_eq!(
            structural,
            [ContractId::ServiceGraph, ContractId::DomainBoundaries]
        );
    }

    #[test]
    fn a_contract_declaring_purity_and_a_write_effect_is_refused() {
        let error = contract(Idempotency::Pure, &[Effect::WritesArtifactStore])
            .expect_err("purity and a write cannot both hold");
        assert!(matches!(
            error,
            ServicesError::MalformedDescriptor {
                part: "effects",
                ..
            }
        ));
    }

    #[test]
    fn a_pure_contract_may_still_read() {
        let ok =
            contract(Idempotency::Pure, &[Effect::ReadsGraph]).expect("reading is not a write");
        assert!(ok.idempotency.is_pure());
    }

    #[test]
    fn a_failure_label_bound_to_two_classes_is_refused_because_the_taxonomy_must_be_a_function() {
        let error = ServiceContract::describing(
            ContractId::ContextCompiler,
            shape("bioprism-test-request/0.1", "goal"),
            shape("bioprism-test-response/0.1", "capsule"),
        )
        .failing(vec![
            FailureMode::result("stale graph", ErrorClass::Stale),
            FailureMode::result("stale graph", ErrorClass::Unavailable),
        ])
        .build()
        .expect_err("one label, two classes");
        assert!(matches!(
            error,
            ServicesError::ContradictoryFailureMode { .. }
        ));
    }

    #[test]
    fn the_same_label_repeated_with_the_same_class_is_allowed() {
        let ok = ServiceContract::describing(
            ContractId::ContextCompiler,
            shape("bioprism-test-request/0.1", "goal"),
            shape("bioprism-test-response/0.1", "capsule"),
        )
        .failing(vec![
            FailureMode::new("stale graph", ErrorClass::Stale, Invalidates::Projection),
            FailureMode::new("stale graph", ErrorClass::Stale, Invalidates::Result),
        ])
        .build()
        .expect("a repeated label with one class is consistent");
        assert_eq!(ok.error_classes(), BTreeSet::from([ErrorClass::Stale]));
    }

    #[test]
    fn a_contract_cannot_be_compared_with_a_different_contract() {
        let a = contract(Idempotency::Pure, &[]).expect("builds");
        let mut b = a.clone();
        b.id = ContractId::RegistryBackend;
        assert!(matches!(
            a.change_to(&b),
            Err(ServicesError::Incomparable { .. })
        ));
    }

    #[test]
    fn adding_a_required_response_field_is_breaking_and_the_governance_classifier_says_so() {
        let before = contract(Idempotency::Pure, &[]).expect("builds");
        let mut after = before.clone();
        after.response = descriptor(
            "bioprism-test-response/0.2",
            CompatibilityMode::PreserveAndForward,
            vec![required("world"), required("build_report")],
        )
        .expect("well formed");

        let change = before.change_to(&after).expect("same contract");
        assert_eq!(change.class(), CompatibilityClass::Breaking);
        assert!(change.moves_a_digest());
        assert_eq!(change.required_bump(), VersionBump::Major);
        change
            .version_gate()
            .expect("0.1 -> 0.2 under a zero major is the breaking bump");
        assert!(change.assert_class(CompatibilityClass::Compatible).is_err());
    }

    #[test]
    fn a_contract_change_that_moves_no_digest_is_not_breaking() {
        let before = contract(Idempotency::Pure, &[]).expect("builds");
        let mut after = before.clone();
        after.response = descriptor(
            "bioprism-test-response/0.1",
            CompatibilityMode::PreserveAndForward,
            vec![
                required("world"),
                FieldSpec::optional("build_report", FieldType::Object).excluded_from_digest(),
            ],
        )
        .expect("well formed");

        let change = before.change_to(&after).expect("same contract");
        assert!(!change.moves_a_digest());
        assert_ne!(change.class(), CompatibilityClass::Breaking);
    }

    #[test]
    fn the_version_field_is_not_reported_as_an_input_or_an_output() {
        let contract = contract(Idempotency::Pure, &[]).expect("builds");
        assert_eq!(contract.required_inputs(), ["manifest"]);
        assert_eq!(contract.declared_outputs(), ["world"]);
    }

    #[test]
    fn every_contract_descriptor_carries_a_version_marker_inside_its_own_bytes() {
        let contract = contract(Idempotency::Pure, &[]).expect("builds");
        for shape in [&contract.request, &contract.response] {
            let marker = shape.version_marker().expect("a version marker");
            assert_eq!(marker.path, VERSION_FIELD);
            assert!(marker.digest.is_hashed());
        }
    }

    #[test]
    fn an_append_only_operation_is_not_safe_to_retry_blindly() {
        assert!(!Idempotency::AppendOnly.retry_is_safe());
        assert!(Idempotency::ContentAddressed.retry_is_safe());
        assert!(!Idempotency::NotIdempotent.retry_is_safe());
    }
}
