//! Blueprint 15.00 and 29.00 — the encoded portfolio, and what the coverage matrix says is
//! missing.

use bioprism_packs::{
    coverage, matrix, portfolio, portfolio_coverage, AgentCapability, BioCapability,
    CapabilityFamily, Domain, OracleTier, PackAxis, PackDefinition, PackVersion, ReleaseWave,
    SchemaRange,
};

/// A pack that only a human can judge. Nothing in the real portfolio looks like this, so the weak
/// coverage path needs a constructed example rather than a fixture drawn from the blueprint.
static RUBRIC_ONLY: PackDefinition = PackDefinition {
    id: "test.rubric-only",
    title: "Rubric-only pack",
    blueprint_module: "test",
    axis: PackAxis::Mechanism,
    measures: "Nothing that can be re-run.",
    capabilities: &[CapabilityFamily::Agent(AgentCapability::HumanCollaboration)],
    domains: &[Domain::Coding],
    decision_families: &["ask a question"],
    oracles: &[OracleTier::Rubric, OracleTier::ExpertReview],
    release_wave: ReleaseWave::Unsequenced,
};

#[test]
fn the_portfolio_encodes_every_numbered_module_of_blueprint_section_15() {
    let encoded: Vec<&str> = portfolio::section_15()
        .iter()
        .map(|p| p.blueprint_module)
        .collect();

    for number in 1..=25 {
        let module = format!("15.{number:02}");
        assert_eq!(
            encoded.iter().filter(|m| **m == module).count(),
            1,
            "blueprint module {module} should be encoded exactly once"
        );
    }
    assert_eq!(
        encoded.len(),
        25,
        "section 15 has twenty-five numbered pack modules; `00` is the portfolio spec, not a pack"
    );
}

#[test]
fn the_portfolio_encodes_every_numbered_module_of_blueprint_section_29() {
    let encoded: Vec<&str> = portfolio::section_29()
        .iter()
        .map(|p| p.blueprint_module)
        .collect();

    for number in 1..=21 {
        let module = format!("29.{number:02}");
        assert_eq!(
            encoded.iter().filter(|m| **m == module).count(),
            1,
            "blueprint module {module} should be encoded exactly once"
        );
    }
    assert_eq!(encoded.len(), 21);
    assert_eq!(portfolio::all().len(), 46);
}

#[test]
fn every_pack_has_a_unique_parseable_id_a_construct_and_at_least_one_oracle() {
    let mut seen: Vec<&str> = Vec::new();
    for pack in portfolio::all() {
        assert!(
            bioprism_packs::PackId::parse(pack.id).is_ok(),
            "`{}` is not a well-formed pack id",
            pack.id
        );
        assert!(!seen.contains(&pack.id), "duplicate pack id `{}`", pack.id);
        seen.push(pack.id);

        assert!(
            !pack.capabilities.is_empty(),
            "{} claims no capability",
            pack.id
        );
        assert!(!pack.oracles.is_empty(), "{} declares no oracle", pack.id);
        assert!(!pack.domains.is_empty(), "{} names no domain", pack.id);
        assert!(
            !pack.decision_families.is_empty(),
            "{} mines no decision family",
            pack.id
        );
        assert!(
            pack.measures.len() > 40,
            "{} states a construct too short to be a construct: {}",
            pack.id,
            pack.measures
        );
        assert!(pack.strongest_oracle().is_some());
    }
}

#[test]
fn every_pack_definition_promotes_to_a_manifest_that_validates() {
    for pack in portfolio::all() {
        let manifest = pack
            .to_manifest(
                PackVersion::new(0, 1, 0),
                SchemaRange::new(1, 1),
                vec!["prism-core".into()],
                "Apache-2.0",
            )
            .expect("a portfolio definition yields a manifest");
        manifest
            .validate()
            .unwrap_or_else(|e| panic!("{} produced an invalid manifest: {e}", pack.id));
        assert_eq!(manifest.blueprint_module, pack.blueprint_module);
        assert_eq!(manifest.measures, pack.measures);
    }
}

