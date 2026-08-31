//! The invariants each §25 module states, one test per claim.

use bioprism_bioir::{EvidenceId, SpecimenId};
use bioprism_biolang::act::{
    BioRole, CommitmentType, EvidenceTransition, ScientificAct, ScientificActKind,
    SpecimenTransition,
};
use bioprism_biolang::bundle::{
    Attestation, RecordedVerdict, Repudiability, ResultBundle, RunManifest, Score, TracedAction,
};
use bioprism_biolang::capsule::{
    Assumption, BioContextCapsule, Omission, Staleness, Stance, Summary,
};
use bioprism_biolang::clock::Clock;
use bioprism_biolang::error::{
    ActError, BundleIrError, CapsuleError, FbcError, InterventionError, MoleculeError,
    MutationIrError, OracleIrError, StateError, SystemError, WorldError, WorldlineError,
};
use bioprism_biolang::fbc::{
    ClaimSchema, EvidenceObligation, Falsifier, Fbc, Intent, TerminalState, Termination,
};
use bioprism_biolang::ids::{
    ActId, ActionId, AssetId, ComponentId, FbcId, MoleculeId, MutationId, ObligationId, StateId,
    SystemId, WorldlineId,
};
use bioprism_biolang::intervention::{
    ActionClass, ActionDefinition, AuthorityRequirement, CostModel, Effect, Idempotence,
    Precondition, Reversibility,
};
use bioprism_biolang::molecule::{
    CapabilityEvidence, Choreography, FailureSemantics, Guarantee, Molecule, NestedInterface,
    RoleBinding, Step,
};
use bioprism_biolang::mutation::{
    MutationProgram, Risk, SeedDeclaration, SemanticRelation, Transformation, TransformationTarget,
};
use bioprism_biolang::oracle::{
    DisagreementIr, EvidencePlane, EvidenceTier, Independence, OracleIr, Verdict,
};
use bioprism_biolang::state::{BioState, Plane, ResourceLedger, Transition, UncertaintySummary};
use bioprism_biolang::system::{
    Component, ComponentKind, Pin, PromptDisclosure, SystemManifest, Wire,
};
use bioprism_biolang::world::{
    Asset, BioWorld, CatalogEntry, CounterfactualGrade, HiddenItem, LicensePolicy, SemanticVersion,
    VisibleState, WorldClass,
};
use bioprism_biolang::worldline::{AlignmentConfidence, Branch, Censoring, RevealGate, Worldline};
use bioprism_ids::{ContentHash, RunId, WorldId};
use bioprism_scope::{ScopeKey, Timestamp};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("RFC 3339")
}

fn digest(tag: &str) -> ContentHash {
    ContentHash::of_value(&json!({ "tag": tag })).expect("hashes")
}

fn world_id() -> WorldId {
    WorldId::parse("onco/gbm-longitudinal").expect("well-formed")
}

fn summary() -> UncertaintySummary {
    UncertaintySummary {
        budget_digest: digest("budget"),
        unaccounted_components: 0,
    }
}

fn state(id: &str, event: &str, record: &str) -> BioState {
    BioState::new(
        StateId::parse(id).expect("well-formed"),
        world_id(),
        at(event),
        at(record),
        summary(),
    )
}

// --- 25.01 BioWorld -----------------------------------------------------------------------------

fn world() -> BioWorld {
    BioWorld {
        world_id: world_id(),
        version: SemanticVersion::new(1, 4, 0),
        class: WorldClass::Observed,
        intended_use: "replay of a longitudinal imaging cohort".to_string(),
        standards: BTreeMap::from([("mondo".to_string(), "2026-03-01".to_string())]),
        assets: vec![Asset::new(
            AssetId::parse("scan-001").expect("well-formed"),
            digest("scan"),
            "pack://scans/001.nii.gz",
            "site A, scanner 01, exported 2026-02",
        )],
        visible: VisibleState {
            state_id: StateId::parse("s0").expect("well-formed"),
            exposed: BTreeSet::from([AssetId::parse("scan-001").expect("well-formed")]),
        },
        hidden: Vec::new(),
        prohibited: BTreeSet::new(),
        actions: Vec::new(),
        metered_resources: BTreeSet::from(["tissue_mg".to_string()]),
        oracle_mesh: BTreeSet::from(["oracle/schema".to_string()]),
        license: LicensePolicy {
            license: "CC-BY-4.0".to_string(),
            access_labels: BTreeSet::new(),
            embeddable: false,
        },
        counterfactual: CounterfactualGrade::BranchOnly,
    }
}

#[test]
fn an_asset_without_provenance_is_not_a_valid_world_asset() {
    let mut world = world();
    world.assets[0].provenance = "  ".to_string();
    let WorldError::AssetUnderclared { missing, .. } = world.validate().unwrap_err() else {
        panic!("expected an under-declared asset");
    };
    assert_eq!(missing, "provenance");
}

#[test]
fn an_item_that_is_both_hidden_and_initially_visible_is_refused() {
    let mut world = world();
    world.hidden.push(HiddenItem {
        asset_id: AssetId::parse("scan-001").expect("well-formed"),
        reason: "held-out outcome".to_string(),
    });
    assert!(matches!(
        world.validate().unwrap_err(),
        WorldError::HiddenItemVisible { .. }
    ));
}

