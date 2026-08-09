//! Projecting the crates that already implement §25 into the IR §25 asks for.
//!
//! Modules 25.15, 25.16, 25.18, 25.19 and 25.20 are the wire form of behaviour `bioprism-weave`,
//! `bioprism-oracle`, `bioprism-mutation` and `bioprism-bundle` already implement. These tests do
//! the projection against real values from those crates and record, as
//! [`bioprism_biolang::ProjectionGap`]s, every required field the source cannot fill.
//!
//! Those crates are **dev-dependencies only**. The library's dependency set is unchanged; what the
//! tests add is the ability to check the projection against the running implementation instead of
//! asserting it in prose.
//!
//! A gap is a finding, not a failure. Widening the IR to make a required field optional would hide
//! the disagreement; inventing a value for it would fabricate one.

use bioprism_biolang::act::{ScientificAct, ScientificActKind};
use bioprism_biolang::bundle::Repudiability as IrRepudiability;
use bioprism_biolang::capsule::{BioContextCapsule, Omission, Staleness, Stance};
use bioprism_biolang::ids::{ActId, MutationId};
use bioprism_biolang::mutation::{MutationProgram, Risk, SeedDeclaration, SemanticRelation};
use bioprism_biolang::oracle::{EvidencePlane, EvidenceTier as IrTier, Independence, OracleIr};
use bioprism_biolang::{Projection, ProjectionGap};
use bioprism_bioir::EvidenceId;
use bioprism_bundle::{AuthenticationScheme, MacTag, Repudiability};
use bioprism_mutation::{Mutation, MutationKind, Relation};
use bioprism_oracle::{
    EvidenceTier, Independence as OracleIndependence, OracleId, OracleManifest, OracleRef,
    OracleVersion, Plane, UtcTimestamp, ValidityWindow,
};
use bioprism_scope::Timestamp;
use bioprism_weave::{Act, ActKind, ContextCapsule, Label, WithheldItem};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

// --- 25.15: acts ---------------------------------------------------------------------------------

/// Projects a weave communicative act onto the scientific act it stands for, when one exists.
fn scientific_kind_of(kind: ActKind) -> Option<ScientificActKind> {
    ScientificActKind::ALL
        .into_iter()
        .find(|scientific| scientific.communicative_act() == Some(kind.as_str()))
}

#[test]
fn every_scientific_act_that_claims_a_weave_counterpart_names_a_real_actkind() {
    let real: BTreeSet<&str> = [
        ActKind::Ask,
        ActKind::Claim,
        ActKind::Propose,
        ActKind::Accept,
        ActKind::Reject,
        ActKind::Challenge,
        ActKind::Discharge,
        ActKind::Delegate,
        ActKind::Revoke,
        ActKind::Attest,
    ]
    .into_iter()
    .map(ActKind::as_str)
    .collect();

    for kind in ScientificActKind::ALL {
        if let Some(mapped) = kind.communicative_act() {
            assert!(
                real.contains(mapped),
                "{kind} projects onto {mapped:?}, which bioprism-weave does not define"
            );
        }
    }
}

#[test]
fn bioprism_weave_has_no_act_that_retracts_a_claim() {
    let retracting: Vec<&str> = [
        ActKind::Ask,
        ActKind::Claim,
        ActKind::Propose,
        ActKind::Accept,
        ActKind::Reject,
        ActKind::Challenge,
        ActKind::Discharge,
        ActKind::Delegate,
        ActKind::Revoke,
        ActKind::Attest,
    ]
    .into_iter()
    .map(ActKind::as_str)
    .filter(|name| *name == "retract")
    .collect();
    assert!(
        retracting.is_empty(),
        "25.15 requires a retract act; the microkernel deliberately has none, and challenge is documented as not retracting"
    );
    assert_eq!(ScientificActKind::Retract.communicative_act(), None);
}