#[test]
fn the_whole_portfolio_leaves_no_capability_family_without_a_pack() {
    let report = portfolio_coverage();
    assert!(
        report.uncovered.is_empty(),
        "unexpected gaps: {:?}",
        report.uncovered
    );
    assert_eq!(report.rows.len(), CapabilityFamily::all().len());
}

#[test]
fn families_covered_by_exactly_one_pack_are_reported_as_single_points_of_failure() {
    let report = portfolio_coverage();

    for family in [
        CapabilityFamily::Agent(AgentCapability::HumanCollaboration),
        CapabilityFamily::Biology(BioCapability::AssayUnderstanding),
        CapabilityFamily::Biology(BioCapability::VerificationAndAbstention),
    ] {
        assert!(
            report.singly_covered.contains(&family),
            "{} rests on one pack and should be reported as such",
            family.code()
        );
        assert_eq!(report.row(family).unwrap().pack_count(), 1);
    }
    assert!(report.gap_summary().contains("single pack"));
}

#[test]
fn a_capability_family_with_no_pack_appears_in_the_gap_list() {
    let biology_only: Vec<&PackDefinition> = portfolio::section_29();
    let report = coverage(&biology_only);

    for family in [
        CapabilityFamily::Agent(AgentCapability::ToolUse),
        CapabilityFamily::Agent(AgentCapability::Memory),
        CapabilityFamily::Agent(AgentCapability::Routing),
        CapabilityFamily::Agent(AgentCapability::Observability),
    ] {
        assert!(
            report.uncovered.contains(&family),
            "{} has no biological pack and belongs in the gap list",
            family.code()
        );
        assert!(!report.is_covered(family));
    }
    assert!(report.gap_summary().contains("Uncovered: A00, A01, A02"));
}

#[test]
fn a_family_judged_only_by_human_review_is_reported_as_weakly_covered() {
    let report = coverage(&[&RUBRIC_ONLY]);
    let family = CapabilityFamily::Agent(AgentCapability::HumanCollaboration);

    assert!(report.is_covered(family));
    assert!(report.weakly_covered.contains(&family));

    let row = report.row(family).unwrap();
    assert!(!row.grounded);
    assert_eq!(row.strongest_oracle, Some(OracleTier::ExpertReview));

    let real = portfolio_coverage();
    assert!(
        real.weakly_covered.is_empty(),
        "every blueprint pack declares at least one execution-grounded oracle"
    );
}

#[test]
fn removing_an_unhealthy_pack_turns_a_covered_family_into_a_gap() {
    let sole_coverer = "prism.human-collaboration";
    let family = CapabilityFamily::Agent(AgentCapability::HumanCollaboration);
    assert!(portfolio_coverage().is_covered(family));

    let healthy: Vec<&PackDefinition> = portfolio::all()
        .iter()
        .filter(|p| p.id != sole_coverer)
        .collect();
    let report = coverage(&healthy);

    assert!(
        report.uncovered.contains(&family),
        "retiring the only pack covering {} must show as a gap, not as one fewer pack",
        family.code()
    );
}

#[test]
fn the_blueprint_release_order_sequences_only_part_of_the_portfolio() {
    let sequenced = portfolio::release_order();
    let unsequenced = portfolio::unsequenced();

    assert_eq!(sequenced.len() + unsequenced.len(), portfolio::all().len());
    assert_eq!(
        sequenced.len(),
        13,
        "15.00 sequences eight waves over thirteen packs"
    );
    assert_eq!(sequenced.first().unwrap().blueprint_module, "15.01");
    assert_eq!(
        sequenced.first().unwrap().release_wave,
        ReleaseWave::Wave(1)
    );

    assert!(
        portfolio::section_29()
            .iter()
            .all(|p| !p.release_wave.is_sequenced()),
        "section 29 gives no release order, and none is invented here"
    );
}

