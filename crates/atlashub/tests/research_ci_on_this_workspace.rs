//! Research CI (34.20) run against this workspace's own artifacts, plus one end-to-end pass
//! through the four other modules.
//!
//! 34.20 is the only module in this crate's remit whose predicates can be pointed at something real
//! that already exists, so they are. The observations below are read from files rather than
//! invented: dependency pinning comes from the workspace manifest, and the statement of what is not
//! established comes from this crate's own module documentation.
//!
//! The honest outcome is that two checks pass and six are undetermined, and the suite reports the
//! artifact as **not publishable** because of it. That is the correct answer. This workspace
//! publishes no figures, runs no decision-cell regression suite, records no egress events, has no
//! machine-identified claims and references no world card, so six of the eight checks have nothing
//! to look at — and 34.20's whole point is that "nothing to look at" is not "fine".

use bioprism_atlashub::card::{Ancestry, AncestryStep, ClaimKind, ProvenanceRung, WorldCardDraft};
use bioprism_atlashub::ci::{Check, CheckOutcome, CiReport, Observation, Publishability};
use bioprism_atlashub::connector::{
    select, AuthMode, Conformance, Connector, ConnectorId, Egress, Fetch, Use,
};
use bioprism_atlashub::federated::{
    pool, Aggregate, DataOrigin, Evaluation, SiteParticipation, SiteResult, SmallCellPolicy,
    Unchecked,
};
use bioprism_atlashub::voe::{
    pareto_front, Budget, DeclaredValue, Experiment, ExperimentId, Hypothesis,
};
use bioprism_hub::{AccessTier, Licence};
use bioprism_hubapi::federation::TrustStanding;
use bioprism_hubapi::RegistryId;
use bioprism_ids::{ContentHash, WorldId};
use bioprism_scope::{LossLedger, MappingKind, ScopeKey, ScopeMapping};
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/atlashub sits two levels below the workspace root")
        .to_path_buf()
}

/// Reads `[workspace.dependencies]` and reports, per entry, whether the version it admits is exact.
///
/// A path dependency counts as pinned: it resolves inside this tree and moves only when the tree
/// does. A version requirement counts as pinned only when it begins with `=`, because every other
/// cargo operator admits a range, and a range is the thing 34.20's "dependency and environment
/// changes" check exists to catch.
fn dependency_observations(root: &Path) -> Vec<Observation> {
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("workspace manifest");
    let mut observations = Vec::new();
    let mut inside = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[workspace.dependencies]";
            continue;
        }
        if !inside || line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim().to_string(), value.trim());

        let pinned = if value.contains("path =") {
            Some(true)
        } else if let Some(rest) = value.split_once("version =").map(|(_, r)| r.trim()) {
            Some(rest.starts_with("\"="))
        } else {
            value
                .strip_prefix('"')
                .map(|literal| literal.starts_with('='))
        };

        observations.push(Observation::Dependency { name, pinned });
    }
    assert!(
        observations.len() > 10,
        "the workspace manifest parse found only {} dependencies, which means the parse broke \
         rather than that the workspace shrank",
        observations.len()
    );
    observations
}

/// This crate's own statement of what it does not establish, read from its module documentation.
fn non_claim_observation() -> Observation {
    let lib = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("lib.rs"),
    )
    .expect("this crate's lib.rs");
    let marker = "# What is not implemented, and will not be";
    let statement = if lib.contains(marker) {
        "bioprism-atlashub documents, in its crate-level module docs, that it implements no server, \
         no UI, no network, no clock, no estimators and no cryptography"
            .to_string()
    } else {
        String::new()
    };
    Observation::NonClaim { statement }
}

