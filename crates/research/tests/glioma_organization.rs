//! Source-level organization checks for the folder-owned glioma product.

use bioprism_research::{generate_feature_catalog, validate_feature_catalog};
use serde_json::Value;
use std::path::Path;

#[test]
fn organization_manifest_matches_runtime_catalog_and_all_program_folders_exist() {
    let manifest: Value =
        serde_json::from_str(include_str!("../../../docs/glioma/organization.json"))
            .expect("glioma organization manifest is valid JSON");
    assert_eq!(manifest["program_count"], 12);
    assert_eq!(manifest["features_per_program"], 32);
    assert_eq!(manifest["feature_count"], 384);
    let features = generate_feature_catalog();
    validate_feature_catalog(&features).expect("runtime catalog is valid");
    assert_eq!(
        features.len(),
        manifest["feature_count"].as_u64().unwrap() as usize
    );

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("glioma")
        .join("programs");
    for program in manifest["programs"].as_array().unwrap() {
        let folder = program["folder"].as_str().unwrap();
        assert!(
            source_root.join(folder).join("mod.rs").is_file(),
            "missing program folder: {folder}"
        );
    }
}

#[test]
fn organization_manifest_keeps_the_preclinical_boundary_explicit() {
    let manifest: Value =
        serde_json::from_str(include_str!("../../../docs/glioma/organization.json")).unwrap();
    let boundary = manifest["boundary"].as_str().unwrap();
    assert!(boundary.contains("preclinical-research-only"));
    assert!(boundary.contains("no diagnosis"));
    assert!(boundary.contains("no human-subject"));
}