#[test]
fn packs_sharing_a_capability_signature_are_surfaced_for_redundancy_review() {
    let groups = portfolio::duplicate_signatures();
    assert!(!groups.is_empty());

    let shares_signature = |left: &str, right: &str| {
        groups
            .iter()
            .any(|(_, ids)| ids.contains(&left) && ids.contains(&right))
    };
    assert!(
        shares_signature("bio.statistical-estimands", "bio.causal-inference"),
        "both claim B5 in the biomedical domain and differ only in decision families"
    );
    assert!(shares_signature(
        "bio.experiment-design",
        "bio.value-of-information"
    ));
}

#[test]
fn the_platform_axis_is_marked_as_an_extension_rather_than_blueprint_text() {
    assert!(PackAxis::Mechanism.is_blueprint_axis());
    assert!(PackAxis::Domain.is_blueprint_axis());
    assert!(!PackAxis::Platform.is_blueprint_axis());

    let platform: Vec<&str> = portfolio::by_axis(PackAxis::Platform)
        .iter()
        .map(|p| p.blueprint_module)
        .collect();
    assert!(platform.contains(&"15.19"));
    assert!(platform.contains(&"15.24"));
    assert!(!platform.contains(&"15.05"));
}

#[test]
fn added_domains_are_distinguishable_from_the_six_the_blueprint_names() {
    assert!(Domain::Coding.is_blueprint_domain());
    assert!(Domain::Neuroscience.is_blueprint_domain());
    assert!(!Domain::Operations.is_blueprint_domain());
    assert!(!Domain::Evaluation.is_blueprint_domain());
}

#[test]
fn the_coverage_matrix_indexes_packs_on_both_the_capability_and_domain_axes() {
    let packs: Vec<&PackDefinition> = portfolio::all().iter().collect();
    let cells = matrix(&packs);

    let evidence_in_coding = cells
        .iter()
        .find(|c| {
            c.family == CapabilityFamily::Agent(AgentCapability::EvidenceAcquisition)
                && c.domain == Domain::Coding
        })
        .expect("evidence acquisition is exercised in coding worlds");
    assert!(evidence_in_coding
        .packs
        .contains(&"prism.context-acquisition".to_string()));
    assert!(evidence_in_coding
        .packs
        .contains(&"prism.coding-repository-inference".to_string()));

    assert!(
        !cells.iter().any(|c| c.packs.is_empty()),
        "empty cells are omitted rather than published as zeros"
    );
}

#[test]
fn biology_capability_codes_are_blueprint_identifiers_and_agent_codes_are_not() {
    let codes: Vec<&str> = BioCapability::ALL.iter().map(|c| c.code()).collect();
    assert_eq!(
        codes,
        vec!["B0", "B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B9", "B10", "B11", "B12"]
    );

    for capability in BioCapability::ALL {
        assert!(CapabilityFamily::Biology(*capability).code_is_from_blueprint());
    }
    for capability in AgentCapability::ALL {
        assert!(
            !CapabilityFamily::Agent(*capability).code_is_from_blueprint(),
            "section 15 numbers no capability nodes, so {} must not be cited as blueprint text",
            capability.code()
        );
    }
}

#[test]
fn oracle_strength_orders_execution_grounded_tiers_above_human_judgement() {
    for grounded in [
        OracleTier::Deterministic,
        OracleTier::Executable,
        OracleTier::PolicyVeto,
    ] {
        assert!(grounded.is_execution_grounded());
        assert!(!grounded.is_nondeterministic());
        for soft in [OracleTier::ExpertReview, OracleTier::Rubric] {
            assert!(grounded.strength() > soft.strength());
        }
    }
    assert!(OracleTier::Statistical.strength() > OracleTier::ExpertReview.strength());
    assert!(!OracleTier::Statistical.is_execution_grounded());
}

#[test]
fn a_pack_is_found_by_id_and_an_unknown_id_is_a_typed_error() {
    let pack = portfolio::find("prism.benchmark-meta-evaluation").unwrap();
    assert_eq!(pack.blueprint_module, "15.19");
    assert!(pack.covers(CapabilityFamily::Agent(
        AgentCapability::EvaluationIntegrity
    )));

    assert!(matches!(
        portfolio::find("prism.does-not-exist"),
        Err(bioprism_packs::PackError::UnknownPack(_))
    ));
}