#[test]
fn a_weave_claim_projects_onto_a_hypothesize_act_and_declares_what_it_could_not_fill() {
    let act = Act::new(
        ActKind::Claim,
        "agent-a",
        "agent-b",
        json!({ "proposition": "the lesion grew", "evidence": ["ev-1"] }),
    );
    let kind = scientific_kind_of(act.kind).expect("claim maps to hypothesize");
    assert_eq!(kind, ScientificActKind::Hypothesize);

    let projected = ScientificAct::new(
        ActId::parse("a1").expect("well-formed"),
        kind,
        "unknown",
        bioprism_biolang::act::CommitmentType::Asserted,
    )
    .citing(EvidenceId::parse("ev-1").expect("well-formed"));

    let gaps = vec![
        ProjectionGap::new(
            "25.15",
            "actor_role",
            "bioprism_weave::Act",
            "an act names a participant, not a scientific role; 25.15 requires role capability and domain scope",
        ),
        ProjectionGap::new(
            "25.15",
            "claim_scope",
            "bioprism_weave::Act",
            "the payload is an untyped JSON value; a scope is not a declared field on an act",
        ),
    ];
    let projection = Projection::with_gaps(projected, gaps);
    assert_eq!(
        projection.unfilled_fields(),
        vec!["actor_role", "claim_scope"]
    );
    assert!(projection.value_digest().is_ok());
}

#[test]
fn a_weave_act_kind_with_no_scientific_counterpart_projects_to_nothing() {
    for kind in [ActKind::Delegate, ActKind::Revoke, ActKind::Discharge] {
        assert_eq!(
            scientific_kind_of(kind),
            None,
            "{kind:?} is authority or commitment plumbing, not a scientific act"
        );
    }
}

// --- 25.16: context capsules ----------------------------------------------------------------------

fn weave_capsule() -> ContextCapsule {
    ContextCapsule {
        recipient: "agent-7".to_string(),
        role: "analyst".to_string(),
        layer: bioprism_section::Layer::L2,
        content: json!({ "evidence": [{ "id": "ev-1" }] }),
        withheld: vec![WithheldItem {
            id: "ev-9".to_string(),
            required_labels: vec![Label::new("phi:identified").0],
        }],
        upstream_supports_sufficiency: false,
        capsule_sha256: String::new(),
    }
}

/// The projection: three of 25.16's ten required field groups come from a weave capsule.
fn project_capsule(capsule: &ContextCapsule) -> Projection<BioContextCapsule> {
    let value = BioContextCapsule {
        recipient: capsule.recipient.clone(),
        recipient_role: capsule.role.clone(),
        objective: String::new(),
        success_contract: String::new(),
        evidence: BTreeMap::from([(
            Stance::Provisional,
            BTreeSet::from([EvidenceId::parse("ev-1").expect("well-formed")]),
        )]),
        assumptions: Vec::new(),
        accessible_actions: BTreeSet::new(),
        authority: BTreeSet::new(),
        budget: BTreeMap::new(),
        open_obligations: BTreeSet::new(),
        omissions: capsule
            .withheld
            .iter()
            .map(|item| Omission {
                item: item.id.clone(),
                reason: "withheld by the recipient's clearance".to_string(),
                required_labels: item.required_labels.iter().cloned().collect(),
            })
            .collect(),
        summaries: Vec::new(),
        staleness: Staleness::Untracked {
            built_at: Timestamp::parse("2026-03-01T00:00:00Z").expect("RFC 3339"),
        },
        upstream_supports_sufficiency: capsule.upstream_supports_sufficiency,
    };

    let gaps = [
        ("objective", "a capsule transports a projection; it carries no task"),
        ("success_contract", "no field; 25.16 requires what counts as done"),
        (
            "evidence.verified/contradicted",
            "weave records selection and withholding, not epistemic stance",
        ),
        ("assumptions", "no field"),
        ("accessible_actions", "no field; the kernel holds the action catalog"),
        ("authority", "held in the AuthorityTable, not in the capsule"),
        ("budget", "held in an affine Budget, which is deliberately not Clone and so cannot be projected"),
        ("staleness", "the capsule has a digest but no built-at instant; weave reads no clock"),
    ]
    .into_iter()
    .map(|(field, detail)| {
        ProjectionGap::new("25.16", field, "bioprism_weave::ContextCapsule", detail)
    })
    .collect();

    Projection::with_gaps(value, gaps)
}