#[test]
fn an_action_that_produces_a_hidden_item_makes_it_reachable_and_is_refused() {
    let mut world = world();
    let secret = AssetId::parse("outcome-2027").expect("well-formed");
    world.hidden.push(HiddenItem {
        asset_id: secret.clone(),
        reason: "prospective outcome".to_string(),
    });
    world.actions.push(CatalogEntry {
        action_id: ActionId::parse("reveal").expect("well-formed"),
        produces: BTreeSet::from([secret]),
    });
    assert!(matches!(
        world.validate().unwrap_err(),
        WorldError::HiddenItemVisible { .. }
    ));
}

#[test]
fn a_world_citing_an_oracle_outside_its_mesh_is_refused() {
    assert!(matches!(
        world().validate_oracles(["oracle/judge"]).unwrap_err(),
        WorldError::OracleNotInMesh { .. }
    ));
}

#[test]
fn a_version_that_is_not_three_numbers_is_not_a_semantic_version() {
    assert!(SemanticVersion::parse("1.4").is_err());
    assert!(SemanticVersion::parse("1.4.0-rc1").is_err());
    assert_eq!(
        SemanticVersion::parse("1.4.0").expect("parses"),
        SemanticVersion::new(1, 4, 0)
    );
}

// --- 25.02 BioState -----------------------------------------------------------------------------

#[test]
fn a_state_recorded_before_the_event_it_records_is_refused() {
    let state = state("s1", "2026-03-02T00:00:00Z", "2026-03-01T00:00:00Z");
    assert!(matches!(
        state.validate().unwrap_err(),
        StateError::RecordBeforeEvent { .. }
    ));
}

#[test]
fn a_fork_may_spend_more_than_its_parent_and_may_not_spend_less() {
    let parent = state("s1", "2026-03-01T00:00:00Z", "2026-03-01T01:00:00Z")
        .having_consumed(ResourceLedger::new().consume("tissue_mg", 40.0));
    let child = parent.fork(
        StateId::parse("s2").expect("well-formed"),
        at("2026-03-02T00:00:00Z"),
        at("2026-03-02T01:00:00Z"),
    );
    parent
        .validate_fork(&child)
        .expect("carrying forward is fine");

    let spendier = child
        .clone()
        .having_consumed(ResourceLedger::new().consume("tissue_mg", 60.0));
    parent
        .validate_fork(&spendier)
        .expect("spending more is fine");

    let unspent = child.having_consumed(ResourceLedger::new().consume("tissue_mg", 10.0));
    assert!(matches!(
        parent.validate_fork(&unspent).unwrap_err(),
        StateError::ForkUnspendsResource { .. }
    ));
}

#[test]
fn a_fork_that_names_a_different_parent_is_refused() {
    let parent = state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z");
    let orphan = state("s2", "2026-03-02T00:00:00Z", "2026-03-02T00:00:00Z");
    assert!(matches!(
        parent.validate_fork(&orphan).unwrap_err(),
        StateError::ForkParentMismatch { .. }
    ));
}

#[test]
fn a_transition_that_moves_the_biological_plane_without_declaring_it_is_refused() {
    let before = state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z")
        .with_plane(Plane::Biological, digest("tumor-a"))
        .with_plane(Plane::Knowledge, digest("belief-a"));
    let after = state("s2", "2026-03-02T00:00:00Z", "2026-03-02T00:00:00Z")
        .with_plane(Plane::Biological, digest("tumor-b"))
        .with_plane(Plane::Knowledge, digest("belief-b"));

    let epistemic_only = Transition::new(
        "read the report",
        before.state_id.clone(),
        after.state_id.clone(),
    )
    .changing(Plane::Knowledge);
    let StateError::UndeclaredPlaneChange { plane, .. } =
        epistemic_only.validate(&before, &after).unwrap_err()
    else {
        panic!("expected an undeclared plane change");
    };
    assert_eq!(plane, "biological");
}

#[test]
fn a_transition_declaring_a_plane_that_did_not_move_is_refused_too() {
    let before = state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z")
        .with_plane(Plane::Knowledge, digest("belief-a"));
    let after = state("s2", "2026-03-02T00:00:00Z", "2026-03-02T00:00:00Z")
        .with_plane(Plane::Knowledge, digest("belief-a"));
    let claim = Transition::new("assay", before.state_id.clone(), after.state_id.clone())
        .changing(Plane::Biological);
    assert!(matches!(
        claim.validate(&before, &after).unwrap_err(),
        StateError::DeclaredPlaneUnchanged { .. }
    ));
}

#[test]
fn an_assay_result_moves_the_observation_and_knowledge_planes_and_leaves_biology_alone() {
    let before = state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z")
        .with_plane(Plane::Biological, digest("tumor"))
        .with_plane(Plane::Observation, digest("nothing-measured"))
        .with_plane(Plane::Knowledge, digest("belief-a"));
    let after = state("s2", "2026-03-02T00:00:00Z", "2026-03-02T00:00:00Z")
        .with_plane(Plane::Biological, digest("tumor"))
        .with_plane(Plane::Observation, digest("mri-read"))
        .with_plane(Plane::Knowledge, digest("belief-b"));
    let transition = Transition::new(
        "read the MRI",
        before.state_id.clone(),
        after.state_id.clone(),
    )
    .changing(Plane::Observation)
    .changing(Plane::Knowledge);
    transition
        .validate(&before, &after)
        .expect("declaration matches");
    assert!(!transition.is_ontic(), "measuring is not changing");
}

// --- 25.09 BioWorldline --------------------------------------------------------------------------

