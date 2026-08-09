//! End-to-end invariants of the repository adapter (blueprint 40.17 invariant 1, 28.00).
//!
//! The claim under test is narrow and absolute: the inventory adapter says what exists and
//! what its bytes hash to, and declares everything it did not interpret. It never gains a
//! format reader by accident — a `.nii` file must produce a loss entry saying the NIfTI reader
//! lives elsewhere, not a set of facts about a brain.

use bioprism_adapter::{
    conformance, Adapter, InventoryAdapter, InventoryProfile, LossKind, LossSeverity, SemanticLoss,
    Source, SourceProvenance,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A scratch directory unique to one test, removed when the guard drops.
struct Scratch {
    root: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root = std::env::temp_dir()
            .join("bioprism-adapter-tests")
            .join(format!("{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("scratch directory");
        Scratch { root }
    }

    fn write(&self, relative: &str, contents: &[u8]) -> &Scratch {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("scratch subdirectory");
        }
        std::fs::write(path, contents).expect("scratch file");
        self
    }

    fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn traced(id: &str, root: &Path) -> Source {
    Source::directory(id, root).with_provenance(SourceProvenance {
        accession: Some("SYN-0001".into()),
        version: Some("2026-01".into()),
        retrieved_at: Some("2026-02-03T00:00:00Z".into()),
    })
}

fn adapter() -> InventoryAdapter {
    InventoryAdapter::new(InventoryProfile::new("archive"))
}

#[test]
fn every_file_becomes_one_fact_carrying_its_path_length_and_digest() {
    let scratch = Scratch::new("digests");
    scratch
        .write("manifest.json", b"{}")
        .write("labels/split.csv", b"subject,split\nS1,train\n");

    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    assert_eq!(ingestion.fact_count(), 2);
    let value = &ingestion.facts()[0]["value"];
    assert!(value["sha256"].is_string());
    assert!(value["byte_length"].is_number());
    assert!(value["path"].is_string());
}

#[test]
fn every_unread_file_is_declared_content_uninterpreted() {
    let scratch = Scratch::new("uninterpreted");
    scratch.write("a.bin", b"\x00\x01").write("b.bin", b"\x02");

    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    let uninterpreted = ingestion
        .loss()
        .entries()
        .iter()
        .filter(|entry| entry.kind == LossKind::ContentUninterpreted)
        .count();
    assert_eq!(uninterpreted, 2, "one entry per file, with no exceptions");
}

#[test]
fn a_modality_file_names_the_reader_that_lives_in_the_python_layer() {
    let scratch = Scratch::new("modality");
    scratch.write("sub-01/anat/T1w.nii", b"not really a nifti");

    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    let entry = &ingestion.loss().entries()[0];
    assert_eq!(entry.kind, LossKind::ContentUninterpreted);
    assert!(entry.detail.contains("NIfTI"), "{}", entry.detail);
    assert!(entry.detail.contains("Python"), "{}", entry.detail);
}

#[test]
fn nested_paths_are_slash_separated_so_the_world_is_platform_independent() {
    let scratch = Scratch::new("separators");
    scratch.write("a/b/c.txt", b"x");

    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    assert_eq!(ingestion.facts()[0]["value"]["path"], Value::from("a/b/c.txt"));
    assert!(!ingestion.facts()[0]["value"]["path"]
        .as_str()
        .unwrap()
        .contains('\\'));
}

#[test]
fn the_walk_order_does_not_leak_into_the_digest() {
    let scratch = Scratch::new("order");
    scratch
        .write("z.txt", b"z")
        .write("a.txt", b"a")
        .write("m/n.txt", b"n");

    let first = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();
    let second = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    let paths: Vec<&str> = first
        .facts()
        .iter()
        .map(|fact| fact["value"]["path"].as_str().unwrap())
        .collect();
    let mut sorted = paths.clone();
    sorted.sort_unstable();
    assert_eq!(paths, sorted);
}

#[test]
fn an_empty_repository_with_declared_provenance_is_lossless_not_unaudited() {
    let scratch = Scratch::new("empty");
    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    assert_eq!(ingestion.fact_count(), 0);
    assert!(matches!(ingestion.loss(), SemanticLoss::Lossless { .. }));
    assert!(ingestion.loss().is_audited());
}

#[test]
fn a_repository_with_no_upstream_accession_declares_provenance_unavailable() {
    let scratch = Scratch::new("untraced");
    scratch.write("a.txt", b"a");

    let ingestion = adapter()
        .ingest(&Source::directory("archive", scratch.path()))
        .unwrap();

    assert!(ingestion
        .loss()
        .kinds()
        .contains(&LossKind::ProvenanceUnavailable));
}

#[test]
fn a_file_too_large_to_hash_is_named_but_declared_unaddressable() {
    let scratch = Scratch::new("limit");
    scratch.write("small.bin", b"ab").write("large.bin", b"abcdefghij");

    let adapter = InventoryAdapter::new(InventoryProfile::new("archive").with_max_digest_bytes(4));
    let ingestion = adapter.ingest(&traced("archive", scratch.path())).unwrap();

    let large = ingestion
        .facts()
        .iter()
        .find(|fact| fact["value"]["path"] == *"large.bin")
        .unwrap();
    assert!(large["value"]["sha256"].is_null());

    let entry = ingestion
        .loss()
        .entries()
        .iter()
        .find(|entry| entry.location.artifact.as_deref() == Some("large.bin"))
        .unwrap();
    assert_eq!(entry.severity, LossSeverity::Blocking);
}

#[test]
fn identical_bytes_under_different_names_produce_the_same_digest_in_their_facts() {
    let scratch = Scratch::new("same-bytes");
    scratch.write("one.txt", b"same").write("two.txt", b"same");

    let ingestion = adapter()
        .ingest(&traced("archive", scratch.path()))
        .unwrap();

    let digests: Vec<&Value> = ingestion
        .facts()
        .iter()
        .map(|fact| &fact["value"]["sha256"])
        .collect();
    assert_eq!(digests[0], digests[1]);
}

#[test]
fn the_inventory_adapter_is_certified_against_a_real_directory() {
    let scratch = Scratch::new("certify");
    scratch
        .write("manifest.json", b"{}")
        .write("imaging/scan.dcm", b"\x00DICM")
        .write("tables/clinical.csv", b"subject\nS1\n");

    let source = traced("archive", scratch.path());
    let (report, ingestion) = conformance::certify(&adapter(), &source).unwrap();

    assert!(report.verified(), "{}", report.summary());
    assert_eq!(ingestion.fact_count(), 3);
    assert_eq!(ingestion.loss().entries().len(), 3);
}

#[test]
fn a_byte_source_is_refused_by_the_inventory_adapter() {
    let error = adapter()
        .ingest(&Source::bytes("archive", b"not a directory".to_vec()))
        .unwrap_err();
    assert!(error.to_string().contains("directory"));
}