#[test]
fn the_research_ci_suite_run_against_this_workspace_reports_two_passes_and_six_undetermined() {
    let root = workspace_root();
    let mut result = bioprism_atlashub::ci::ResultUnderReview::of("bioprism workspace");
    result.observations.extend(dependency_observations(&root));
    result.observations.push(non_claim_observation());

    let report = CiReport::full("bioprism workspace", &result);
    for line in report.lines() {
        println!("{line}");
    }

    assert_eq!(
        report.outcomes[&Check::EnvironmentPinned],
        CheckOutcome::Pass,
        "every workspace dependency should be pinned to an exact version or a path"
    );
    assert_eq!(report.outcomes[&Check::NonClaimDeclared], CheckOutcome::Pass);

    match report.publishability() {
        Publishability::Blocked {
            failed,
            undetermined,
        } => {
            assert!(
                failed.is_empty(),
                "no check should fail against this workspace, but these did: {failed:?}"
            );
            assert_eq!(undetermined.len(), 6);
            for check in [
                Check::ClaimResolvesToResult,
                Check::CohortSplitsAreDisjoint,
                Check::FigureReproduces,
                Check::DecisionCellRegression,
                Check::DataPolicyRespected,
                Check::ProvenanceRungDeclared,
            ] {
                assert!(undetermined.contains(&check), "{check} should be undetermined");
            }
        }
        Publishability::Publishable => {
            panic!("a workspace with no figures and no cell regression suite is not publishable")
        }
    }
}

#[test]
fn six_undetermined_checks_block_publication_exactly_as_six_failures_would() {
    let root = workspace_root();
    let mut result = bioprism_atlashub::ci::ResultUnderReview::of("bioprism workspace");
    result.observations.extend(dependency_observations(&root));
    result.observations.push(non_claim_observation());
    assert_ne!(
        CiReport::full("bioprism workspace", &result).publishability(),
        Publishability::Publishable
    );
}

fn glioma_scope() -> ScopeKey {
    ScopeKey::new()
        .exact("disease", "glioma")
        .exact("modality", "mri")
}

#[test]
fn a_publishable_world_card_reaches_a_federated_result_that_still_says_what_it_could_not_check() {
    let ancestry = Ancestry::derived_from(
        &Ancestry::of(
            AncestryStep::new(
                ProvenanceRung::Observed,
                ContentHash::of_bytes(b"registry-extract"),
                Licence::permissive("CC-BY-4.0"),
            )
            .describing("imported from a multi-site registry extract"),
        ),
        AncestryStep::new(
            ProvenanceRung::SemiSynthetic,
            ContentHash::of_bytes(b"graft"),
            Licence::permissive("CC-BY-4.0"),
        )
        .describing("injected a known scanner confounder"),
    );

    let card = WorldCardDraft::new(
        WorldId::parse("world/glioma-confounder@1").unwrap(),
        "1.0.0",
        glioma_scope(),
        ancestry,
    )
    .at_access(AccessTier::Federated)
    .disclaiming_what_the_rung_cannot_support()
    .limited_by("adult only; three sites; no pediatric subgroup")
    .publish()
    .expect("a card that disclaims what its rung cannot support is publishable");

    assert_eq!(card.rung(), ProvenanceRung::SemiSynthetic);
    assert!(card.permits(ClaimKind::CausalEffectInReality).is_some());
    assert!(card.permits(ClaimKind::InjectedStructureRecovery).is_none());
    assert!(card.offerable().is_ok());

    let connector_id = ConnectorId::new("site-a-pacs");
    let connector = Connector::declare(
        connector_id.clone(),
        ["mri".to_string()],
        ScopeMapping {
            from: ScopeKey::new().exact("disease", "glioma"),
            to: glioma_scope(),
            kind: MappingKind::Restriction,
            loss: LossLedger::default(),
        },
        AuthMode::SiteLocal,
        Egress::AggregateOnly,
    )
    .unwrap()
    .with_conformance(
        Conformance::recorded(
            &connector_id,
            "connector-conformance-v1",
            ScopeKey::new().exact("disease", "glioma"),
            40,
            40,
            ContentHash::of_bytes(b"conformance-run"),
        )
        .unwrap(),
    );

    let selection = select(
        &[connector],
        &Fetch::new("mri", glioma_scope(), Use::LoadBearing).needing(Egress::AggregateOnly),
    )
    .expect("a conformant aggregate-only connector serves an aggregate-only load-bearing request");
    assert!(!selection.unverified);

    let standing = |site: &str| -> TrustStanding {
        serde_json::from_str(&format!(
            r#"{{"registry":"{site}","subject":{{"name":"bioatlas/glioma","version":"1.0.0","digest":"sha256:abc"}},"tier":"reviewed","basis":{{"basis":"earned"}}}}"#
        ))
        .unwrap()
    };
    let policy = SmallCellPolicy::of(10).unwrap();
    let sites = ["site-a", "site-b", "site-c"];
    let mut results = Vec::new();
    for (index, site) in sites.iter().enumerate() {
        let participation =
            SiteParticipation::admit(RegistryId::parse(*site).unwrap(), &standing(site)).unwrap();
        results.push(SiteResult::report(
            participation,
            DataOrigin::BringYourOwn { world_card: None },
            Aggregate::new(100, 0.7 + index as f64 * 0.05),
            policy,
            [Unchecked::OracleReaderIdentity],
        ));
    }

    let federated = pool(&results).unwrap();
    assert_eq!(federated.evaluation(), Evaluation::Federated);
    assert!(federated.unchecked().contains(&Unchecked::RecordLevelAudit));
    assert!(federated
        .unchecked()
        .contains(&Unchecked::ProvenanceOfLocalData));
    assert!(federated
        .unchecked()
        .contains(&Unchecked::CohortOverlapAcrossSites));
    assert!(federated.suppressed().is_empty());
    assert!((federated.weighted_mean().unwrap() - 0.75).abs() < 1e-12);

    let review = bioprism_atlashub::ci::ResultUnderReview::of("glioma-confounder")
        .observing(Observation::WorldReference {
            world: card.world().to_string(),
            rung: Some(card.rung()),
        })
        .observing(Observation::NonClaim {
            statement: federated
                .unchecked()
                .iter()
                .map(|u| u.to_string())
                .collect::<Vec<_>>()
                .join("; "),
        });
    assert!(Check::ProvenanceRungDeclared.run(&review).is_pass());
    assert!(Check::NonClaimDeclared.run(&review).is_pass());
}

