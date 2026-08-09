//! Blueprint 03.06 — the pack document: content addressing, the trust boundary, and the counts
//! it is allowed to publish.

use bioprism_packs::{
    AgentCapability, CapabilityFamily, ContentHash, Domain, InstanceSource, OracleTier, PackAxis,
    PackContent, PackDependency, PackError, PackId, PackIr, PackManifest, PackVersion,
    ParentEnvironment, SchemaRange, SeedRange, WorldId,
};

fn manifest() -> PackManifest {
    PackManifest {
        id: PackId::parse("prism.context-acquisition").unwrap(),
        version: PackVersion::new(0, 1, 0),
        schema_range: SchemaRange::new(1, 2),
        title: "Context Acquisition and Evidence Value".into(),
        measures: "Whether an agent seeks the smallest, highest-value evidence and then stops."
            .into(),
        blueprint_module: "15.01".into(),
        axis: PackAxis::Mechanism,
        capabilities: vec![CapabilityFamily::Agent(
            AgentCapability::EvidenceAcquisition,
        )],
        domains: vec![Domain::Coding, Domain::Science],
        owners: vec!["prism-core".into()],
        license: "Apache-2.0".into(),
        dependencies: Vec::new(),
    }
}

fn content(instances: InstanceSource, oracles: Vec<OracleTier>) -> PackContent {
    PackContent {
        parent_environments: vec![
            ParentEnvironment {
                world: WorldId::parse("world-repo-debug-001").unwrap(),
                decision_parents: 18,
            },
            ParentEnvironment {
                world: WorldId::parse("world-incident-002").unwrap(),
                decision_parents: 24,
            },
        ],
        decision_families: vec!["choose which artifact to inspect next".into()],
        mutation_relations: vec![
            "rename-and-relocate".into(),
            "insert-stale-distractor".into(),
        ],
        oracles,
        instances,
        executed_trials: 4_200,
        independent_reproductions: 2,
        effective_sample_size: Some(96),
    }
}

fn pack() -> PackIr {
    PackIr {
        manifest: manifest(),
        content: content(
            InstanceSource::Authored { validated: 512 },
            vec![OracleTier::Deterministic, OracleTier::Executable],
        ),
    }
}

#[test]
fn a_pack_round_trips_through_json_without_changing_its_content_hash() {
    let original = pack();
    let encoded = serde_json::to_string(&original).unwrap();
    let decoded = PackIr::parse(&encoded).unwrap();

    assert_eq!(decoded, original);
    assert_eq!(decoded.digest().unwrap(), original.digest().unwrap());
}

#[test]
fn two_documents_differing_only_in_key_order_address_the_same_pack() {
    let original = pack();
    let compact = serde_json::to_string(&original).unwrap();
    let reordered: serde_json::Value = serde_json::from_str(&compact).unwrap();
    let pretty = serde_json::to_string_pretty(&reordered).unwrap();

    let from_pretty = PackIr::parse(&pretty).unwrap();
    assert_eq!(from_pretty.digest().unwrap(), original.digest().unwrap());
}

#[test]
fn changing_a_single_oracle_tier_changes_the_pack_address() {
    let strong = pack();
    let mut weakened = pack();
    weakened.content.oracles = vec![OracleTier::Rubric];

    assert_ne!(weakened.digest().unwrap(), strong.digest().unwrap());
}

#[test]
fn a_manifest_parses_even_when_the_content_section_is_malformed() {
    let document = format!(
        r#"{{"manifest":{},"content":{{"parent_environments":"not an array","oracles":42}}}}"#,
        serde_json::to_string(&manifest()).unwrap()
    );

    assert!(PackIr::parse(&document).is_err());

    let parsed =
        PackIr::parse_manifest_first(&document).expect("metadata is parsed before content");
    assert_eq!(parsed.id.as_str(), "prism.context-acquisition");
    assert_eq!(parsed.license, "Apache-2.0");
}

