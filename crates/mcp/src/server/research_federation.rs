//! Federated research, release, and cross-domain assurance contract handlers.

use super::*;

impl Server {
    pub(super) fn protocol_assurance_harness(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ProtocolAssuranceRequest")?;
        let receipt = crate::research_contracts::assess_protocol_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_policy::PROTOCOL_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "policy, protected-closure, partition, and unknown-cell gates remain explicit",
                "unknown simulation cells cannot become a pass",
                "the route performs no physical instrument or external data effect"
            ],
            "limitations": [
                "the route evaluates caller-supplied simulation counts and does not run a protocol or inspect hardware",
                "a passed assurance receipt is a preflight gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_multimodal_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedMultimodalAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_federated_multimodal_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_routing::FEDERATED_MULTIMODAL_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "institution-local raw modality bytes remain outside the federation envelope",
                "policy, protected-closure, harmonization, and semantic-loss gates remain explicit",
                "partial harmonization is unknown and cannot become a comparable pass"
            ],
            "limitations": [
                "the route evaluates typed modality manifests and does not read raw imaging or omics bytes",
                "a passed assurance receipt is a federation preflight gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_knowledge_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedKnowledgeGatewayRequest")?;
        let receipt = crate::research_contracts::admit_federated_knowledge_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_store::FEDERATED_KNOWLEDGE_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "canonical store manifests and permitted projections are exchanged without raw world records",
                "policy, protected-closure, schema, locality, and omission gates remain explicit",
                "an absent permitted projection remains unknown rather than an unrestricted export"
            ],
            "limitations": [
                "the route validates manifest metadata and does not open or transmit the indexed store",
                "a passed gateway receipt is an interoperability admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_lens_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedLensAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_federated_lens_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lens::FEDERATED_LENS_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "federated lens reports are exchanged by content digest rather than raw evidence",
                "a required lens that was not run remains unknown rather than negative evidence",
                "policy and protected-closure gates remain explicit and deterministic"
            ],
            "limitations": [
                "the route verifies caller-supplied lens report digests and does not reconstruct a lens report",
                "a passed assurance receipt is a representation gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_retrieval_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedRetrievalAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_federated_retrieval_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_fiber::FEDERATED_RETRIEVAL_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "returned source identities are canonicalized and missing sources remain visible",
                "an absent evidence derivation receipt cannot become a synthesized conclusion",
                "the route exchanges metadata and digests only and performs no raw-data export"
            ],
            "limitations": [
                "the route evaluates caller-supplied source identifiers and does not retrieve source bytes",
                "a passed retrieval receipt is an evidence admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn backends_federated_retrieval_synthesis_workflow(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedRetrievalSynthesisRequest6")?;
        let receipt =
            crate::research_contracts::run_backends_federated_retrieval_synthesis_workflow_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_backends::FEDERATED_RETRIEVAL_SYNTHESIS_WORKFLOW_FEATURE_ID,
            "contract_version": bioprism_backends::FEDERATED_RETRIEVAL_SYNTHESIS_WORKFLOW_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "checkpointed federated retrieval/synthesis stages deterministically qualify peers and candidate coverage",
                "replay, provenance, semantic profile, evidence state, quorum, policy, approval, locality, omission, uncertainty, and negative evidence remain explicit",
                "only a fully qualified aggregate coordination receipt emits an effect; unsafe or incomplete states block"
            ],
            "limitations": [
                "the fabric consumes digest-only peer summaries and does not retrieve sources or execute a backend",
                "a qualified coordination run is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn retrieval_synthesis_operations(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_retrieval_synthesis_operations_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": RETRIEVAL_SYNTHESIS_OPERATIONS_FEATURE_ID,
            "contract_version": RETRIEVAL_SYNTHESIS_OPERATIONS_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "prospective capacity, A2 authority, checkpoint, local assurance, policy, federation, and locality gates are deterministic",
                "admit, retrieve-local, synthesize, and checkpoint operation states preserve capacity, authority, omission, uncertainty, negative, and adversarial evidence",
                "qualified output is limited to permitted-summary exchange and local capability management; the tool never silently falls back",
                "the tool never performs network retrieval, moves raw preclinical data, dispatches instruments, asserts scientific truth, or makes clinical decisions"
            ],
            "limitations": [
                "the control plane operates caller-supplied local retrieval attestations and does not retrieve corpus bytes or independently validate scientific claims",
                "a qualified receipt is an operational admission and synthesis gate, not completed computation, biological validity, or clinical advice"
            ]
        }))
    }

    pub(super) fn scale_federation_trust_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_scale_federation_trust_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATION_TRUST_FEATURE_ID,
            "contract_version": FEDERATION_TRUST_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "peer and artifact coverage states are canonicalized and completely partitioned",
                "policy, protected closure, signed approval, authority, federation, replay, provenance, semantic-profile, revocation, and locality gates fail closed",
                "only digest-bound permitted-summary exchange and local capability-management effects can be qualified",
                "raw preclinical data remains local and no network, instrument, scientific, or clinical effect is performed"
            ],
            "limitations": [
                "the plane evaluates caller-supplied peer declarations and does not authenticate identities or contact remote institutions",
                "a qualified envelope is an exchange intent and coverage receipt, not a completed transfer or scientific conclusion"
            ]
        }))
    }

    pub(super) fn mcp_federated_quality_control(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_federated_quality_control_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_QUALITY_CONTROL_FEATURE_ID,
            "contract_version": FEDERATED_QUALITY_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "research-object, observation, site, and modality partitions are deterministic and complete",
                "stale, contradictory, unknown, unmeasured, omitted, negative, replay, provenance, policy, closure, federation, locality, and adversarial states remain observable",
                "federated release is fail-closed when any protected or authorization gate is incomplete",
                "qualified output is a digest-bound quality verification receipt only; the tool never moves raw data, executes instruments, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied aggregate quality declarations and does not inspect raw images, sequencing reads, instrument state, or human data",
                "a QualityVerdict7 is a cross-site assurance gate, not a biological conclusion, completed experiment, or clinical advice"
            ]
        }))
    }

    pub(super) fn mutation_federated_publication_release(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_mutation_publication_release_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": MUTATION_PUBLICATION_FEATURE_ID,
            "contract_version": MUTATION_PUBLICATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "run, artifact, site, signer, evidence, omission, uncertainty, and negative-result partitions are deterministic and complete",
                "stale, revoked, contradictory, unknown, speculative, missing, replay, provenance, policy, closure, federation, locality, and adversarial states remain observable",
                "A2 bounded publication effects fail closed until signed approval, protected closure, and cross-site gates qualify",
                "raw mutation-study data remains institution-local and the route never diagnoses, treats, triages, enrolls, or makes clinical decisions"
            ],
            "limitations": [
                "the copilot verifies caller-supplied validated run declarations and does not fetch artifact bytes, contact key registries, or independently reproduce analyses",
                "a SignedResearchObject3 receipt is a release-governance artifact, not proof of biological validity or a clinical conclusion"
            ]
        }))
    }

    pub(super) fn mutation_federated_continual_bounded_evolution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_mutation_federated_continual_bounded_evolution_assurance_json(arguments)?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_mutation::MUTATION_FEDERATED_EVOLUTION_FEATURE_ID,"contract_version":bioprism_mutation::MUTATION_FEDERATED_EVOLUTION_CONTRACT_VERSION,"receipt":receipt,"guarantees":["A1 mutation assurance verifies federated continual MutationEvolutionCandidate proposals against compatibility, benchmark, safety, evidence, replay, signature, policy, protected-closure, aggregate-only locality, and fail-closed gates","incompatible, benchmark-failed, unsafe, unsigned, contradicted, unknown, unmeasured, omitted, and negative proposals remain explicit","the route never mutates implementations, grants authority, exports raw preclinical data, or makes clinical decisions"],"limitations":["the harness evaluates caller-supplied mutation attestations and does not apply mutations, execute instruments, or publish releases","qualified output is a reversible verification preview, not scientific truth or release authorization"]}),
        )
    }

    pub(super) fn mutation_federated_resource_discovery_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_mutation_federated_resource_discovery_json(
                arguments,
            )?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_mutation::MUTATION_RESOURCE_DISCOVERY_FEATURE_ID,"contract_version":bioprism_mutation::MUTATION_RESOURCE_DISCOVERY_CONTRACT_VERSION,"receipt":receipt,"guarantees":["typed ResourceNeed4, endpoint, and peer attestations are deterministically partitioned into a QualifiedResourceSet8","capability fitness, protocol migration, evidence, provenance, replay, policy, locality, quorum, omission, uncertainty, negative, and adversarial governance states remain explicit","qualified effects are limited to local capability management and aggregate-only permitted-summary exchange; raw data never leaves the institution"],"limitations":["the control plane evaluates caller-supplied manifests and never contacts endpoints, executes instruments, or moves raw preclinical data","a qualified resource set is a bounded selection artifact, not evidence of scientific validity or a clinical decision"]}),
        )
    }

    pub(super) fn factory_prospective_evidence_surveillance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_factory_prospective_evidence_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": PROSPECTIVE_EVIDENCE_FEATURE_ID,
            "contract_version": PROSPECTIVE_EVIDENCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "candidate, selected, unresolved, blocked, overflow, study, and modality partitions are deterministic and complete",
                "stale, unavailable, low-relevance, unknown, speculative, contradicted, omitted, negative, replay, provenance, and capacity evidence remains observable",
                "prospective high-throughput admission is fail-closed under policy, protected-closure, aggregate-only, locality, and adversarial gates",
                "raw preclinical evidence remains local and the route never diagnoses, treats, triages, enrolls, or makes clinical decisions"
            ],
            "limitations": [
                "the harness verifies caller-supplied feed attestations and does not retrieve sources, inspect raw files, or independently reproduce a study",
                "a QualifiedEvidenceSet7 is an admission and release artifact, not proof of biological validity or a clinical conclusion"
            ]
        }))
    }

    pub(super) fn factory_federated_quality_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResearchObject4")?;
        let receipt =
            crate::research_contracts::operate_factory_federated_quality_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_factory::FEDERATED_QUALITY_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_factory::FEDERATED_QUALITY_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A1 federated continual quality workbench deterministically ranks observations and verifies peer quorum, modality closure, replay, provenance, policy, protected closure, and locality",
                "unknown, stale, contradicted, unmeasured, omitted, negative, adversarial, and missing-peer states remain explicit and fail closed",
                "qualified output is a content-addressed local researcher view with no raw-data federation or physical effect"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied quality attestations and does not inspect raw images, sequencing reads, or instruments",
                "a qualified verdict is release evidence for a bounded research workflow, not scientific truth or a clinical decision"
            ]
        }))
    }

    pub(super) fn fiber_federated_resource_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_fiber_federated_resource_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_RESOURCE_FEATURE_ID,
            "contract_version": FEDERATED_RESOURCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "candidate, ranking, selected, unresolved, blocked, missing, site, omission, uncertainty, and negative-evidence states are deterministic and complete",
                "capability, semantic-profile, availability, trust, provenance, replay, policy, approval, federation, aggregate-only, locality, and adversarial gates remain explicit",
                "qualified output is a read-only researcher resource view and all unsafe postures emit block:unsafe-release",
                "raw preclinical data remains local and the route never diagnoses, treats, triages, enrolls, or makes clinical decisions"
            ],
            "limitations": [
                "the workbench verifies caller-supplied resource attestations and does not fetch resources, contact registries, or execute providers",
                "a QualifiedResourceSet5 receipt is a capability-discovery aid, not proof of scientific validity or a clinical conclusion"
            ]
        }))
    }

    pub(super) fn fiber_federated_analysis_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_fiber_federated_analysis_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_fiber::FEDERATED_ANALYSIS_FEATURE_ID,
            "contract_version": bioprism_fiber::FEDERATED_ANALYSIS_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "analysis candidates and study/modality/model axes are deterministically ordered and partitioned into selected, unresolved, blocked, and missing states",
                "policy, protected closure, signed approval, federation, replay, provenance, aggregate-only locality, budget, and adversarial gates fail closed",
                "qualified effects are limited to local capability management and permitted-summary exchange; raw data and unsupported conclusions never leave the institution"
            ],
            "limitations": [
                "the control plane evaluates caller-supplied analysis attestations and does not execute statistical, causal, or ML models",
                "a qualified result is an operations admission receipt, not scientific truth, diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn docgraph_instrument_action_contract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_docgraph_instrument_action_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_docgraph::INSTRUMENT_ACTION_FEATURE_ID,
            "contract_version": bioprism_docgraph::INSTRUMENT_ACTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed instrument actions are deterministically ordered and partitioned into selected, unresolved, blocked, and missing states",
                "schema compatibility, evidence, replay, provenance, scope, endpoint, policy, protected-closure, locality, and adversarial checks fail closed",
                "the contract emits validation receipts only and cannot dispatch hardware, move raw data, or silently substitute defaults"
            ],
            "limitations": [
                "the primitive serializes caller-supplied action declarations and does not contact endpoints or execute instruments",
                "a qualified receipt is a compatibility artifact, not an execution authorization, scientific conclusion, or clinical advice"
            ]
        }))
    }

    pub(super) fn lens_provenance_signing_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_lens_provenance_signing_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lens::PROVENANCE_SIGNING_FEATURE_ID,
            "contract_version": bioprism_lens::PROVENANCE_SIGNING_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal artifacts and derivations are deterministically partitioned into selected, unresolved, blocked, and missing lineage states",
                "scope, signer-key coverage, replay, provenance, evidence, policy, protected-closure, federation, locality, and adversarial gates fail closed",
                "qualified output is limited to a bounded declared-tool invocation receipt and never authenticates keys or moves raw data"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied provenance attestations and does not contact key registries or verify cryptographic signatures",
                "a SignedProvenanceEnvelope3 is a lineage admission artifact, not scientific truth, publication acceptance, or clinical advice"
            ]
        }))
    }

    pub(super) fn services_multimodal_interpretation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::operate_services_multimodal_interpretation_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": MULTIMODAL_INTERPRETATION_FEATURE_ID,
            "contract_version": MULTIMODAL_INTERPRETATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "study, modality, and result partitions are deterministic and omission-aware",
                "replay, provenance, comparability, policy, protected-closure, signed-approval, and locality gates fail closed",
                "negative results, uncertainty, missing studies, missing modalities, and adversarial inputs remain observable",
                "qualified output is a local observation receipt only; the tool never reads raw data, exports data, dispatches instruments, or makes clinical decisions"
            ],
            "limitations": [
                "the engine interprets caller-supplied EvidenceBackedResult2 declarations and does not compute new scientific claims from raw bytes",
                "an InteractiveInterpretation1 receipt is a reproducible visualization/interpretation artifact, not a biological conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn services_context_compilation_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::operate_services_context_compilation_copilot_json(
            arguments,
        )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": CONTEXT_COMPILATION_COPILOT_FEATURE_ID,
            "contract_version": CONTEXT_COMPILATION_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed DecisionQuery4 inputs are deterministically ordered and partitioned into selected, unresolved, blocked, and missing states",
                "replay, provenance, influence, policy, protected-closure, signed-approval, authority, locality, aggregate-only, and adversarial gates fail closed",
                "omissions, uncertainty, contradictory evidence, and negative results remain explicit in the CertifiedDecisionSection3 artifact",
                "qualified output is limited to a bounded declared-tool invocation receipt; the copilot never retrieves evidence, moves raw data, or makes scientific or clinical decisions"
            ],
            "limitations": [
                "the copilot evaluates caller-supplied decision-context attestations and does not invoke external tools or independently validate scientific claims",
                "a CertifiedDecisionSection3 is a reproducible context-admission artifact, not a completed workflow, biological conclusion, or clinical advice"
            ]
        }))
    }

    pub(super) fn federated_continual_retrieval_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedContinualRetrievalRequest")?;
        let receipt = crate::research_contracts::synthesize_federated_continuum_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": FEDERATED_CONTINUAL_RETRIEVAL_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "continual source metadata is canonicalized and selected identities are explicit",
                "stale updates and absent synthesis anchors remain unknown rather than synthesized",
                "raw source and experimental content remain institution-local",
                "policy and protected-closure gates are deterministic and fail closed"
            ],
            "limitations": [
                "the route evaluates caller-supplied source metadata and does not retrieve source bytes",
                "a passed receipt is an evidence-refresh admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_context_compilation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ContextCompilationAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_context_compilation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_devplat::CONTEXT_COMPILATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "required context identities are canonicalized and missing context remains visible",
                "an absent derivation receipt cannot become a certified decision section",
                "policy and protected-closure gates remain explicit and deterministic",
                "raw institution-local records remain outside the federation envelope"
            ],
            "limitations": [
                "the route verifies caller-supplied context metadata and does not execute a compiler",
                "a passed receipt is a context certification gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_knowledge_representation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a KnowledgeRepresentationAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_knowledge_representation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_ops::KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "required fact identities are canonicalized and missing facts remain visible",
                "an absent derivation receipt cannot become an asserted knowledge projection",
                "policy and protected-closure gates remain explicit and deterministic",
                "raw institution-local knowledge remains outside the federation envelope"
            ],
            "limitations": [
                "the route verifies caller-supplied fact metadata and does not infer or retrieve facts",
                "a passed receipt is a representation admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_resource_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ResourceControlPlaneRequest")?;
        let receipt = crate::research_contracts::operate_resource_control_plane_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_weave::RESOURCE_CONTROL_PLANE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "resource identities and institution identities are canonicalized",
                "A2 approval, policy, and protected-closure gates remain explicit",
                "missing qualification remains unknown rather than executable",
                "raw institution-local resource records remain outside the federation envelope"
            ],
            "limitations": [
                "the route verifies caller-supplied qualification metadata and does not execute tools or instruments",
                "a passed receipt is an operations admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn weavelang_release_assurance(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a WeaveLangReleaseAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_weavelang_release_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_weavelang::WEAVELANG_RELEASE_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "release identity, evidence, provenance, locality, authority, and protected closure remain explicit",
                "incomplete release closure remains unknown rather than published",
                "raw preclinical data remains local and no signing key is handled by this route"
            ],
            "limitations": [
                "the route verifies caller-supplied release metadata and does not mint a cryptographic signature",
                "a passed receipt is a release admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn weavelang_federated_commons_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a WeavelangFederationRequest5")?;
        let receipt =
            crate::research_contracts::run_weavelang_federated_commons_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_weavelang::WEAVELANG_FEDERATED_COMMONS_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_weavelang::WEAVELANG_FEDERATED_COMMONS_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "WeaveLang capability declarations are ranked and partitioned deterministically across required federation axes",
                "schema identity, semantic profile, replay, provenance, signature, policy, protected closure, quorum, locality, omission, uncertainty, and negative evidence remain explicit",
                "unsafe, adversarial, or incomplete federation postures fail closed and raw research data remains local"
            ],
            "limitations": [
                "the harness verifies caller-supplied declarations and never opens federation connections or executes weave programs",
                "a qualified envelope is capability assurance evidence, not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn federated_mechanism_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a MechanismControlPlaneRequest")?;
        let receipt = crate::research_contracts::operate_mechanism_control_plane_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::MECHANISM_CONTROL_PLANE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "candidate identities are canonicalized and missing mechanism candidates remain visible",
                "A2 approval, policy, evidence, and protected-closure gates remain explicit",
                "incomplete mechanism evidence remains unknown rather than admitted",
                "raw institution-local evidence remains outside the federation envelope"
            ],
            "limitations": [
                "the route verifies caller-supplied candidate metadata and does not infer mechanisms or execute experiments",
                "a passed receipt is a mechanism-admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn megafactory_mechanism_exploration_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FederatedMechanismControlRequest")?;
        let receipt =
            crate::research_contracts::operate_megafactory_mechanism_exploration_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_megafactory::mechanism_exploration_federated_control_plane::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "ranked competing mechanisms and origin quorum remain deterministic",
                "A2 policy, approval, replay, locality, federation, and protected-closure gates remain explicit",
                "unknown, speculative, contradicted, omitted, and negative evidence remain visible",
                "qualified effects are limited to permitted summaries and local capability management"
            ],
            "limitations": [
                "the route validates caller-supplied mechanism metadata and does not infer mechanisms or execute experiments",
                "raw institution-local factor tables never cross the federation boundary",
                "a passed receipt is a governed mechanism-operation gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn federated_mechanism_gateway(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a MechanismGatewayRequest")?;
        let receipt = crate::research_contracts::admit_mechanism_gateway_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_fiber::MECHANISM_GATEWAY_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "source and target profiles plus interoperability profile are explicit",
                "missing candidate projections remain unknown rather than interoperable",
                "policy, locality, protected-closure, and projection gates are deterministic",
                "raw institution-local mechanism evidence remains outside the federation envelope"
            ],
            "limitations": [
                "the route validates projected metadata and does not move or transform source records",
                "a passed receipt is an interoperability admission gate, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn evidence_surveillance_copilot(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceFeedRequest")?;
        let receipt = crate::research_contracts::run_evidence_surveillance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::EVIDENCE_SURVEILLANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "single-study feed sources are deterministically ordered",
                "stale, contradictory, missing, and protected evidence remains omitted or uncertain",
                "negative results are preserved in the qualified evidence set",
                "raw source payloads remain institution-local"
            ],
            "limitations": [
                "the copilot qualifies evidence metadata and does not infer a clinical conclusion",
                "unknown evidence cannot be promoted by an MCP caller"
            ]
        }))
    }

    pub(super) fn scope_federated_evidence_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a Scope EvidenceControlRequest6")?;
        let receipt =
            crate::research_contracts::run_scope_federated_evidence_control_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_scope::SCOPE_FEDERATED_EVIDENCE_CONTROL_FEATURE_ID,
            "contract_version": bioprism_scope::SCOPE_FEDERATED_EVIDENCE_CONTROL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "bounded high-throughput EvidenceFeed3 batches are checkpointed and ranked deterministically",
                "peer quorum, overflow, replay, provenance, policy, approval, scope, aggregate-only, locality, and negative evidence remain explicit",
                "only a qualified coordination receipt emits an effect; no sources, raw observations, tools, or instruments are accessed"
            ],
            "limitations": [
                "the control plane evaluates caller-supplied summaries and does not retrieve or inspect raw evidence",
                "a qualified coordination receipt is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn scope_federated_commons_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ScopeFederationGatewayRequest7")?;
        let receipt =
            crate::research_contracts::run_scope_federated_interoperability_gateway_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_scope::SCOPE_FEDERATED_INTEROPERABILITY_FEATURE_ID,
            "contract_version": bioprism_scope::SCOPE_FEDERATED_INTEROPERABILITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "scope summaries are exchanged only after deterministic schema, semantic-profile, signature, checkpoint, policy, and locality checks",
                "migration, omission, uncertainty, negative evidence, and missing peers remain explicit",
                "raw preclinical data remains institution-local and non-qualified exchanges fail closed"
            ],
            "limitations": [
                "the gateway consumes caller-supplied digest-bound manifests and does not fetch or move source payloads",
                "a qualified scope exchange is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn hubapi_federated_experiment_design_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ExperimentObjective4")?;
        let receipt =
            crate::research_contracts::run_hubapi_experiment_design_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_hubapi::EXPERIMENT_DESIGN_ASSURANCE_FEATURE_ID,
            "contract_version": bioprism_hubapi::EXPERIMENT_DESIGN_ASSURANCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed experiment-design candidates are checked for scope, modalities, controls, power, evidence, replay, provenance, and peer closure",
                "missing, uncertain, contradictory, negative, policy, federation, and locality evidence remains explicit",
                "the assurance harness is verification-only and always emits a fail-closed release effect"
            ],
            "limitations": [
                "the harness does not design, execute, or dispatch an experiment or instrument protocol",
                "a qualified assurance receipt is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn fabric_experiment_design_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a FabricExperimentDesignContractRequest4")?;
        let receipt =
            crate::research_contracts::run_fabric_experiment_design_contract_model_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_fabric::EXPERIMENT_DESIGN_CONTRACT_MODEL_FEATURE_ID,
            "contract_version": bioprism_fabric::EXPERIMENT_DESIGN_CONTRACT_MODEL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "exact, additive, and breaking schema compatibility are deterministic and migration witnesses are explicit",
                "candidate evidence, replay, provenance, policy, protected-closure, locality, and negative results remain typed",
                "the contract model emits observation-only effects and never dispatches an experiment"
            ],
            "limitations": [
                "the model consumes caller-supplied envelopes and does not generate or execute an experiment design",
                "a compatible contract is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn prism_laboratory_integration_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InstrumentActionRequest4")?;
        let receipt =
            crate::research_contracts::run_prism_laboratory_integration_copilot_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_prism::LABORATORY_INTEGRATION_COPILOT_FEATURE_ID,
            "contract_version": bioprism_prism::LABORATORY_INTEGRATION_COPILOT_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "A4 instrument actions require signed preflight, interlock, emergency-stop, provenance, replay, policy, federation, budget, and locality gates",
                "missing or unsafe gates emit explicit omission and uncertainty receipts and block release",
                "qualified output is only a bounded declared-tool invocation request; hardware execution remains in a separate gateway"
            ],
            "limitations": [
                "the copilot never contacts an instrument, consumes material, or executes a protocol directly",
                "a qualified invocation receipt is not a clinical decision or treatment instruction"
            ]
        }))
    }

    pub(super) fn scale_interpretation_visualization_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceBackedResult4")?;
        let receipt =
            crate::research_contracts::run_scale_interpretation_visualization_assurance_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_scale::INTERPRETATION_VISUALIZATION_FEATURE_ID,
            "contract_version": bioprism_scale::INTERPRETATION_VISUALIZATION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "multimodal interpretations require comparability, evidence, provenance, replay, policy, protected-closure, signed-approval, adversarial, and locality gates",
                "missing, uncertain, contradictory, negative, and omitted evidence remains explicit and non-qualified states block",
                "the harness is read-only and never renders raw data, fits models, moves payloads, or makes clinical decisions"
            ],
            "limitations": [
                "the harness consumes caller-supplied interpretation metadata and does not infer scientific truth",
                "a qualified interpretation receipt is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn scale_interpretation_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceBackedResult2")?;
        let receipt =
            crate::research_contracts::run_scale_interpretation_interoperability_gateway_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_scale::INTERPRETATION_INTEROPERABILITY_FEATURE_ID,
            "contract_version": bioprism_scale::INTERPRETATION_INTEROPERABILITY_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "versioned interpretation exchange requires protocol, schema, semantic-profile, evidence, provenance, replay, authorization, policy, and aggregate-only locality gates",
                "schema migration, semantic loss, negative evidence, and unresolved states remain explicit",
                "the gateway exports only permitted aggregate artifacts and never moves raw experimental data"
            ],
            "limitations": [
                "the gateway negotiates caller-supplied metadata and does not render or infer interpretations",
                "a qualified exchange receipt is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn stress_federated_multimodal_ingestion_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = crate::research_contracts::run_stress_federated_multimodal_ingestion_contract_model_json(arguments)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_stress::FEDERATED_MULTIMODAL_INGESTION_FEATURE_ID,
            "contract_version": bioprism_stress::FEDERATED_MULTIMODAL_INGESTION_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "typed modality manifests are deterministically harmonized with explicit required-modality and peer closure",
                "missing, unresolved, contradicted, quality, semantic-loss, replay, provenance, policy, locality, and negative-result states remain visible",
                "the A1 contract model never reads raw imaging or omics bytes, exports payloads, or makes clinical decisions"
            ],
            "limitations": [
                "the model validates caller-supplied metadata and does not transform raw arrays or matrices",
                "a qualified harmonized object is a compatibility contract, not a biological conclusion"
            ]
        }))
    }

    pub(super) fn oracle_interoperability_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ExternalCapabilityRequest1")?;
        let receipt =
            crate::research_contracts::run_oracle_interoperability_research_workbench_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oracle::INTEROPERABILITY_WORKBENCH_FEATURE_ID,
            "contract_version": bioprism_oracle::INTEROPERABILITY_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "external research capabilities are compared by target identity, schema, standards, support, evidence, and local-only constraints",
                "semantic loss, negative evidence, mismatches, disabled providers, and policy failures remain explicit",
                "the A0 workbench is read-only and never invokes external providers or grants execution authority"
            ],
            "limitations": [
                "the workbench negotiates caller-supplied capability manifests and does not execute or benchmark providers",
                "a qualified integration is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn atlashub_provenance_signing_inference_engine(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ProvenanceSigningRequest1")?;
        let receipt =
            crate::research_contracts::run_atlashub_provenance_signing_inference_engine_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlashub::PROVENANCE_SIGNING_INFERENCE_FEATURE_ID,
            "contract_version": bioprism_atlashub::PROVENANCE_SIGNING_INFERENCE_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "artifact lineage is content-addressed and signer-bound to a deterministic envelope",
                "unknown evidence, non-local artifacts, negative results, policy gaps, and protected-closure gaps remain explicit",
                "the A0 inference engine never exports raw payloads and does not invoke a signing provider"
            ],
            "limitations": [
                "the envelope carries digest-bound signature evidence; deployment cryptographic signing is a transport concern",
                "a qualified envelope is not diagnosis, treatment, triage, enrollment, or clinical advice"
            ]
        }))
    }

    pub(super) fn conformance_retrieval_synthesis_contract_model(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ScopedRetrievalQuery3")?;
        let receipt =
            crate::research_contracts::run_conformance_retrieval_synthesis_contract_model_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_conformance::RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_FEATURE_ID,
            "contract_version": bioprism_conformance::RETRIEVAL_SYNTHESIS_CONTRACT_MODEL_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "prospective high-throughput retrieval and synthesis schemas are negotiated with deterministic candidate ordering",
                "compatibility, migration, unresolved evidence, omissions, negative results, provenance, replay, semantic loss, policy, protected closure, and locality remain explicit",
                "raw research data stays local and any non-compatible or unsafe disposition fails closed"
            ],
            "limitations": [
                "the contract model consumes caller-supplied metadata and never fetches or interprets source payloads",
                "a compatible contract is not a scientific conclusion or clinical advice"
            ]
        }))
    }

    pub(super) fn multimodal_retrieval_synthesis(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceSynthesisRequest")?;
        let receipt = crate::research_contracts::compile_evidence_synthesis_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::RETRIEVAL_SYNTHESIS_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "study scope and comparability profile are typed inputs",
                "incompatible, stale, protected, and contradictory evidence remains visible",
                "negative evidence is retained in the synthesis artifact",
                "raw source payloads remain institution-local"
            ],
            "limitations": [
                "the contract model does not infer a clinical or treatment conclusion",
                "missing modality coverage remains unknown rather than completed"
            ]
        }))
    }

    pub(super) fn foundation_mechanism_exploration_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a MechanismExplorationAssuranceRequest")?;
        let receipt =
            crate::research_contracts::run_foundation_mechanism_exploration_assurance_json(
                request,
            )?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_foundation::FOUNDATION_MECHANISM_EXPLORATION_ASSURANCE_FEATURE_ID, "receipt": receipt, "guarantees": ["A1 assurance evaluates prospective high-throughput mechanism candidates against typed evidence, provenance, artifact, comparability, baseline, replay, policy, approval, protected-closure, and capacity gates", "unknown, speculative, contradicted, below-threshold, omitted, negative, and required-but-not-admitted candidates remain explicit", "non-qualified portfolios fail closed with block:unsafe-release and raw preclinical data remains local"], "limitations": ["the assurance harness is not a clinical decision system", "operators remain responsible for independent replication, causal interpretation, baseline selection, and release governance"]}),
        )
    }

    pub(super) fn atlashub_mechanism_exploration_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be an Atlashub MechanismExplorationAssuranceRequest",
        )?;
        let receipt =
            crate::research_contracts::run_atlashub_mechanism_exploration_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_atlashub::MECHANISM_EXPLORATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "A1 assurance evaluates prospective high-throughput mechanism candidates against typed evidence, provenance, artifact, comparability, baseline, replay, policy, approval, protected-closure, and capacity gates",
                "unknown, speculative, contradicted, below-threshold, omitted, negative, and required-but-not-admitted candidates remain explicit",
                "non-qualified portfolios fail closed with block:unsafe-release and raw preclinical data remains local"
            ],
            "limitations": [
                "the assurance harness is not a clinical decision system",
                "operators remain responsible for independent replication, causal interpretation, baseline selection, and release governance"
            ]
        }))
    }

    pub(super) fn dataops_provenance_signing_workflow_fabric(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt =
            crate::research_contracts::run_dataops_provenance_signing_workflow_fabric_json(
                arguments,
            )?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_dataops::PROVENANCE_SIGNING_WORKFLOW_FABRIC_FEATURE_ID,"receipt":receipt,"guarantees":["A1 workflow fabric verifies high-throughput artifact derivation lineage, detached signature attestations, root and replay identity, capacity, policy, protected closure, approval, aggregate-only locality, and adversarial gates","missing parents, cycles, invalid signatures, root drift, unknown or negative evidence, omissions, and unresolved closure remain explicit","non-qualified workflows fail closed with block:unsafe-release; raw preclinical data never leaves its institution"],"limitations":["the fabric validates caller-supplied attestations and does not sign, upload, execute, or move artifacts","the capability is not a clinical decision system"]}),
        )
    }

    pub(super) fn oraclex_publication_release(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a PublicationReleaseRequest")?;
        let receipt = crate::research_contracts::run_oraclex_publication_release_json(request)?;
        Ok(
            json!({"ok": true, "schema": "aurora-research-contract/1.0", "feature_id": bioprism_oraclex::publication_release_contract_model::feature_id(), "receipt": receipt, "guarantees": ["A2 release admission requires typed artifact, provenance, workflow replay, evaluation baseline, standards, reproducibility, policy, protected closure, signed authority, federation permission, and raw-locality gates", "unknown and speculative evidence become conditional review, contradicted evidence and failed gates block release, and negative findings remain in the signed receipt", "release payloads are content-addressed and digest-only across the federation boundary; contract migration records semantic loss and preserves replay identity"], "limitations": ["the contract model does not mint signatures, upload payloads, or perform publication hosting", "the capability is not a clinical decision system and does not process human-subject or clinical-source data"]}),
        )
    }

    pub(super) fn oraclex_interpretation_inference(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceBackedResult3")?;
        let receipt =
            crate::research_contracts::run_oraclex_interpretation_inference_json(request)?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_oraclex::INTERPRETATION_INFERENCE_FEATURE_ID,"receipt":receipt,"guarantees":["candidate ordering and qualified/unresolved/blocked/incomparable partitions are deterministic","study and modality closure, comparability, replay, provenance, policy, federation, locality, budget, omission, uncertainty, negative, and adversarial gates remain explicit","the InteractiveInterpretation1 artifact is content-addressed and release-blocked by default"],"limitations":["the engine ranks caller-supplied declarations and does not render, infer biology, access raw data, or make clinical decisions","qualified panels remain subject to independent scientific and governance review"]}),
        )
    }

    pub(super) fn oraclex_performance_reliability_interoperability_gateway(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a CapabilityWorkload4")?;
        let receipt = crate::research_contracts::run_oraclex_performance_reliability_interoperability_gateway_json(request)?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_oraclex::PERFORMANCE_RELIABILITY_INTEROPERABILITY_FEATURE_ID,"receipt":receipt,"guarantees":["A2 federated reliability exchange is deterministic, aggregate-only, and replay-bound","retry, timeout, duplicate-event, migration, omission, uncertainty, adversarial, and negative evidence remain explicit","unsigned, unpermitted, non-local, incomplete, or over-budget invocations fail closed","the capability is not a clinical decision system and excludes human-subject and clinical-source data"],"limitations":["the gateway evaluates caller-supplied telemetry and does not execute workloads, sign artifacts, or move raw data","qualified exchange remains subject to institutional governance and independent reliability review"]}),
        )
    }

    pub(super) fn oraclex_statistical_analysis_research_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an AnalysisQuestion4")?;
        let receipt =
            crate::research_contracts::run_oraclex_statistical_analysis_research_workbench_json(
                request,
            )?;
        Ok(
            json!({"ok":true,"schema":"aurora-research-contract/1.0","feature_id":bioprism_oraclex::STATISTICAL_ANALYSIS_RESEARCH_WORKBENCH_FEATURE_ID,"receipt":receipt,"guarantees":["A1 analysis qualification is deterministic, replay-bound, and local","identification, comparability, quality, evidence, provenance, omission, uncertainty, contradiction, negative, and adversarial states remain explicit","the workbench never executes models, exports raw arrays, or makes clinical decisions"],"limitations":["the workbench validates caller-supplied attestations and does not fit models or publish results","qualified analysis remains subject to independent statistical and institutional review"]}),
        )
    }

    pub(super) fn interweave_frontier_control(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InterweaveControlPlaneRequest")?;
        let receipt = crate::research_contracts::run_interweave_frontier_control_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": INTERWEAVE_FRONTIER_FEATURE_ID(),
            "receipt": receipt,
            "guarantees": [
                "A2 prospective high-throughput admission is deterministic, digest-bound, and gated by protocol pinning, peer quorum, policy, protected closure, signed approval, capacity, locality, and replay identity",
                "unknown/speculative evidence remains conditional, contradicted evidence and failed gates block, incompatible peers are explicit, and negative results remain retained",
                "control-plane artifacts expose only aggregate federation metadata while raw preclinical data remains institution-local"
            ],
            "limitations": [
                "the contract admits and records work but does not execute jobs, mint signatures, or move raw data",
                "the capability is not a clinical decision system and excludes human-subject and clinical-source data"
            ]
        }))
    }

    pub(super) fn influence_federated_continual_interpretation(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvidenceBackedResult4")?;
        let receipt =
            crate::research_contracts::run_federated_continual_interpretation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_influence::FEDERATED_CONTINUAL_INTERPRETATION_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "sound dynamic-range, contraction, abstract-interpretation, and exact-removal methods remain visible per factor",
                "federated peers are version-negotiated by signed capability and quorum while raw factor tables remain institution-local",
                "unknown influence, protected omissions, contradictory or negative evidence, migration loss, and approval requirements remain explicit",
                "continual epochs bind replay and content-addressed interactive interpretation artifacts"
            ],
            "limitations": [
                "the gateway computes caller-supplied local regions and does not fetch remote data or render a browser UI",
                "a qualified influence view is not a biological, clinical, treatment, or causal decision",
                "independent replication, institutional authorization, and external transport conformance remain required"
            ]
        }))
    }

    pub(super) fn influence_local_evidence_surveillance_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InfluenceEvidenceFeedRequest")?;
        let receipt = crate::research_contracts::operate_influence_local_evidence_surveillance_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": INFLUENCE_LOCAL_EVIDENCE_SURVEILLANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "A0 local evidence surveillance deterministically ranks caller-supplied preclinical observations and emits a typed qualified evidence set",
                "unsupported, stale, contradictory, unknown, omitted, policy-denied, locality, and replay failures remain explicit and fail closed",
                "the artifact is content-addressed, provenance-bound, read-only, and preserves negative evidence"
            ],
            "limitations": [
                "the gateway does not fetch external literature, infer unsupported biological conclusions, or perform clinical decisions",
                "raw preclinical source payloads remain caller/institution local and independent replication remains required"
            ]
        }))
    }

    pub(super) fn safety_prospective_laboratory_integration_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an InstrumentActionRequest3")?;
        let receipt = crate::research_contracts::operate_safety_prospective_laboratory_integration_assurance_json(request)?;
        let typed: InstrumentActionReceipt7 =
            serde_json::from_value(receipt.clone()).map_err(|error| {
                format!("safety laboratory-integration receipt serialization failed: {error}")
            })?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": PROSPECTIVE_LABORATORY_INTEGRATION_FEATURE_ID,
            "contract_version": PROSPECTIVE_LABORATORY_INTEGRATION_CONTRACT_VERSION,
            "receipt": receipt,
            "disposition": typed.disposition,
            "guarantees": [
                "A1 prospective high-throughput laboratory actions are verified before any physical gateway and every unsafe posture fails closed",
                "interlock, emergency-stop, deterministic, signed-approval, replay, provenance, policy, protected-closure, federation, budget, locality, and compensation evidence remains explicit",
                "qualified output is a content-addressed verification receipt with no hardware effect"
            ],
            "limitations": [
                "the harness validates attestations only and never sends commands, schedules material, or runs instruments",
                "preclinical research only; no human-subject or clinical-source data and no clinical decisions"
            ]
        }))
    }

    pub(super) fn multimodal_knowledge_workflow(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a ClaimsWorkflowRequest")?;
        let receipt = crate::research_contracts::run_knowledge_workflow_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::KNOWLEDGE_WORKFLOW_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "study and claim identities are canonicalized",
                "workflow stages are explicit and replayable",
                "missing derivation and claim closure remain unknown"
            ],
            "limitations": [
                "the fabric orchestrates typed claims and does not infer biological truth",
                "a passed knowledge world is not a clinical or treatment decision"
            ]
        }))
    }

    pub(super) fn governance_experiment_design_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an ExperimentObjective")?;
        let receipt =
            crate::research_contracts::run_governance_experiment_design_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_governance::experiment_design_assurance::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "power, variance, required-factor, evidence, provenance, replay, and arm closure are deterministic",
                "policy, protected-closure, signed approval, federation, locality, budget, and adversarial gates fail closed",
                "underpowered, contradicted, unknown, omitted, negative, and missing-factor evidence remains visible"
            ],
            "limitations": [
                "the harness validates design attestations and does not schedule animals, consume material, or execute instruments",
                "approval is a governance receipt, not a clinical or diagnostic decision"
            ]
        }))
    }

    pub(super) fn governance_federated_continual_interpretation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedContinualInterpretationAssuranceRequest",
        )?;
        let receipt =
            crate::research_contracts::assure_governance_federated_continual_interpretation_json(
                request,
            )?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_governance::federated_continual_interpretation_assurance::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "interpretation and visualization candidates are deterministically ordered and linked to declared results",
                "unknown, unmeasured, contradicted, omitted, uncertain, and negative evidence remain visible",
                "protected closure, policy, signed approval, replay, provenance, baseline, and locality gates fail closed",
                "the only effect is block:unsafe-release until a separate release board accepts the evidence"
            ],
            "limitations": [
                "the harness verifies caller-supplied metadata and does not inspect raw imaging or omics bytes",
                "it does not render visualizations, infer mechanisms, execute tools, or make clinical decisions"
            ]
        }))
    }

    pub(super) fn mcp_bounded_evolution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be an EvolutionAssuranceRequest")?;
        let receipt = crate::research_contracts::assure_bounded_evolution_json(request)?;
        let parsed =
            crate::research_contracts::validate_bounded_evolution_assurance_json(&receipt)?;
        let serialized = serde_json::to_value(parsed)
            .map_err(|error| format!("cannot serialize bounded evolution assurance: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": crate::evolution_assurance::FEATURE_ID,
            "receipt": serialized,
            "guarantees": [
                "the adapter receipt digest, replay identity, canonical ordering, policy, approval, locality, and protected-closure gates are independently recomputed",
                "adversarial containment forbids execution or deployment effects and retains negative evidence",
                "missing checks produce unknown rather than pass; failed release gates produce block:unsafe-release",
                "the tool emits assurance evidence only and never executes, signs, or deploys a candidate"
            ],
            "limitations": [
                "the harness validates caller-supplied evidence and does not replace independent site replication or institutional release governance",
                "benchmark quality, signer identity, and external key validity remain accountable to the research consortium"
            ]
        }))
    }

    pub(super) fn research_release_validate(&self, arguments: &Value) -> Result<Value, String> {
        let receipt = arguments
            .get("receipt")
            .ok_or("receipt is required and must be a serialized ResearchReleaseReceipt")?;
        let validated = crate::research_contracts::validate_research_release_receipt_json(receipt)?;
        let serialized = serde_json::to_value(&validated)
            .map_err(|error| format!("cannot serialize validated research release: {error}"))?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_services::RESEARCH_RELEASE_FEATURE_ID,
            "receipt": serialized,
            "guarantees": [
                "signed research-object metadata, provenance, policy, and localization are present",
                "raw experimental bytes are not accepted in the federation envelope",
                "the receipt remains independently verifiable with the origin public key"
            ],
            "limitations": [
                "this MCP route validates metadata and does not hold or use a private signing key",
                "cryptographic signature verification requires the receiving institution's public-key verifier"
            ]
        }))
    }

    pub(super) fn research_release_batch_validate(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let receipt = arguments
            .get("receipt")
            .ok_or("receipt is required and must be a serialized ResearchReleaseBatchReceipt")?;
        let validated =
            crate::research_contracts::validate_research_release_batch_receipt_json(receipt)?;
        let serialized = serde_json::to_value(&validated).map_err(|error| {
            format!("cannot serialize validated research release batch: {error}")
        })?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_services::RESEARCH_RELEASE_BATCH_FEATURE_ID,
            "receipt": serialized,
            "guarantees": [
                "published and blocked releases are retained with canonical release ordering and per-release reasons",
                "a published entry carries a release digest while a blocked entry cannot masquerade as published",
                "the route validates the signed batch artifact without accepting private keys or raw source data"
            ],
            "limitations": [
                "signing and federation effects occur only in the institution-local services API",
                "cryptographic signature verification still requires each origin public-key verifier"
            ]
        }))
    }

    pub(super) fn federated_publication_release_inference(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments.get("request").ok_or(
            "request is required and must be a FederatedPublicationReleaseInferenceRequest",
        )?;
        let receipt =
            crate::research_contracts::run_federated_publication_release_inference_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_services::FEDERATED_PUBLICATION_RELEASE_INFERENCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "release attestations are ranked deterministically across federated continual origins",
                "unknown, speculative, contradicted, omitted, negative, policy, locality, and replay states remain explicit and fail closed",
                "the route emits digest-only recommendations; signing and publication remain separate authorized service effects"
            ],
            "limitations": [
                "the inference engine does not sign, publish, dereference, or move raw experimental data",
                "a qualified recommendation is not a scientific conclusion or a clinical decision"
            ]
        }))
    }

    pub(super) fn instrument_preflight(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized InstrumentPreflightRequest")?;
        let receipt = crate::research_contracts::instrument_preflight_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_lab::INSTRUMENT_PREFLIGHT_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "actions are sorted and content-addressed before any physical effect",
                "missing interlocks, missing evidence, budget overflow, emergency stop, and non-allow policy fail closed",
                "the route performs no instrument, network, filesystem, or material effect"
            ],
            "limitations": [
                "a ready receipt is a signed-preflight input, not evidence that hardware executed",
                "actual instrument execution requires a separate institution-local A3 gateway and human authorization"
            ]
        }))
    }

    pub(super) fn mutation_knowledge_federated_control(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a MutationKnowledgeFederatedControlRequest")?;
        let receipt =
            crate::research_contracts::run_mutation_knowledge_federated_control_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_mutation::knowledge_representation_federated_control_plane::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "mutation knowledge candidates are ranked deterministically with origin quorum and continual checkpoint identity",
                "policy, protected-closure, signed-approval, locality, replay, oracle, and evidence gates fail closed",
                "negative, contradicted, unknown, speculative, and omitted states remain explicit in the receipt"
            ],
            "limitations": [
                "the control plane exchanges lineage and digest metadata only; raw experimental data remains institution-local",
                "admission is a governed research automation outcome, not a clinical conclusion or treatment decision"
            ]
        }))
    }

    pub(super) fn multimodal_harmonize(&self, arguments: &Value) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized MultimodalHarmonizationRequest")?;
        let object = crate::research_contracts::harmonize_multimodal_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_adapter::MULTIMODAL_HARMONIZATION_FEATURE_ID,
            "receipt": object,
            "guarantees": [
                "modality ordering and feature alignment are deterministic",
                "unit and coordinate conflicts are rejected before artifact creation",
                "missing required modalities and missing QC digests remain explicit limitations",
                "raw imaging and omics data remain institution-local"
            ],
            "limitations": [
                "the route harmonizes caller-supplied manifests and does not inspect raw modality bytes",
                "comparability is a manifest-level gate, not a biological validity claim"
            ]
        }))
    }

    pub(super) fn mcp_multimodal_ingestion_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized MultimodalIngestionRequest")?;
        let receipt =
            crate::research_contracts::assure_multimodal_ingestion_assurance_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": crate::multimodal_ingestion_assurance::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "modality attestations and peer summaries are canonicalized before release",
                "raw bytes remain institution-local and only aggregate signed metadata crosses federation",
                "unknown, unmeasured, contradictory, omitted, and adversarial states remain explicit"
            ],
            "limitations": [
                "the MCP route verifies caller-supplied manifests and does not inspect or harmonize raw bytes",
                "a qualified ingestion receipt is a research-data readiness signal, not a biological or clinical conclusion"
            ]
        }))
    }

    pub(super) fn weavelang_computational_execution_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized ResearchWorkflowSpec")?;
        let receipt =
            crate::research_contracts::assure_weavelang_computational_execution_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_weavelang::COMPUTATIONAL_EXECUTION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "workflow nodes are deterministically ordered and graph defects are explicit",
                "only local read, local computation, and local-artifact effects are admitted",
                "unknown, unmeasured, contradictory, omitted, budget, and adversarial states fail closed"
            ],
            "limitations": [
                "the route verifies a caller-declared graph and never dispatches tools or processes",
                "a qualified execution receipt is not a scientific or clinical conclusion"
            ]
        }))
    }

    pub(super) fn mcp_knowledge_representation_contract(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized KnowledgeRepresentationRequest")?;
        let receipt =
            crate::research_contracts::model_mcp_knowledge_representation_contract_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": crate::knowledge_representation_contract_model::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "claim identities and peer summaries are canonically ordered and partitioned",
                "only semantically comparable, signed aggregate metadata is considered qualified",
                "unknown, speculative, contradictory, omitted, and denied claims remain explicit"
            ],
            "limitations": [
                "the route validates caller-supplied claim attestations and does not infer relations from raw studies",
                "a qualified knowledge-world envelope is not a biological, diagnostic, or clinical conclusion"
            ]
        }))
    }

    pub(super) fn registry_multimodal_scale_frontier_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized RegistryScaleWorkload")?;
        let receipt = crate::research_contracts::assure_registry_scale_frontier_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_registry::REGISTRY_SCALE_FRONTIER_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "multimodal study order, capacity totals, omission sets, and effect receipts are deterministic",
                "missing modality closure, unknown or unmeasured workload state, contradiction, policy denial, capacity overflow, and adversarial events fail closed",
                "raw registry data remains institution-local and only typed capacity metadata is returned",
                "qualified output measures declared registry capacity and is never a biological or clinical conclusion"
            ],
            "limitations": [
                "the route evaluates caller-supplied workload manifests and does not inspect raw registry bytes or schedule work",
                "capacity qualification is a release gate and requires independent benchmark, replication, and institutional governance evidence"
            ]
        }))
    }

    pub(super) fn registry_knowledge_representation_assurance(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized ScopedResearchClaims")?;
        let receipt =
            crate::research_contracts::assure_registry_knowledge_representation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_registry::KNOWLEDGE_REPRESENTATION_ASSURANCE_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "scoped claim identities, peer summaries, evidence states, and effect receipts are canonical and partitioned",
                "minimum peer quorum, semantic profile, signed aggregate-only locality, provenance/replay, policy, approval, and protected-closure gates are explicit",
                "unknown, speculative, contradicted, omitted, negative, and adversarial evidence cannot be promoted to a qualified knowledge world",
                "raw study payloads remain institution-local and the route never infers biology or makes a clinical decision"
            ],
            "limitations": [
                "the assurance harness verifies caller-supplied claims and peer summaries; it does not infer relations from raw studies",
                "a qualified typed world is a release-readiness receipt and still requires independent replication and governance review"
            ]
        }))
    }

    pub(super) fn registry_replication_workbench(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized ClaimAndProtocol1")?;
        let receipt =
            crate::research_contracts::operate_registry_replication_workbench_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": REPLICATION_WORKBENCH_FEATURE_ID,
            "contract_version": REPLICATION_WORKBENCH_CONTRACT_VERSION,
            "receipt": receipt,
            "guarantees": [
                "claim, protocol, evidence, omission, uncertainty, and negative-result axes are deterministic and partitioned",
                "replication, independence, provenance, replay, policy, protected-closure, researcher-authorization, locality, and adversarial gates fail closed",
                "qualified output is a read-only local replication record; raw study data is never moved or executed",
                "null and negative results remain first-class evidence rather than being treated as absent"
            ],
            "limitations": [
                "the workbench evaluates caller-supplied attestations and does not run protocols or infer biological truth",
                "a qualified record is release evidence for review and not a clinical decision or treatment recommendation"
            ]
        }))
    }

    pub(super) fn oraclex_context_compilation_research_copilot(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized DecisionQuery")?;
        let receipt = crate::research_contracts::compile_oraclex_context_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_oraclex::context_compilation_research_copilot::FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "fact and peer identities are canonically ordered and partitioned before a decision section is emitted",
                "only scoped supported local facts and signed aggregate-only peer summaries enter a bounded declared-tool plan",
                "unknown, speculative, contradictory, omitted, uncertain, policy-denied, approval-missing, and adversarial states fail closed",
                "raw evidence stays institution-local and the MCP route never invokes tools or makes a biological or clinical conclusion"
            ],
            "limitations": [
                "the copilot compiles caller-supplied attestations and plans declared tools; it does not retrieve evidence or execute a workflow",
                "a qualified section is an automation-readiness receipt and still requires downstream tool-specific validation and independent research governance"
            ]
        }))
    }

    pub(super) fn ops_context_compilation_federated_control_plane(
        &self,
        arguments: &Value,
    ) -> Result<Value, String> {
        let request = arguments
            .get("request")
            .ok_or("request is required and must be a serialized DecisionQuery")?;
        let receipt = crate::research_contracts::operate_ops_context_compilation_json(request)?;
        Ok(json!({
            "ok": true,
            "schema": "aurora-research-contract/1.0",
            "feature_id": bioprism_ops::CONTEXT_COMPILATION_CONTROL_FEATURE_ID,
            "receipt": receipt,
            "guarantees": [
                "high-throughput context identities, peer summaries, capacity witnesses, and effects are deterministic and partitioned",
                "policy, signed approval, protected closure, federation, locality, queue, active-run, and adversarial gates fail closed",
                "unknown, speculative, contradictory, missing, omitted, uncertain, and negative evidence remains visible",
                "the operator route emits only typed control metadata and never schedules processes or moves raw research data"
            ],
            "limitations": [
                "the control plane evaluates caller-declared queue and context manifests; it does not execute or transport a workflow",
                "qualified admission remains subject to institution-local scheduler, instrument, security, and independent validation gates"
            ]
        }))
    }
}