#[test]
fn choosing_the_next_experiment_needs_no_exchange_rate_and_refuses_an_unpriced_candidate() {
    let hypotheses = vec![
        Hypothesis::open("h1", "the confounder is scanner-driven"),
        Hypothesis::open("h2", "the confounder is site-driven"),
    ];
    let h1 = hypotheses[0].id.clone();

    let biopsy = Experiment::new("repeat-biopsy", Budget::new().spending("tissue", 2))
        .addressing(&h1);
    let biopsy_value =
        DeclaredValue::uncalibrated(&biopsy.id, 8.0, "tumour.board", "eliminates h2 outright")
            .unwrap();
    let phantom = Experiment::new("phantom-scan", Budget::new().spending("tissue", 0))
        .addressing(&h1);
    let phantom_value = DeclaredValue::uncalibrated(
        &phantom.id,
        3.0,
        "physics.lead",
        "separates scanner from site partially",
    )
    .unwrap();

    let candidates = vec![biopsy.worth(biopsy_value), phantom.worth(phantom_value)];
    let purse = Budget::new().spending("tissue", 4);
    let front = pareto_front(&candidates, &hypotheses, &purse).unwrap();
    assert_eq!(front.front.len(), 2, "neither dominates: cheaper but weaker");

    let unpriced = Experiment::new("registry-linkage", Budget::new().spending("tissue", 0))
        .addressing(&h1);
    let with_unpriced = vec![candidates[0].clone(), unpriced];
    let err = pareto_front(&with_unpriced, &hypotheses, &purse).unwrap_err();
    assert_eq!(
        err,
        bioprism_atlashub::ValueError::ValueUndetermined {
            experiment: ExperimentId::new("registry-linkage").to_string()
        }
    );
}