fn worldline(scope: ScopeKey) -> Worldline {
    Worldline::new(
        WorldlineId::parse("wl-1").expect("well-formed"),
        world_id(),
        scope,
        Clock::Event,
        AlignmentConfidence::declared(0.9, "declared by the assembling adapter"),
        Censoring::NotCensored,
    )
}

#[test]
fn a_worldline_cannot_silently_interleave_states_from_different_scopes() {
    let line = worldline(ScopeKey::new().exact("subject", "S1"))
        .then(
            state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z")
                .within(ScopeKey::new().exact("subject", "S1")),
        )
        .then(
            state("s2", "2026-03-02T00:00:00Z", "2026-03-02T00:00:00Z")
                .within(ScopeKey::new().exact("subject", "S2")),
        );
    let WorldlineError::ScopeInterleaving { state, dimension } = line.validate().unwrap_err()
    else {
        panic!("expected scope interleaving");
    };
    assert_eq!((state.as_str(), dimension.as_str()), ("s2", "subject"));
}

#[test]
fn a_state_that_leaves_a_worldline_scope_dimension_unbound_is_interleaving_too() {
    let line = worldline(ScopeKey::new().exact("genome_build", "GRCh38")).then(state(
        "s1",
        "2026-03-01T00:00:00Z",
        "2026-03-01T00:00:00Z",
    ));
    assert!(matches!(
        line.validate().unwrap_err(),
        WorldlineError::ScopeInterleaving { .. }
    ));
}