#[test]
fn a_weave_capsule_fills_three_of_the_ten_field_groups_bio_context_capsule_requires() {
    let projection = project_capsule(&weave_capsule());
    assert_eq!(
        projection.gaps.len(),
        8,
        "25.16 lists ten required field groups; weave's capsule is a transport, not a briefing"
    );
    assert!(!projection.is_complete());
}

#[test]
fn the_omissions_a_weave_capsule_records_project_faithfully_with_their_labels() {
    let projection = project_capsule(&weave_capsule());
    let omission = projection.value.omissions.first().expect("one omission");
    assert_eq!(omission.item, "ev-9");
    assert!(omission.required_labels.contains("phi:identified"));
    assert!(
        !omission.reason.trim().is_empty(),
        "25.16 requires omissions to be explicit and this one survives the projection"
    );
}

#[test]
fn a_capsule_that_withheld_something_does_not_inherit_a_sufficiency_claim() {
    let projection = project_capsule(&weave_capsule());
    assert!(!projection.value.upstream_supports_sufficiency);
    assert!(
        !weave_capsule().supports_sufficiency_claim(),
        "the flag this IR keeps is the one weave has and 25.16 never asks for"
    );
}

// --- 25.18: oracles ------------------------------------------------------------------------------

fn oracle_manifest() -> OracleManifest {
    OracleManifest::new(
        OracleRef::new(
            OracleId::parse("bioprism:schema").expect("well-formed"),
            OracleVersion::new(1, 0, 0),
        ),
        EvidenceTier::Deterministic,
        [Plane::Artifact],
        [Plane::Biological],
        ValidityWindow::open_ended(UtcTimestamp::parse("2026-01-01T00:00:00Z").expect("RFC 3339")),
    )
    .expect("a manifest that establishes something")
    .with_independence(OracleIndependence::independent())
}

fn project_plane(plane: Plane) -> EvidencePlane {
    match plane {
        Plane::Artifact => EvidencePlane::Artifact,
        Plane::Analytical => EvidencePlane::Analytical,
        Plane::Measurement => EvidencePlane::Measurement,
        Plane::Biological => EvidencePlane::Biological,
        Plane::Causal => EvidencePlane::Causal,
        Plane::Longitudinal => EvidencePlane::Longitudinal,
        Plane::Translational => EvidencePlane::Translational,
        Plane::Policy => EvidencePlane::Policy,
    }
}

fn project_tier(tier: EvidenceTier) -> IrTier {
    match tier {
        EvidenceTier::Deterministic => IrTier::Deterministic,
        EvidenceTier::Execution => IrTier::Execution,
        EvidenceTier::Property => IrTier::Property,
        EvidenceTier::Statistical => IrTier::Statistical,
        EvidenceTier::Judge => IrTier::Judge,
    }
}

