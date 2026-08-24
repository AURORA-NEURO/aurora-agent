//! Three non-biological domains compiled end to end, one per verdict.
//!
//! These are the proof that the pipeline generalises: the same closure, slice, temporal-cut and
//! certificate machinery that judges the radiogenomic reference world here judges a trading
//! session, a privilege review and a software release — and the honesty properties survive the
//! trip. The privilege world is the important one: its decisive evidence is withheld by the
//! temporal cut, and the verdict is `underdetermined`, not `valid`. Before
//! `compile_with_oracle` existed, that world would have compiled to `valid` with an empty
//! witness list.

use bioprism_domain::DomainPack;
use bioprism_fiber::{compile_with_oracle, CompileOutput, Query};
use bioprism_section::{CertificateProfile, LeakageWitness, OracleStatus};
use bioprism_world::World;
use serde_json::Value;
use std::path::PathBuf;

fn fixture(domain: &str, file: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "domains",
        domain,
        file,
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

fn compile_domain(domain: &str) -> (DomainPack, CompileOutput) {
    let pack = DomainPack::from_json(&fixture(domain, "domain.json")).expect("pack loads");
    let world = World::from_json(fixture(domain, "world.json")).expect("world loads");
    let query = Query::from_json(fixture(domain, "query.json")).expect("query loads");
    let out = compile_with_oracle(&world, &query, pack.oracle()).expect("compiles");
    (pack, out)
}

#[test]
fn the_trade_surveillance_world_is_judged_invalid_with_a_checkable_witness() {
    let (_, out) = compile_domain("trade-surveillance");

    assert_eq!(out.certificate.oracle.status, OracleStatus::Invalid);
    assert_eq!(out.certificate.oracle.oracle_kind, "rule/trade-surveillance-v1");
    assert_eq!(out.certificate.oracle.witnesses.len(), 1);

    let LeakageWitness::DomainCheck {
        check,
        observed,
        detail,
    } = &out.certificate.oracle.witnesses[0]
    else {
        panic!("expected a domain_check witness");
    };
    assert_eq!(check, "self_cross");
    assert_eq!(
        observed.get("self_match_conflicts").map(String::as_str),
        Some(r#"[{"account":"ACC-9","buy_order":"ORD-1201","sell_order":"ORD-1288"}]"#),
        "the witness carries the observed binding, canonically rendered"
    );
    assert!(detail.contains("same beneficial account"));

    // The compiled region is the decision's region, not the corpus: the six exploratory
    // market-colour facts are omitted and the protected closure is fully delivered.
    assert_eq!(out.certificate.selected_facts.len(), 4);
    assert_eq!(out.certificate.omissions.total_facts, 6);
    assert!(out.trace.dropped_protected.is_empty());
}

#[test]
fn the_privilege_review_world_abstains_because_the_disclosure_log_postdates_the_decision() {
    let (_, out) = compile_domain("privilege-review");

    assert_eq!(out.certificate.oracle.status, OracleStatus::Underdetermined);
    assert_eq!(out.certificate.oracle.witnesses.len(), 1);
    let LeakageWitness::DomainCheck {
        check,
        observed,
        detail,
    } = &out.certificate.oracle.witnesses[0]
    else {
        panic!("expected a domain_check witness");
    };
    assert_eq!(check, "required_evidence");
    assert_eq!(
        observed.get("third_party_disclosures").map(String::as_str),
        Some("absent")
    );
    assert!(detail.contains("third_party_disclosures"));

    // The gap is the temporal cut's own doing, and the certificate says so: the disclosure
    // fact was selected, then withheld as not yet available at the decision time.
    assert_eq!(
        out.certificate.omissions.inaccessible_selected_before_cut,
        vec!["fact.third_party_disclosures".to_string()]
    );

    // The abstention survives serialisation on the extended profile, where 43.28 reports it.
    let encoded = out
        .certificate
        .to_json(CertificateProfile::Extended)
        .expect("serialises");
    assert_eq!(encoded["oracle"]["status"], Value::from("underdetermined"));
}

#[test]
fn the_supply_chain_world_is_judged_valid_with_full_protected_closure() {
    let (_, out) = compile_domain("supply-chain");

    assert_eq!(out.certificate.oracle.status, OracleStatus::Valid);
    assert!(out.certificate.oracle.witnesses.is_empty());
    assert_eq!(out.certificate.protected_closure.len(), 3);
    assert!(out.trace.dropped_protected.is_empty());
    assert_eq!(out.section.goal, "Decide whether release candidate 2025.05 meets the supply-chain integrity bar.");
}

#[test]
fn a_domain_compile_is_deterministic_byte_for_byte() {
    for domain in ["trade-surveillance", "privilege-review", "supply-chain"] {
        let (_, first) = compile_domain(domain);
        let (_, second) = compile_domain(domain);
        for profile in [CertificateProfile::Reference, CertificateProfile::Extended] {
            assert_eq!(
                first.certificate.digest(profile).unwrap().as_str(),
                second.certificate.digest(profile).unwrap().as_str(),
                "{domain} certificate digest moved between identical compiles"
            );
        }
        assert_eq!(
            first.section.to_canonical_string().unwrap(),
            second.section.to_canonical_string().unwrap(),
            "{domain} section bytes moved between identical compiles"
        );
    }
}

#[test]
fn every_fixture_pack_classifies_every_scope_dimension_its_world_uses() {
    for domain in ["trade-surveillance", "privilege-review", "supply-chain"] {
        let pack = DomainPack::from_json(&fixture(domain, "domain.json")).expect("pack loads");
        let registry = pack.dimension_registry();
        let world = fixture(domain, "world.json");
        for fact in world["facts"].as_array().expect("facts") {
            for dimension in fact["scope"].as_object().expect("scope").keys() {
                assert!(
                    registry.classify(dimension).is_classified(),
                    "{domain}: dimension {dimension:?} is unclassified under its own pack"
                );
            }
        }
    }
}
