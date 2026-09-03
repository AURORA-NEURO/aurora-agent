use bioprism_neurosurgery::{
    EvidenceGraphQuery, NeurosurgeryError, RealDataRecordKind, RealGliomaBundle,
    MAX_EVIDENCE_GRAPH_EDGES, MAX_EVIDENCE_GRAPH_NODES,
};

fn bundle() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real glioma snapshot parses")
}

#[test]
fn graph_projects_only_explicit_source_crosswalks_and_keeps_isolates_visible() {
    let report = bundle()
        .evidence_graph(&EvidenceGraphQuery {
            max_nodes: MAX_EVIDENCE_GRAPH_NODES,
            max_edges: MAX_EVIDENCE_GRAPH_EDGES,
            ..EvidenceGraphQuery::default()
        })
        .expect("validated graph compiles");
    assert_eq!(
        report.schema_version,
        "bioprism-neurosurgery-evidence-graph/0.1"
    );
    assert_eq!(report.bundle_digest.len(), 64);
    assert_eq!(report.graph_digest.len(), 64);
    assert_eq!(report.total_node_count, 88);
    assert_eq!(report.nodes.len(), report.total_node_count);
    assert_eq!(report.total_edge_count, 120);
    assert_eq!(report.edges.len(), report.total_edge_count);
    assert!(report.isolated_node_count > 0);
    assert!(report.connected_component_count > 1);
    assert_eq!(report.omitted_node_count, 0);
    assert_eq!(report.omitted_edge_count, 0);
    assert!(!report.truncated);
    assert!(report
        .edges
        .iter()
        .all(|edge| edge.from_record_id != edge.to_record_id));
    assert!(report
        .nodes
        .iter()
        .all(|node| node.source_uri.starts_with("https://")));
    assert!(report.human_review_required);
    assert_eq!(report.provider, "none");
    assert!(!report.network);
    report
        .validate_integrity()
        .expect("graph digest and topology are self-consistent");
    report
        .validate_for_inputs(&bundle())
        .expect("graph replays against the exact source snapshot");
    let mut tampered = report.clone();
    tampered.nodes[0].title.push_str(" (tampered)");
    assert!(tampered.validate_integrity().is_err());
}

#[test]
fn rooted_graph_traversal_reaches_linked_study_profiles_and_pubmed_metadata() {
    let report = bundle()
        .evidence_graph(&EvidenceGraphQuery {
            root_record_id: Some("24120142".to_string()),
            root_record_kind: Some(RealDataRecordKind::LiteratureArticle),
            max_nodes: 64,
            max_edges: 128,
        })
        .expect("rooted graph compiles");
    assert_eq!(report.root_count, 1);
    assert!(report.total_node_count >= 2);
    assert!(report.nodes.iter().any(|node| node.record_id == "24120142"));
    assert!(report.nodes.iter().any(|node| {
        node.record_kind == RealDataRecordKind::PortalStudy && node.record_id == "gbm_tcga_pub2013"
    }));
    assert!(report
        .nodes
        .iter()
        .any(|node| { node.record_kind == RealDataRecordKind::PortalMolecularProfile }));
    assert!(report.edges.iter().any(|edge| {
        edge.from_record_id == "24120142" && edge.to_record_id == "gbm_tcga_pub2013"
    }));
    assert!(report.edges.iter().all(|edge| {
        report.nodes.iter().any(|node| {
            node.record_kind == edge.from_record_kind && node.record_id == edge.from_record_id
        }) && report.nodes.iter().any(|node| {
            node.record_kind == edge.to_record_kind && node.record_id == edge.to_record_id
        })
    }));
}

#[test]
fn graph_bounds_and_root_errors_are_explicit() {
    let bounded = bundle()
        .evidence_graph(&EvidenceGraphQuery {
            max_nodes: 3,
            max_edges: 2,
            ..EvidenceGraphQuery::default()
        })
        .expect("bounded graph compiles");
    assert_eq!(bounded.nodes.len(), 3);
    assert!(bounded.omitted_node_count > 0 || bounded.omitted_edge_count > 0);
    assert!(bounded.truncated);
    let missing_root = bundle().evidence_graph(&EvidenceGraphQuery {
        root_record_id: Some("not-in-bundle".to_string()),
        ..EvidenceGraphQuery::default()
    });
    assert!(matches!(
        missing_root,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
    let invalid_kind = bundle().evidence_graph(&EvidenceGraphQuery {
        root_record_kind: Some(RealDataRecordKind::PortalStudy),
        ..EvidenceGraphQuery::default()
    });
    assert!(matches!(
        invalid_kind,
        Err(NeurosurgeryError::RealDataRejected { .. })
    ));
}