#[test]
fn a_document_without_a_manifest_is_a_typed_error_rather_than_an_empty_manifest() {
    let error = PackIr::parse_manifest_first(r#"{"content":{}}"#).unwrap_err();
    assert!(matches!(error, PackError::MissingManifest));
}

#[test]
fn a_dependency_without_a_digest_is_rejected_rather_than_resolved_by_name() {
    let mut unpinned = pack();
    unpinned.manifest.dependencies = vec![PackDependency {
        id: PackId::parse("prism.tool-selection").unwrap(),
        digest: None,
    }];

    let error = unpinned.validate().unwrap_err();
    assert!(matches!(error, PackError::UnpinnedDependency { .. }));

    let digest = pack().digest().unwrap();
    let mut pinned = pack();
    pinned.manifest.dependencies = vec![PackDependency::pinned(
        PackId::parse("prism.tool-selection").unwrap(),
        digest,
    )];
    assert!(pinned.validate().is_ok());
}

#[test]
fn declaring_more_instances_than_the_seed_range_can_materialize_is_rejected() {
    let mut overstated = pack();
    overstated.content.instances = InstanceSource::DeterministicGenerator {
        seeds: SeedRange::new(0, 10_000),
        declared: 1_000_000,
        validated: 2_000,
    };

    let error = overstated.validate().unwrap_err();
    match error {
        PackError::CountsExceedGenerator {
            declared,
            available,
            ..
        } => {
            assert_eq!(declared, 1_000_000);
            assert_eq!(available, 10_000);
        }
        other => panic!("expected a generator-capacity error, got {other:?}"),
    }
}

#[test]
fn validating_more_instances_than_were_declared_is_rejected() {
    let mut impossible = pack();
    impossible.content.instances = InstanceSource::AdapterImport {
        adapter: "harbor".into(),
        declared: 100,
        validated: 101,
    };

    assert!(matches!(
        impossible.validate().unwrap_err(),
        PackError::ValidatedExceedsDeclared { .. }
    ));
}

#[test]
fn a_pack_that_claims_no_capability_is_rejected_because_its_score_would_be_uninterpretable() {
    let mut anonymous = pack();
    anonymous.manifest.capabilities.clear();

    assert!(matches!(
        anonymous.validate().unwrap_err(),
        PackError::NoCapabilityClaim(_)
    ));
}

#[test]
fn a_pack_with_no_oracle_declares_nothing_that_could_decide_an_instance() {
    let mut undecidable = pack();
    undecidable.content.oracles.clear();

    assert!(matches!(
        undecidable.validate().unwrap_err(),
        PackError::NoOracle(_)
    ));
}

#[test]
fn an_inverted_schema_range_is_rejected_rather_than_silently_accepting_every_runtime() {
    let mut inverted = pack();
    inverted.manifest.schema_range = SchemaRange::new(5, 2);

    assert!(matches!(
        inverted.validate().unwrap_err(),
        PackError::EmptySchemaRange { .. }
    ));
    assert!(!SchemaRange::new(5, 2).accepts(3));
    assert!(SchemaRange::new(1, 2).accepts(2));
}

#[test]
fn malformed_pack_ids_are_typed_errors_not_silently_accepted_strings() {
    for bad in [
        "",
        "PRISM.Context",
        "no-namespace",
        "9lives.pack",
        ".leading",
        "a..b",
    ] {
        assert!(
            matches!(PackId::parse(bad), Err(PackError::MalformedPackId(_))),
            "`{bad}` should not parse as a pack id"
        );
    }
    assert!(PackId::parse("bio.cross-species-translation").is_ok());
}

#[test]
fn the_public_count_headline_never_states_an_instance_count_without_its_denominators() {
    let counts = pack().content.counts();
    let headline = counts.headline(&PackId::parse("prism.context-acquisition").unwrap());

    assert!(headline.contains("512 validated instances"));
    for denominator in [
        "parent environment",
        "decision parent",
        "equivalence classes",
        "executed trial",
        "independent reproduction",
    ] {
        assert!(
            headline.contains(denominator),
            "headline dropped `{denominator}`: {headline}"
        );
    }
    assert_eq!(counts.parent_environments, 2);
    assert_eq!(counts.decision_parents, 42);
}

#[test]
fn an_unmeasured_effective_sample_size_is_reported_as_unmeasured_not_as_the_instance_count() {
    let mut unmeasured = pack();
    unmeasured.content.effective_sample_size = None;
    let counts = unmeasured.content.counts();
    let headline = counts.headline(&PackId::parse("prism.context-acquisition").unwrap());

    assert!(headline.contains("effective sample size not measured"));
    assert!(!headline.contains("512 independent"));
}

#[test]
fn a_generator_pack_reports_the_fraction_of_its_headline_that_actually_exists() {
    let mut generated = pack();
    generated.content.instances = InstanceSource::DeterministicGenerator {
        seeds: SeedRange::new(0, 2_000_000),
        declared: 1_000_000,
        validated: 2_000,
    };
    assert!(generated.validate().is_ok());

    let counts = generated.content.counts();
    assert_eq!(counts.declared_instances, 1_000_000);
    assert_eq!(counts.validated_instances, 2_000);
    let fraction = counts.materialized_fraction().unwrap();
    assert!((fraction - 0.002).abs() < 1e-12);
}

#[test]
fn a_content_hash_parsed_from_a_pack_is_a_well_formed_sha256() {
    let digest = pack().digest().unwrap();
    assert_eq!(digest.as_str().len(), 64);
    assert_eq!(ContentHash::parse(digest.to_string()).unwrap(), digest);
}