#[test]
fn states_out_of_order_on_the_worldline_clock_are_refused() {
    let scope = ScopeKey::new().exact("subject", "S1");
    let line = worldline(scope.clone())
        .then(state("s2", "2026-03-05T00:00:00Z", "2026-03-05T00:00:00Z").within(scope.clone()))
        .then(state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z").within(scope));
    assert!(matches!(
        line.validate().unwrap_err(),
        WorldlineError::OutOfOrder { .. }
    ));
}

#[test]
fn a_state_appearing_before_its_reveal_gate_is_refused() {
    let scope = ScopeKey::new().exact("subject", "S1");
    let line = worldline(scope.clone())
        .then(state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z").within(scope))
        .gated_by(RevealGate {
            gate_id: "outcome".to_string(),
            gated_state: StateId::parse("s1").expect("well-formed"),
            reveal_at: at("2026-06-01T00:00:00Z"),
        });
    assert!(matches!(
        line.validate().unwrap_err(),
        WorldlineError::PrematureReveal { .. }
    ));
}

#[test]
fn a_branch_from_a_state_not_on_the_worldline_is_refused() {
    let scope = ScopeKey::new().exact("subject", "S1");
    let line = worldline(scope.clone())
        .then(state("s1", "2026-03-01T00:00:00Z", "2026-03-01T00:00:00Z").within(scope))
        .branching(Branch {
            branch_id: "b1".to_string(),
            parent_state: StateId::parse("s9").expect("well-formed"),
            worldline: WorldlineId::parse("wl-2").expect("well-formed"),
        });
    assert!(matches!(
        line.validate().unwrap_err(),
        WorldlineError::BranchParentMissing { .. }
    ));
}

#[test]
fn comparing_two_worldlines_requires_their_scopes_to_overlap() {
    let left = worldline(ScopeKey::new().exact("subject", "S1"));
    let mut right = worldline(ScopeKey::new().exact("subject", "S2"));
    right.worldline_id = WorldlineId::parse("wl-2").expect("well-formed");
    assert!(matches!(
        left.comparable_with(&right).unwrap_err(),
        WorldlineError::ScopesDisjoint { .. }
    ));
}

#[test]
fn two_worldlines_in_compatible_scopes_compare_on_their_common_scope() {
    let left = worldline(ScopeKey::new().exact("subject", "S1"));
    let mut right = worldline(
        ScopeKey::new()
            .exact("subject", "S1")
            .exact("genome_build", "GRCh38"),
    );
    right.worldline_id = WorldlineId::parse("wl-2").expect("well-formed");
    let common = left.comparable_with(&right).expect("overlapping scopes");
    assert_eq!(common.len(), 2, "the meet keeps both bound dimensions");
}

#[test]
fn a_decision_ordered_worldline_reports_that_no_state_carries_that_clock() {
    let mut line = worldline(ScopeKey::new());
    line.ordering_clock = Clock::Decision;
    assert!(
        !line.orders_on_a_carried_clock(),
        "25.09 requires decision_time; 25.02 gives a state no field for it"
    );
}

// --- 25.06 Intervention -------------------------------------------------------------------------

fn action(class: ActionClass) -> ActionDefinition {
    ActionDefinition {
        action_id: ActionId::parse("act-1").expect("well-formed"),
        class,
        input_planes: BTreeSet::from([Plane::Observation]),
        preconditions: Vec::new(),
        effects: vec![Effect {
            plane: Plane::Knowledge,
            description: "a differential expression table".to_string(),
            observed: false,
        }],
        reversibility: Reversibility::Reversible,
        authority: AuthorityRequirement::None,
        cost: CostModel::new().costing("cpu_seconds", 30.0),
        latency_seconds: 30.0,
        uncertainty: "sampling noise in the input counts".to_string(),
        side_effects: Vec::new(),
        idempotence: Idempotence::Idempotent,
        result_schema: "sha256:table-v1".to_string(),
    }
}

#[test]
fn a_modeled_perturbation_may_not_claim_an_observed_biological_effect() {
    let mut definition = action(ActionClass::ModeledPerturbation);
    definition.effects.push(Effect {
        plane: Plane::Biological,
        description: "tumour shrinks".to_string(),
        observed: true,
    });
    assert!(matches!(
        definition.validate().unwrap_err(),
        InterventionError::SimulationClaimsRealEffect { .. }
    ));
}

#[test]
fn a_real_world_action_must_touch_a_material_or_biological_plane() {
    let definition = action(ActionClass::RealWorldEffect);
    assert!(matches!(
        definition.validate().unwrap_err(),
        InterventionError::RealEffectWithoutRealPlane { .. }
    ));
}

#[test]
fn an_irreversible_action_that_requires_no_authority_is_refused() {
    let mut definition = action(ActionClass::ComputationalTransformation);
    definition.reversibility = Reversibility::Irreversible;
    assert!(matches!(
        definition.validate().unwrap_err(),
        InterventionError::IrreversibleWithoutAuthority { .. }
    ));
}

#[test]
fn compensation_that_claims_to_leave_no_residue_is_refused() {
    let mut definition = action(ActionClass::ComputationalTransformation);
    definition.reversibility = Reversibility::CompensatableWithResidue {
        residue: Vec::new(),
    };
    assert!(
        matches!(
            definition.validate().unwrap_err(),
            InterventionError::CompensationClaimsNoResidue { .. }
        ),
        "bioprism-choreography established that compensation is not rollback"
    );
}

#[test]
fn an_irreversible_material_consumption_cannot_also_be_idempotent() {
    let mut definition = action(ActionClass::RealWorldEffect);
    definition.effects = vec![Effect {
        plane: Plane::Material,
        description: "reserves 40 mg of tissue".to_string(),
        observed: true,
    }];
    definition.reversibility = Reversibility::Irreversible;
    definition.authority = AuthorityRequirement::HumanApproval {
        role: "biobank steward".to_string(),
    };
    assert!(matches!(
        definition.validate().unwrap_err(),
        InterventionError::IrreversibleConsumptionCannotBeIdempotent { .. }
    ));
}

#[test]
fn a_precondition_on_a_plane_the_action_never_reads_is_refused() {
    let mut definition = action(ActionClass::InformationAcquisition);
    definition.preconditions.push(Precondition {
        plane: Plane::Material,
        expression: "at least 40 mg remains".to_string(),
    });
    assert!(matches!(
        definition.validate().unwrap_err(),
        InterventionError::PreconditionOffInputPlane { .. }
    ));
}

// --- 25.07 FBC ------------------------------------------------------------------------------------

fn contract() -> Fbc {
    Fbc {
        fbc_id: FbcId::parse("fbc-1").expect("well-formed"),
        intent: Intent {
            statement: "does this variant meet the interpretation criteria".to_string(),
            requester_role: "molecular pathologist".to_string(),
        },
        scope: ScopeKey::new().exact("genome_build", "GRCh38"),
        preconditions: Vec::new(),
        obligations: vec![EvidenceObligation {
            obligation_id: ObligationId::parse("ob-1").expect("well-formed"),
            description: "population frequency".to_string(),
            dischargeable_by: BTreeSet::from([ActionId::parse("lookup").expect("well-formed")]),
            required: true,
        }],
        allowed_actions: BTreeSet::from([ActionId::parse("lookup").expect("well-formed")]),
        claim_schema: ClaimSchema {
            schema_digest: "sha256:claim-v1".to_string(),
            description: "a five-tier classification".to_string(),
        },
        falsifiers: vec![Falsifier {
            falsifier_id: "f-1".to_string(),
            condition: "the variant is common in a matched population".to_string(),
            oracle: "oracle/frequency".to_string(),
        }],
        oracle_mesh: BTreeSet::from(["oracle/frequency".to_string()]),
        capped_resources: BTreeSet::new(),
        terminal_states: vec![TerminalState {
            label: "classified".to_string(),
            termination: Termination::Success,
            open_obligations: BTreeSet::new(),
        }],
    }
}

#[test]
fn a_success_state_that_leaves_a_required_obligation_open_is_refused() {
    let mut contract = contract();
    contract.terminal_states[0].open_obligations =
        BTreeSet::from([ObligationId::parse("ob-1").expect("well-formed")]);
    assert!(matches!(
        contract.validate().unwrap_err(),
        FbcError::SuccessWithOpenObligation { .. }
    ));
}

#[test]
fn an_obligation_no_allowed_action_can_discharge_is_unreachable() {
    let mut contract = contract();
    contract.allowed_actions.clear();
    assert!(matches!(
        contract.validate().unwrap_err(),
        FbcError::UnreachableObligation { .. }
    ));
}

#[test]
fn a_falsifier_naming_an_oracle_outside_the_mesh_is_refused() {
    let mut contract = contract();
    contract.oracle_mesh.clear();
    assert!(matches!(
        contract.validate().unwrap_err(),
        FbcError::FalsifierWithoutOracle { .. }
    ));
}

#[test]
fn a_contract_with_no_falsifier_is_not_falsifiable() {
    let mut contract = contract();
    contract.falsifiers.clear();
    assert!(matches!(
        contract.validate().unwrap_err(),
        FbcError::NoFalsifier { .. }
    ));
}

#[test]
fn a_claim_wider_than_the_contract_envelope_names_the_dimension_that_escaped() {
    let contract = contract();
    let FbcError::UnsupportedScopeExpansion { dimension } =
        contract.admits_claim_in(&ScopeKey::new()).unwrap_err()
    else {
        panic!("expected an unsupported scope expansion");
    };
    assert_eq!(dimension, "genome_build");
}

#[test]
fn underdetermined_is_a_valid_terminal_state() {
    let mut contract = contract();
    contract.terminal_states.push(TerminalState {
        label: "not settled".to_string(),
        termination: Termination::Underdetermined {
            reason: "the population database has no matched ancestry".to_string(),
        },
        open_obligations: BTreeSet::from([ObligationId::parse("ob-1").expect("well-formed")]),
    });
    contract
        .validate()
        .expect("an underdetermined ending may leave obligations open");
}

// --- 25.14 System ---------------------------------------------------------------------------------

fn component(id: &str, version: &str) -> Component {
    Component {
        component_id: ComponentId::parse(id).expect("well-formed"),
        kind: ComponentKind::Model,
        pin: Pin::new(version),
        inputs: BTreeSet::from(["prompt".to_string()]),
        outputs: BTreeSet::from(["completion".to_string()]),
        effects: BTreeSet::new(),
        prompt: PromptDisclosure::NotApplicable,
        behavior_contract: "answers a typed decision query".to_string(),
        nondeterministic_inputs: BTreeSet::new(),
        deterministic: true,
    }
}

fn manifest() -> SystemManifest {
    SystemManifest::new(
        SystemId::parse("sys-1").expect("well-formed"),
        "protected closure then relevance",
        "none",
    )
    .with(component("model", "4.2.0"))
}

#[test]
fn a_component_pinned_to_latest_is_not_pinned() {
    let mut manifest = manifest();
    manifest.components[0].pin = Pin::new("latest");
    assert!(matches!(
        manifest.validate().unwrap_err(),
        SystemError::UnpinnedComponent { .. }
    ));
}

#[test]
fn a_hidden_prompt_is_allowed_and_a_hidden_behaviour_contract_is_not() {
    let mut manifest = manifest();
    manifest.components[0].prompt = PromptDisclosure::Hashed {
        digest: "sha256:prompt".to_string(),
    };
    manifest.validate().expect("hashing a prompt is allowed");

    manifest.components[0].behavior_contract = String::new();
    assert!(matches!(
        manifest.validate().unwrap_err(),
        SystemError::HiddenBehaviourContract { .. }
    ));
}

#[test]
fn a_determinism_claim_contradicted_by_a_declared_source_is_refused() {
    let mut manifest = manifest();
    manifest.components[0]
        .nondeterministic_inputs
        .insert("sampler seed from the OS".to_string());
    assert!(matches!(
        manifest.validate().unwrap_err(),
        SystemError::DeterminismContradicted { .. }
    ));
}

#[test]
fn a_wire_to_an_undeclared_component_is_refused() {
    let manifest = manifest().wired(Wire {
        from: ComponentId::parse("model").expect("well-formed"),
        to: ComponentId::parse("ghost").expect("well-formed"),
        carries: "completion".to_string(),
    });
    assert!(matches!(
        manifest.validate().unwrap_err(),
        SystemError::DanglingComponent { .. }
    ));
}

#[test]
fn comparing_two_architectures_exposes_the_components_that_changed() {
    let left = manifest();
    let right = SystemManifest::new(
        SystemId::parse("sys-1").expect("well-formed"),
        "protected closure then relevance",
        "none",
    )
    .with(component("model", "4.3.0"))
    .with(component("verifier", "1.0.0"));
    let deltas = left.diff(&right);
    assert_eq!(deltas.len(), 2, "one repin and one addition");
}

// --- 25.15 Acts -----------------------------------------------------------------------------------

fn role() -> BioRole {
    BioRole::new(
        "molecular pathologist",
        "variant interpretation",
        "germline",
    )
    .permitting(ScientificActKind::Hypothesize)
    .permitting(ScientificActKind::Reserve)
}

#[test]
fn three_of_the_seven_scientific_acts_have_no_weave_communicative_counterpart() {
    let unmapped: Vec<&str> = ScientificActKind::ALL
        .into_iter()
        .filter(|kind| kind.communicative_act().is_none())
        .map(ScientificActKind::as_str)
        .collect();
    assert_eq!(
        unmapped,
        vec!["measure", "reproduce", "reserve", "retract"],
        "25.15 names four acts bioprism-weave has no ActKind for, retraction among them"
    );
}

#[test]
fn a_claim_act_that_identifies_no_evidence_is_refused() {
    let act = ScientificAct::new(
        ActId::parse("a1").expect("well-formed"),
        ScientificActKind::Hypothesize,
        "molecular pathologist",
        CommitmentType::Asserted,
    )
    .scoped("genome_build=GRCh38");
    let ActError::ClaimWithout { missing, .. } = act.validate(&role(), &|_| 0.0).unwrap_err()
    else {
        panic!("expected a claim without evidence");
    };
    assert_eq!(missing, "evidence");
}

#[test]
fn a_claim_act_that_identifies_no_scope_is_refused() {
    let act = ScientificAct::new(
        ActId::parse("a1").expect("well-formed"),
        ScientificActKind::Hypothesize,
        "molecular pathologist",
        CommitmentType::Asserted,
    )
    .citing(EvidenceId::parse("ev-1").expect("well-formed"));
    let ActError::ClaimWithout { missing, .. } = act.validate(&role(), &|_| 0.0).unwrap_err()
    else {
        panic!("expected a claim without scope");
    };
    assert_eq!(missing, "scope");
}

#[test]
fn a_specimen_act_may_not_overdraw_the_material_that_remains() {
    let act = ScientificAct::new(
        ActId::parse("a2").expect("well-formed"),
        ScientificActKind::Reserve,
        "molecular pathologist",
        CommitmentType::Undertaken,
    )
    .moving_material(SpecimenTransition::Reserved {
        specimen: SpecimenId::parse("sp-1").expect("well-formed"),
        amount: 80.0,
    });
    assert!(matches!(
        act.validate(&role(), &|_| 40.0).unwrap_err(),
        ActError::MaterialOverdrawn { .. }
    ));
}

#[test]
fn a_role_may_not_perform_an_act_it_is_not_authorised_for() {
    let act = ScientificAct::new(
        ActId::parse("a3").expect("well-formed"),
        ScientificActKind::Retract,
        "molecular pathologist",
        CommitmentType::Asserted,
    );
    assert!(matches!(
        act.validate(&role(), &|_| 0.0).unwrap_err(),
        ActError::RoleNotAuthorised { .. }
    ));
}

#[test]
fn an_act_that_must_post_to_a_ledger_and_does_not_is_refused() {
    let act = ScientificAct::new(
        ActId::parse("a4").expect("well-formed"),
        ScientificActKind::Reserve,
        "molecular pathologist",
        CommitmentType::Undertaken,
    );
    assert!(matches!(
        act.validate(&role(), &|_| 100.0).unwrap_err(),
        ActError::ActWithoutLedgerEntry { .. }
    ));
}

#[test]
fn a_well_formed_claim_with_evidence_scope_and_a_ledger_move_is_accepted() {
    let act = ScientificAct::new(
        ActId::parse("a5").expect("well-formed"),
        ScientificActKind::Hypothesize,
        "molecular pathologist",
        CommitmentType::Tentative,
    )
    .citing(EvidenceId::parse("ev-1").expect("well-formed"))
    .scoped("genome_build=GRCh38")
    .moving_evidence(EvidenceTransition::Introduced {
        evidence: EvidenceId::parse("ev-1").expect("well-formed"),
    });
    act.validate(&role(), &|_| 0.0).expect("well formed");
}

// --- 25.16 Capsule ----------------------------------------------------------------------------------

fn capsule() -> BioContextCapsule {
    BioContextCapsule {
        recipient: "agent-7".to_string(),
        recipient_role: "analyst".to_string(),
        objective: "classify the variant".to_string(),
        success_contract: "a five-tier classification with cited evidence".to_string(),
        evidence: BTreeMap::from([(
            Stance::Provisional,
            BTreeSet::from([EvidenceId::parse("ev-1").expect("well-formed")]),
        )]),
        assumptions: vec![Assumption {
            statement: "the frequency database matches the subject's ancestry".to_string(),
            discharged_by: Some("an ancestry-matched cohort lookup".to_string()),
        }],
        accessible_actions: BTreeSet::new(),
        authority: BTreeSet::new(),
        budget: BTreeMap::new(),
        open_obligations: BTreeSet::new(),
        omissions: vec![Omission {
            item: "ev-9".to_string(),
            reason: "requires a clearance the recipient does not hold".to_string(),
            required_labels: BTreeSet::from(["phi:identified".to_string()]),
        }],
        summaries: Vec::new(),
        staleness: Staleness::Untracked {
            built_at: at("2026-03-01T00:00:00Z"),
        },
        upstream_supports_sufficiency: false,
    }
}

#[test]
fn an_omission_without_a_stated_reason_is_refused() {
    let mut capsule = capsule();
    capsule.omissions[0].reason = "   ".to_string();
    assert!(matches!(
        capsule.validate(&BTreeSet::new()).unwrap_err(),
        CapsuleError::OmissionWithoutReason { .. }
    ));
}

#[test]
fn a_summary_that_cites_no_source_evidence_is_refused() {
    let mut capsule = capsule();
    capsule.summaries.push(Summary {
        summary_id: "sum-1".to_string(),
        text: "the variant is rare".to_string(),
        sources: BTreeSet::new(),
    });
    assert!(matches!(
        capsule.validate(&BTreeSet::new()).unwrap_err(),
        CapsuleError::SummaryWithoutSource { .. }
    ));
}

#[test]
fn one_item_may_not_be_filed_under_two_stances() {
    let mut capsule = capsule();
    capsule.evidence.insert(
        Stance::Contradicted,
        BTreeSet::from([EvidenceId::parse("ev-1").expect("well-formed")]),
    );
    assert!(matches!(
        capsule.validate(&BTreeSet::new()).unwrap_err(),
        CapsuleError::ContradictoryStance { .. }
    ));
}

#[test]
fn an_item_declared_omitted_but_nonetheless_present_is_refused() {
    let mut capsule = capsule();
    capsule.omissions[0].item = "ev-1".to_string();
    assert!(matches!(
        capsule.validate(&BTreeSet::new()).unwrap_err(),
        CapsuleError::LabelNotHeld { .. }
    ));
}

// --- 25.17 Molecule ---------------------------------------------------------------------------------

fn molecule() -> Molecule {
    Molecule {
        molecule_id: MoleculeId::parse("mol-1").expect("well-formed"),
        input_schema: "sha256:in".to_string(),
        output_schema: "sha256:out".to_string(),
        roles: vec![RoleBinding {
            role: "reader".to_string(),
            bound_to: "sys-1".to_string(),
            capabilities: BTreeSet::from(["read:evidence".to_string()]),
        }],
        choreography: Choreography::default().then(Step {
            step_id: "s1".to_string(),
            role: "reader".to_string(),
            description: "read the evidence".to_string(),
        }),
        authority: BTreeSet::from(["read:evidence".to_string()]),
        effects: BTreeSet::new(),
        guarantees: vec![Guarantee {
            guarantee_id: "g1".to_string(),
            statement: "every claim cites evidence".to_string(),
            backed_by: BTreeSet::from(["e1".to_string()]),
        }],
        evidence: vec![CapabilityEvidence {
            evidence_id: "e1".to_string(),
            source: "regression pack 12".to_string(),
            finding: "no uncited claim in 400 runs".to_string(),
        }],
        failure: FailureSemantics::PartialWithResidue,
        nested: Vec::new(),
        version: "1.0.0".to_string(),
    }
}

#[test]
fn a_nested_molecule_may_not_require_authority_its_parent_lacks() {
    let mut molecule = molecule();
    molecule.nested.push(NestedInterface {
        molecule: MoleculeId::parse("mol-2").expect("well-formed"),
        requires: BTreeSet::from(["write:specimen".to_string()]),
    });
    assert!(matches!(
        molecule.validate().unwrap_err(),
        MoleculeError::NestedAuthorityBroadened { .. }
    ));
}

#[test]
fn a_guarantee_with_no_evaluation_evidence_behind_it_is_refused() {
    let mut molecule = molecule();
    molecule.evidence.clear();
    assert!(matches!(
        molecule.validate().unwrap_err(),
        MoleculeError::UnbackedGuarantee { .. }
    ));
}

#[test]
fn a_choreography_step_bound_to_no_declared_role_is_refused() {
    let mut molecule = molecule();
    molecule.choreography.steps[0].role = "ghost".to_string();
    assert!(matches!(
        molecule.validate().unwrap_err(),
        MoleculeError::UnboundStep { .. }
    ));
}

// --- 25.18 Oracle -------------------------------------------------------------------------------------

fn oracle(id: &str, tier: EvidenceTier) -> OracleIr {
    OracleIr {
        oracle_id: id.to_string(),
        kind: "schema".to_string(),
        version: "1.0.0".to_string(),
        tier,
        inputs: BTreeSet::from(["document".to_string()]),
        outputs: BTreeSet::from(["verdict".to_string()]),
        establishes: BTreeSet::from([EvidencePlane::Artifact]),
        cannot_establish: BTreeSet::from([EvidencePlane::Biological]),
        evidence_basis: "a JSON schema".to_string(),
        failure_conditions: vec!["the document is not JSON".to_string()],
        priority: 0,
        calibration: "not applicable; the check is exact".to_string(),
        independence: Independence {
            from_evaluated_system: true,
            shared_resources: BTreeSet::new(),
        },
    }
}

#[test]
fn a_judge_may_not_override_a_deterministic_oracle() {
    let judge = oracle("oracle/judge", EvidenceTier::Judge);
    let checksum = oracle("oracle/schema", EvidenceTier::Deterministic);
    assert!(matches!(
        judge.may_override(&checksum).unwrap_err(),
        OracleIrError::WeakerTierOverrides { .. }
    ));
    checksum
        .may_override(&judge)
        .expect("a deterministic oracle may override a judge");
}

#[test]
fn an_oracle_that_both_claims_and_disclaims_a_plane_is_refused() {
    let mut oracle = oracle("oracle/schema", EvidenceTier::Deterministic);
    oracle.cannot_establish.insert(EvidencePlane::Artifact);
    assert!(matches!(
        oracle.validate().unwrap_err(),
        OracleIrError::PlaneClaimedAndDisclaimed { .. }
    ));
}

#[test]
fn an_oracle_not_independent_of_what_it_evaluates_is_refused() {
    let mut oracle = oracle("oracle/self", EvidenceTier::Judge);
    oracle.independence.from_evaluated_system = false;
    assert!(matches!(
        oracle.validate().unwrap_err(),
        OracleIrError::CircularIndependence { .. }
    ));
}

#[test]
fn a_resolved_disagreement_that_kept_no_losing_position_is_refused() {
    let mut disagreement = DisagreementIr {
        left_oracle: "oracle/a".to_string(),
        left_verdict: Verdict::Pass,
        right_oracle: "oracle/b".to_string(),
        right_verdict: Verdict::Fail {
            reason: "the digest does not match".to_string(),
        },
        would_settle: Some("recompute the digest".to_string()),
        retained_losing_position: None,
    };
    assert!(matches!(
        disagreement.validate_resolution().unwrap_err(),
        OracleIrError::DisagreementDiscarded { .. }
    ));
    disagreement.retained_losing_position = Some("oracle/a said pass".to_string());
    disagreement
        .validate_resolution()
        .expect("the loser is retained");
}

#[test]
fn an_oracle_can_abstain_and_abstention_is_not_a_failure() {
    let verdict = Verdict::Abstain {
        reason: "no matched population".to_string(),
    };
    assert!(verdict.is_abstention());
    assert!(!matches!(verdict, Verdict::Fail { .. }));
}

// --- 25.19 Mutation ------------------------------------------------------------------------------------

fn program(relation: SemanticRelation) -> MutationProgram {
    MutationProgram {
        mutation_id: MutationId::parse("mut-1").expect("well-formed"),
        parent: Some("world/base@1".to_string()),
        applicability: "any world with a lesion table".to_string(),
        seed: SeedDeclaration::Seeded { seed: 42 },
        transformations: vec![Transformation {
            target: TransformationTarget::World,
            locator: "$.facts[0].value".to_string(),
            description: "rescale a volume".to_string(),
        }],
        relation,
        oracle_changes: BTreeSet::new(),
        validations: vec!["the world still parses".to_string()],
        risk: Risk::Substantive,
        generator_version: "bioprism-mutation 0.1.0".to_string(),
    }
}

#[test]
fn a_mutation_with_no_parent_lineage_is_refused() {
    let mut program = program(SemanticRelation::Preserving);
    program.parent = None;
    assert!(matches!(
        program.validate().unwrap_err(),
        MutationIrError::LineageBroken { .. }
    ));
}

#[test]
fn a_semantics_changing_mutation_that_updates_no_oracle_is_refused() {
    let program = program(SemanticRelation::Changing);
    assert!(matches!(
        program.validate().unwrap_err(),
        MutationIrError::SemanticChangeWithoutOracleUpdate { .. }
    ));
}

#[test]
fn a_semantics_preserving_mutation_that_moves_an_oracle_is_refused() {
    let mut program = program(SemanticRelation::Preserving);
    program.oracle_changes.insert("oracle/schema".to_string());
    assert!(matches!(
        program.validate().unwrap_err(),
        MutationIrError::PreservingMutationChangesOracle { .. }
    ));
}

#[test]
fn a_mutation_with_no_generator_version_is_not_replayable() {
    let mut program = program(SemanticRelation::Preserving);
    program.generator_version = String::new();
    assert!(matches!(
        program.validate().unwrap_err(),
        MutationIrError::UnseededGenerator { .. }
    ));
}

// --- 25.20 Bundle ---------------------------------------------------------------------------------------

fn bundle() -> ResultBundle {
    ResultBundle {
        manifest: RunManifest {
            run_id: RunId::parse("run-1").expect("well-formed"),
            system: SystemId::parse("sys-1").expect("well-formed"),
            component_versions: BTreeMap::from([("model".to_string(), "4.2.0".to_string())]),
            world: world_id(),
            world_version: "1.4.0".to_string(),
            environment: BTreeMap::new(),
        },
        version: 1,
        amends: None,
        trace: vec![TracedAction {
            action: ActionId::parse("lookup").expect("well-formed"),
            step: 0,
            produced: BTreeSet::from(["sha256:table".to_string()]),
            invoked_oracle: Some("oracle/schema".to_string()),
        }],
        entries: BTreeMap::from([("table".to_string(), "sha256:table".to_string())]),
        verdicts: vec![RecordedVerdict {
            oracle: "oracle/schema".to_string(),
            verdict: Verdict::Pass,
            tier: EvidenceTier::Deterministic,
        }],
        scores: vec![Score {
            name: "admissible".to_string(),
            value: 1.0,
            interval: None,
            entry: "table".to_string(),
        }],
        resource_use: BTreeMap::new(),
        violations: Vec::new(),
        limitations: vec!["single site".to_string()],
        attestation: Some(Attestation {
            tag: "hmac-sha256:abcd".to_string(),
            claimed_producer: "lab-a".to_string(),
            evidence_scope: BTreeSet::from(["table".to_string()]),
            repudiability: Repudiability::ForgeableByAnyVerifier,
        }),
    }
}

#[test]
fn a_score_that_resolves_to_no_bundle_entry_is_refused() {
    let mut bundle = bundle();
    bundle.scores[0].entry = "missing".to_string();
    assert!(matches!(
        bundle.validate().unwrap_err(),
        BundleIrError::ScoreWithoutBundle { .. }
    ));
}

#[test]
fn an_amendment_that_reuses_the_original_version_is_refused() {
    let mut bundle = bundle();
    bundle.amends = Some(1);
    assert!(matches!(
        bundle.validate().unwrap_err(),
        BundleIrError::AmendmentReusesVersion { .. }
    ));
}

#[test]
fn an_attestation_without_an_evidence_scope_is_refused() {
    let mut bundle = bundle();
    if let Some(attestation) = bundle.attestation.as_mut() {
        attestation.evidence_scope.clear();
    }
    let BundleIrError::AttestationWithout { missing, .. } = bundle.validate().unwrap_err() else {
        panic!("expected an incomplete attestation");
    };
    assert_eq!(missing, "evidence scope");
}

#[test]
fn a_verdict_from_an_oracle_the_run_never_invoked_is_refused() {
    let mut bundle = bundle();
    bundle.verdicts[0].oracle = "oracle/never-ran".to_string();
    assert!(matches!(
        bundle.validate().unwrap_err(),
        BundleIrError::VerdictFromUninvokedOracle { .. }
    ));
}

#[test]
fn a_bundle_never_supports_third_party_verification_on_this_platform() {
    assert!(
        !bundle().supports_third_party_verification(),
        "symmetric authentication only: anyone who can verify could have forged"
    );
    assert_eq!(
        bundle().attestation.expect("attested").repudiability,
        Repudiability::ForgeableByAnyVerifier
    );
}