fn project_oracle(manifest: &OracleManifest) -> Projection<OracleIr> {
    let value = OracleIr {
        oracle_id: manifest.oracle.id.as_str().to_string(),
        kind: manifest.kind().to_string(),
        version: manifest.oracle.version.to_string(),
        tier: project_tier(manifest.declared_tier),
        inputs: BTreeSet::new(),
        outputs: BTreeSet::new(),
        establishes: manifest.establishes.iter().copied().map(project_plane).collect(),
        cannot_establish: manifest
            .cannot_establish
            .iter()
            .copied()
            .map(project_plane)
            .collect(),
        evidence_basis: format!("{:?}", manifest.uncertainty_model),
        failure_conditions: manifest.known_failure_modes.clone(),
        priority: 0,
        calibration: format!("{:?}", manifest.uncertainty_model),
        independence: Independence {
            from_evaluated_system: manifest.independence.from_evaluated_system,
            shared_resources: manifest
                .independence
                .shared
                .iter()
                .map(|resource| format!("{resource:?}"))
                .collect(),
        },
    };

    let gaps = [
        ("inputs", "an OracleManifest declares planes, not an input schema"),
        ("outputs", "the same; the trait's evaluate signature carries the shapes"),
        (
            "priority",
            "priority within a mesh is MeshPolicy's, not the manifest's; the projection defaults it to 0",
        ),
    ]
    .into_iter()
    .map(|(field, detail)| ProjectionGap::new("25.18", field, "bioprism_oracle::OracleManifest", detail))
    .collect();

    Projection::with_gaps(value, gaps)
}

#[test]
fn the_ir_evidence_ladder_agrees_with_the_one_bioprism_oracle_enforces() {
    let ladder = [
        EvidenceTier::Deterministic,
        EvidenceTier::Execution,
        EvidenceTier::Property,
        EvidenceTier::Statistical,
        EvidenceTier::Judge,
    ];
    for (index, tier) in ladder.into_iter().enumerate() {
        assert_eq!(
            project_tier(tier),
            IrTier::ALL[index],
            "the IR mirrors the ladder; a mirror that drifted would be worse than no mirror"
        );
        for other in ladder {
            assert_eq!(
                tier.may_override(other),
                project_tier(tier).may_override(project_tier(other)),
                "{tier:?} overriding {other:?} must mean the same in both"
            );
        }
    }
}

#[test]
fn an_oracle_manifest_projects_into_the_ir_and_declares_three_unfillable_fields() {
    let projection = project_oracle(&oracle_manifest());
    projection.value.validate().expect("the projection is valid");
    assert_eq!(
        projection.unfilled_fields(),
        vec!["inputs", "outputs", "priority"]
    );
}

#[test]
fn the_planes_bioprism_oracle_names_all_have_an_ir_counterpart() {
    for plane in Plane::ALL {
        let projected = project_plane(plane);
        assert_eq!(
            format!("{projected:?}").to_lowercase(),
            format!("{plane:?}").to_lowercase(),
            "the eight planes are the same eight"
        );
    }
}

// --- 25.19: mutations -----------------------------------------------------------------------------

fn project_mutation(mutation: &Mutation) -> Projection<MutationProgram> {
    let relation = match mutation.relation {
        Relation::PreservesVerdict => SemanticRelation::Preserving,
        Relation::AddsWitness { .. } | Relation::RemovesWitness { .. } => SemanticRelation::Changing,
    };
    let value = MutationProgram {
        mutation_id: MutationId::parse(&mutation.id).expect("well-formed"),
        parent: Some(mutation.family()),
        applicability: format!("{:?}", mutation.kind),
        seed: SeedDeclaration::Deterministic,
        transformations: Vec::new(),
        relation,
        oracle_changes: if relation.requires_oracle_change() {
            BTreeSet::from(["bioprism-fiber split-integrity oracle".to_string()])
        } else {
            BTreeSet::new()
        },
        validations: Vec::new(),
        risk: Risk::Substantive,
        generator_version: "declared by the caller, not by the mutation".to_string(),
    };

    let gaps = [
        (
            "seed",
            "apply() is a pure function of world and mutation, so the crate needs no seed; the generator that samples a family does, and it is not this object",
        ),
        (
            "generator_version",
            "a property of the run, not of the mutation",
        ),
        ("risk", "not modelled anywhere in bioprism-mutation"),
        (
            "oracle_changes",
            "Relation says whether the verdict must move, but never which oracle produced it",
        ),
    ]
    .into_iter()
    .map(|(field, detail)| ProjectionGap::new("25.19", field, "bioprism_mutation::Mutation", detail))
    .collect();

    Projection::with_gaps(value, gaps)
}

