//! The compile oracle is injectable; the reference oracle stays the default.
//!
//! [`compile`] fixes the oracle to the split-integrity reference, which is what the CPython
//! parity contract pins. [`compile_with_oracle`] exists for worlds whose decision the reference
//! oracle does not know: before it, such a world compiled to `valid` with an empty witness list
//! and read as clean rather than as unjudged — the defect `bioprism-worldgen` records on its
//! `Skeleton` as the reason it carries no `Custom` variant.

use bioprism_fiber::{
    compile, compile_with_oracle, DecisionOracle, FiberError, Query, SplitIntegrityOracle,
};
use bioprism_section::{CertificateProfile, LeakageWitness, OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

fn fixture(relative: &str) -> Value {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        "..",
        "fixtures",
        "fiber-v0.1",
        relative,
    ]
    .iter()
    .collect();
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing fixture {}: {e}", path.display()));
    serde_json::from_str(&text).expect("fixture is valid JSON")
}

fn golden_world() -> World {
    World::from_json(fixture("radiogenomic_world.json")).expect("golden world loads")
}

fn leakage_query() -> Query {
    Query::from_json(fixture("leakage_query.json")).expect("golden query loads")
}

#[test]
fn the_default_compile_and_the_injected_reference_oracle_agree_byte_for_byte() {
    let world = golden_world();
    let query = leakage_query();
    let default_out = compile(&world, &query).expect("default compile succeeds");
    let injected_out =
        compile_with_oracle(&world, &query, &SplitIntegrityOracle).expect("injected compile succeeds");

    for profile in [CertificateProfile::Reference, CertificateProfile::Extended] {
        assert_eq!(
            default_out.certificate.digest(profile).unwrap().as_str(),
            injected_out.certificate.digest(profile).unwrap().as_str(),
            "injecting the reference oracle moved the certificate bytes"
        );
    }
    assert_eq!(
        default_out.section.to_canonical_string().unwrap(),
        injected_out.section.to_canonical_string().unwrap(),
        "injecting the reference oracle moved the section bytes"
    );
}

struct AlwaysFires;

impl DecisionOracle for AlwaysFires {
    fn kind(&self) -> &str {
        "rule/test-always-fires-v1"
    }

    fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
        let mut observed = BTreeMap::new();
        observed.insert(
            "split_integrity_status".to_string(),
            values
                .get("split_integrity_status")
                .map(|v| v.to_string())
                .unwrap_or_else(|| "absent".to_string()),
        );
        Ok(OracleVerdict::new(
            self.kind(),
            vec![LeakageWitness::DomainCheck {
                check: "always_fires".into(),
                observed,
                detail: "test oracle that fires unconditionally".into(),
            }],
        ))
    }
}

#[test]
fn a_custom_oracle_verdict_reaches_the_certificate_and_names_its_kind() {
    let out = compile_with_oracle(&golden_world(), &leakage_query(), &AlwaysFires)
        .expect("custom compile succeeds");

    assert_eq!(out.certificate.oracle.oracle_kind, "rule/test-always-fires-v1");
    assert_eq!(out.certificate.oracle.status, OracleStatus::Invalid);
    assert_eq!(out.certificate.oracle.witness_kinds(), vec!["domain_check"]);
    assert_eq!(out.section.oracle, out.certificate.oracle);

    let receipt = out
        .trace
        .passes
        .iter()
        .find(|pass| pass.name == "oracle")
        .expect("the oracle pass leaves a receipt");
    assert_eq!(receipt.note, "status invalid");
}

struct RequiresUnprovidedVariable;

impl DecisionOracle for RequiresUnprovidedVariable {
    fn kind(&self) -> &str {
        "rule/test-requires-missing-input-v1"
    }

    fn evaluate(&self, values: &BTreeMap<String, Value>) -> Result<OracleVerdict, FiberError> {
        if values.contains_key("a_variable_no_reference_fact_provides") {
            Ok(OracleVerdict::new(self.kind(), Vec::new()))
        } else {
            Ok(OracleVerdict::abstain(self.kind(), Vec::new()))
        }
    }
}

#[test]
fn an_oracle_that_cannot_see_its_inputs_abstains_rather_than_validating() {
    let out = compile_with_oracle(&golden_world(), &leakage_query(), &RequiresUnprovidedVariable)
        .expect("abstaining compile succeeds");

    assert_eq!(out.certificate.oracle.status, OracleStatus::Underdetermined);
    assert!(out.certificate.oracle.witnesses.is_empty());

    let encoded = out
        .certificate
        .to_json(CertificateProfile::Extended)
        .expect("extended certificate serialises");
    assert_eq!(encoded["oracle"]["status"], Value::from("underdetermined"));
}