#[test]
fn a_semantics_preserving_mutation_projects_and_validates() {
    let mutation = Mutation::new(
        "mut-rename",
        MutationKind::RenameSubjects {
            prefix: "ALT".to_string(),
        },
    );
    let projection = project_mutation(&mutation);
    assert_eq!(projection.value.relation, SemanticRelation::Preserving);
    projection.value.validate().expect("the projection is valid");
}

#[test]
fn a_leakage_injecting_mutation_projects_as_semantics_changing() {
    let mutation = Mutation::new(
        "mut-inject",
        MutationKind::InjectLeakage {
            mechanism: bioprism_mutation::Mechanism::ALL[0],
        },
    );
    let projection = project_mutation(&mutation);
    assert_eq!(projection.value.relation, SemanticRelation::Changing);
    projection.value.validate().expect("the projection is valid");
}

#[test]
fn four_of_the_nine_fields_25_19_requires_have_no_field_on_a_mutation() {
    let mutation = Mutation::new("mut-1", MutationKind::CamouflageTags);
    let projection = project_mutation(&mutation);
    assert_eq!(
        projection.unfilled_fields(),
        vec!["generator_version", "oracle_changes", "risk", "seed"]
    );
}

#[test]
fn the_seed_a_mutation_does_carry_lives_inside_its_kind_not_beside_it() {
    let reorder = Mutation::new("mut-reorder", MutationKind::ReorderFacts { seed: 7 });
    let MutationKind::ReorderFacts { seed } = reorder.kind else {
        panic!("expected a reorder");
    };
    assert_eq!(
        seed, 7,
        "some kinds carry a seed as a parameter; the mutation as a whole does not have one, \
         so the IR's seed field cannot be filled uniformly"
    );
}

// --- 25.20: bundles -------------------------------------------------------------------------------

#[test]
fn the_platform_cannot_produce_the_signature_25_20_requires() {
    let scheme = AuthenticationScheme::SymmetricSharedSecret;
    assert_eq!(scheme.algorithm(), "hmac-sha256");

    let repudiability = Repudiability::ForgeableByAnyVerifier;
    match repudiability {
        Repudiability::ForgeableByAnyVerifier => {}
    }
    assert_eq!(
        IrRepudiability::ForgeableByAnyVerifier,
        project_repudiability(repudiability),
        "the IR mirrors the single variant bioprism-bundle admits"
    );
}

fn project_repudiability(repudiability: Repudiability) -> IrRepudiability {
    match repudiability {
        Repudiability::ForgeableByAnyVerifier => IrRepudiability::ForgeableByAnyVerifier,
    }
}

#[test]
fn a_mac_tag_always_carries_its_algorithm_prefix_so_it_cannot_be_quoted_as_a_signature() {
    let text = format!("hmac-sha256:{}", "ab".repeat(32));
    let tag = MacTag::parse_prefixed_hex(&text).expect("well-formed tag");
    assert_eq!(tag.to_prefixed_hex(), text);
    assert!(
        MacTag::parse_prefixed_hex(&"ab".repeat(32)).is_err(),
        "an unlabelled 64-hex blob is indistinguishable from a SHA-256 digest"
    );
}

#[test]
fn the_bio_result_bundle_ir_has_no_field_called_signature() {
    let ir = serde_json::to_value(bioprism_biolang::bundle::Attestation {
        tag: "hmac-sha256:00".to_string(),
        claimed_producer: "lab-a".to_string(),
        evidence_scope: BTreeSet::from(["table".to_string()]),
        repudiability: IrRepudiability::ForgeableByAnyVerifier,
    })
    .expect("serializes");
    let object = ir.as_object().expect("an object");
    assert!(
        !object.contains_key("signature"),
        "25.20 requires signatures; naming a MAC tag one would let a reader conclude non-repudiation"
    );
    assert!(object.contains_key("repudiability"));
}
