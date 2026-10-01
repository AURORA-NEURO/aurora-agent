//! MCP contract tests for research modeling contracts.

use super::*;

#[test]
fn bioatlas_publication_audit_binds_atlas_evidence_card_and_leaderboard_targets() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }

    let ontology = CapabilityOntology::from_nodes(
        "publication-atlas/1",
        [
            CapabilityNode::new(
                cap("agent"),
                "agent",
                CapabilityFamily::DomainReasoning,
                CapabilityDimension::Competence,
            ),
            CapabilityNode::new(
                cap("measured"),
                "measured",
                CapabilityFamily::Verification,
                CapabilityDimension::Reliability,
            )
            .with_parent(cap("agent")),
            CapabilityNode::new(
                cap("efficient"),
                "efficient",
                CapabilityFamily::ToolUse,
                CapabilityDimension::Efficiency,
            )
            .with_parent(cap("agent")),
        ],
    )
    .unwrap();
    let atlas = Atlas::builder(ontology)
        .evidence(EvidenceRecord::new(
            "agent-trial",
            cap("agent"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .evidence(EvidenceRecord::new(
            "measured-trial",
            cap("measured"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .evidence(EvidenceRecord::new(
            "efficient-trial",
            cap("efficient"),
            "publication-atlas/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .build()
        .unwrap();

    let review = call(
        &mut server(),
        "hub_submission_review",
        hub_review_fixture("sub-bioatlas-publication", b"bioatlas-publication"),
    );
    assert_eq!(review["ok"], json!(true));
    let pack = ContentHash::of_bytes(b"bioatlas-publication-pack");
    let disclosure = call(
        &mut server(),
        "hub_disclosure_review",
        json!({
            "actions": [
                { "kind": "declare_held_out", "pack": serde_json::to_value(&pack).unwrap() },
                { "kind": "disclose", "pack": serde_json::to_value(&pack).unwrap(), "at": 5 }
            ]
        }),
    );
    assert_eq!(disclosure["ok"], json!(true));

    let conditions = ComparabilityConditions {
        pack: pack.clone(),
        pack_version: "1.0.0".into(),
        split: "hidden-holdout".into(),
        metric: "atlas-publication-score".into(),
        higher_is_better: true,
        oracle_tier: "deterministic".into(),
        access_mode: AccessTier::Public,
        budget: BudgetEnvelope::unbounded(),
        protocol: ContentHash::of_bytes(b"bioatlas-publication-protocol"),
    };
    let board = Board {
        id: BoardId::parse("bioatlas-publication").unwrap(),
        conditions: conditions.clone(),
        min_verification: VerificationStatus::SelfReported,
    };
    let entry = Entry {
        submission: SubmissionId::parse("sub-bioatlas-publication").unwrap(),
        conditions,
        score: HubScore::point(0.91),
        computed_at: HubEpoch(5),
        acknowledges_disclosure: true,
        scale: EvidenceScale::new(10, 5),
    };

    let evidence_audit = json!({
        "vectors": [
            metric_vector("system-a", 0.91, 0.88, "pack/4"),
            metric_vector("system-b", 0.71, 0.69, "pack/4")
        ],
        "evidence": [{
            "id": "grounding-1",
            "dimension": "evidence_grounding",
            "status": "observed",
            "source": "publication-fixture",
            "scope": "public-atlas-card"
        }],
        "claim_requests": [{
            "id": "grounded-card",
            "claim": "the public atlas card is grounded in the supplied source",
            "requires": ["evidence_grounding"]
        }]
    });
    let atlas_json = serde_json::to_value(atlas).unwrap();
    let result = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({
            "atlas": atlas_json.clone(),
            "evidence_audit": evidence_audit,
            "card": {
                "moderation": review["ledger"].clone(),
                "submission": "sub-bioatlas-publication",
                "score": serde_json::to_value(HubScore::point(0.91)).unwrap(),
                "pack": serde_json::to_value(&pack).unwrap(),
                "computed_at": 5,
                "acknowledges_disclosure": true,
                "disclosure": disclosure["ledger"].clone()
            },
            "leaderboard": {
                "board": serde_json::to_value(board).unwrap(),
                "entries": [serde_json::to_value(entry).unwrap()],
                "moderation": review["ledger"].clone(),
                "disclosure": disclosure["ledger"].clone()
            },
            "release_request": {
                "id": "publication-fixture-release",
                "targets": [
                    "atlas_profile",
                    "atlas_aggregation",
                    "evidence_claims",
                    "card_render",
                    "numeric_card_score",
                    "ranked_leaderboard"
                ]
            }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/bioatlas-publication-audit/0.1")
    );
    assert_eq!(result["workflow"], json!("bioatlas_publication_audit"));
    assert_eq!(result["release_request"]["ready"], json!(true));
    assert_eq!(
        result["cross_layer"]["numeric_score_evidence_ready"],
        json!(true)
    );
    assert_eq!(
        result["cross_layer"]["leaderboard_unranked_count"],
        json!(0)
    );
    assert_eq!(result["card"]["score"]["attached"], json!(true));

    let no_request = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({ "atlas": atlas_json.clone() }),
    );
    assert_eq!(no_request["ok"], json!(true));
    assert_eq!(no_request["release_request"]["present"], json!(false));
    assert_eq!(no_request["release_request"]["ready"], json!(false));

    let numeric_without_evidence = call(
        &mut server(),
        "bioatlas_publication_audit",
        json!({
            "atlas": atlas_json,
            "card": {
                "moderation": review["ledger"].clone(),
                "submission": "sub-bioatlas-publication",
                "score": serde_json::to_value(HubScore::point(0.91)).unwrap(),
                "pack": serde_json::to_value(&pack).unwrap(),
                "computed_at": 5,
                "acknowledges_disclosure": true,
                "disclosure": disclosure["ledger"].clone()
            },
            "release_request": {
                "id": "numeric-without-evidence",
                "targets": ["numeric_card_score"]
            }
        }),
    );
    assert_eq!(
        numeric_without_evidence["release_request"]["ready"],
        json!(false)
    );
    assert!(
        numeric_without_evidence["release_request"]["targets"][0]["blockers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|blocker| blocker == "evidence_audit_missing")
    );
}

#[test]
fn bioworlds_catalog_runs_reference_slices_and_keeps_unbuilt_worlds_explicit() {
    let mut server = server();
    let payload = call(&mut server, "bioworlds_catalog", json!({}));
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["mode"], json!("catalog"));
    assert_eq!(payload["slice_count"], json!(4));
    assert!(payload["report"]["digest"].is_string());
    assert!(payload["report"]["slices"].as_array().unwrap().len() >= 4);
    assert!(payload["unbuilt_worlds"].as_array().unwrap().len() >= 10);
}

#[test]
fn modality_catalog_exposes_resolution_and_failure_mode_boundaries() {
    let mut server = server();
    let payload = call(
        &mut server,
        "modality_catalog",
        json!({ "modality": "single-cell and multiome", "include_failure_modes": true }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["returned"], json!(1));
    assert_eq!(payload["modalities"][0]["blueprint_module"], json!("28.04"));
    assert!(payload["modalities"][0]["resolutions"].is_array());
    assert!(payload["modalities"][0]["failure_modes"].is_array());
    assert!(payload["unmechanised_failure_modes"].is_number());
}

#[test]
fn modality_support_check_separates_claim_eligibility_from_analysis_unit() {
    let refused = call(
        &mut server(),
        "modality_support_check",
        json!({
            "modality": "bulk_transcriptomics",
            "claim": "cell_intrinsic_change",
            "counted_unit": "population"
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(
        refused["support"]["root_refusal_kind"],
        json!("missing_resolution")
    );
    assert_eq!(refused["descriptor"]["complete"], json!(true));
    assert_eq!(refused["analysis_unit"]["admissible"], json!(false));
    assert_eq!(
        refused["analysis_unit"]["refusal_kind"],
        json!("named_failure_mode")
    );

    let admitted = call(
        &mut server(),
        "modality_support_check",
        json!({
            "modality": "single_cell",
            "claim": "cell_composition",
            "counted_unit": "subject"
        }),
    );
    assert_eq!(admitted["outcome_kind"], json!("supported"));
    assert_eq!(admitted["supported"], json!(true));
    assert_eq!(admitted["analysis_unit"]["admissible"], json!(true));
    assert!(admitted["claim_requirements"]["axes"].is_array());
    assert!(admitted["descriptor"]["supported_catalogue_claims"].is_array());
}

#[test]
fn modality_transport_check_preserves_loss_fidelity_and_support_changes() {
    let aggregated = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "single_cell",
            "to": "bulk_transcriptomics",
            "axis": "cell",
            "transport": {"kind": "aggregation", "operator": "mean"},
            "claims": ["cell_intrinsic_change", "cell_composition"]
        }),
    );
    assert_eq!(aggregated["ok"], json!(true));
    assert_eq!(aggregated["outcome_kind"], json!("constructed"));
    assert_eq!(aggregated["constructed"], json!(true));
    assert_eq!(aggregated["fidelity"]["fidelity"], json!("exact"));
    assert_eq!(aggregated["inverse"]["invertible"], json!(false));
    assert_eq!(aggregated["scope_mapping_check"], json!("sound"));
    assert!(aggregated["loss"]["discarded"].as_array().unwrap().len() >= 2);
    assert_eq!(aggregated["application"]["applied"], json!(true));
    assert_eq!(aggregated["claims"][0]["support_lost"], json!(true));

    let deconvolved = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "bulk_transcriptomics",
            "to": "single_cell",
            "axis": "cell",
            "transport": {"kind": "deconvolution", "reference": "signature-matrix-v1", "recomposition": "sum"},
            "claims": ["cell_composition", "cell_intrinsic_change"]
        }),
    );
    assert_eq!(deconvolved["fidelity"]["fidelity"], json!("estimated"));
    assert_eq!(deconvolved["inverse"]["invertible"], json!(true));
    assert_eq!(deconvolved["claims"][0]["after"]["supported"], json!(true));
    assert_eq!(deconvolved["claims"][1]["after"]["supported"], json!(false));

    let refused = call(
        &mut server(),
        "modality_transport_check",
        json!({
            "from": "bulk_transcriptomics",
            "to": "single_cell",
            "axis": "cell",
            "transport": {"kind": "deconvolution", "reference": "", "recomposition": "sum"}
        }),
    );
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["constructed"], json!(false));
    assert_eq!(
        refused["transport_evidence"]["refusal_kind"],
        json!("unstated_basis")
    );
}

#[test]
fn modality_comparability_check_blocks_category_errors_before_standards() {
    let term =
        TermBinding::exact("TP53", OntologyId::parse("HGNC:11998", "2026-01").unwrap()).unwrap();
    let rna = ModalMeasurement::new(
        bioprism_modalities::descriptor(Modality::BulkTranscriptomics),
        Resolution::Population,
        Measurement::scalar("RNA abundance", Quantity::parse(1.0, "1").unwrap()).of(term.clone()),
    );
    let protein = ModalMeasurement::new(
        bioprism_modalities::descriptor(Modality::Proteomics),
        Resolution::Population,
        Measurement::scalar("protein abundance", Quantity::parse(1.0, "1").unwrap()).of(term),
    );
    let blocked = call(
        &mut server(),
        "modality_comparability_check",
        json!({
            "left": serde_json::to_value(&rna).unwrap(),
            "right": serde_json::to_value(&protein).unwrap()
        }),
    );
    assert_eq!(blocked["ok"], json!(true));
    assert_eq!(blocked["outcome_kind"], json!("blocked"));
    assert_eq!(blocked["comparable"], json!(false));
    assert_eq!(
        blocked["report"]["verdict"]["reason"]["blocked_by"],
        json!("measurand_mismatch")
    );
    assert_eq!(blocked["report"]["standards"], Value::Null);
    assert_eq!(blocked["report_sha256"].as_str().unwrap().len(), 64);

    let comparable = call(
        &mut server(),
        "modality_comparability_check",
        json!({
            "left": serde_json::to_value(&rna).unwrap(),
            "right": serde_json::to_value(&rna).unwrap(),
            "policy": {"require_bound_terms": true}
        }),
    );
    assert_eq!(comparable["outcome_kind"], json!("comparable"));
    assert!(comparable["report"]["standards"].is_object());
}

#[test]
fn literature_bind_check_separates_source_binding_from_citation_support() {
    let published = Timestamp::parse("2026-01-01T00:00:00Z").unwrap();
    let population = ScopeKey::new().exact("disease", "diffuse_glioma");
    let claim = LiteratureClaim::new(
        "the source reports an observed cohort result",
        SourceProvenance::new(
            "doi:10.1000/example",
            LiteratureEvidenceTier::Primary,
            published,
        )
        .studying(population),
    );
    let bound = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&claim).unwrap(),
            "target": serde_json::to_value(ScopeKey::new().exact("disease", "diffuse_glioma").exact("site", "site-a")).unwrap(),
            "at_tier": "primary",
            "horizon": serde_json::to_value(EvaluationHorizon::open()).unwrap(),
            "claim_kind": serde_json::to_value(ModalityClaimKind::PublishedClaimSupport).unwrap()
        }),
    );
    assert_eq!(bound["ok"], json!(true));
    assert_eq!(bound["outcome_kind"], json!("citable"));
    assert_eq!(bound["bound"], json!(true));
    assert_eq!(bound["citable"], json!(true));
    assert_eq!(bound["evidence"]["citation"]["cited_as"], json!("primary"));
    assert_eq!(
        bound["evidence"]["citation"]["direct_evidence"],
        json!(true)
    );

    let review = LiteratureClaim::new(
        "a review summary",
        SourceProvenance::new(
            "doi:10.1000/review",
            LiteratureEvidenceTier::Review,
            published,
        )
        .studying(ScopeKey::new().exact("disease", "diffuse_glioma")),
    );
    let laundered = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&review).unwrap(),
            "target": { "disease": "diffuse_glioma" },
            "at_tier": "primary",
            "horizon": { "horizon": "open" }
        }),
    );
    assert_eq!(laundered["outcome_kind"], json!("refused"));
    assert_eq!(laundered["bound"], json!(false));
    assert_eq!(
        laundered["evidence"]["refusal_kind"],
        json!("citation_laundering")
    );

    let flagged = LiteratureClaim::new(
        "a flagged source",
        SourceProvenance::new(
            "doi:10.1000/flagged",
            LiteratureEvidenceTier::Primary,
            published,
        )
        .flagged(RetractionStatus::Retracted)
        .studying(ScopeKey::new().exact("disease", "diffuse_glioma")),
    );
    let warrant = call(
        &mut server(),
        "literature_bind_check",
        json!({
            "claim": serde_json::to_value(&flagged).unwrap(),
            "target": { "disease": "diffuse_glioma" },
            "at_tier": "review",
            "horizon": { "horizon": "open" },
            "flag_warrant": "citing the retraction history explicitly"
        }),
    );
    assert_eq!(warrant["outcome_kind"], json!("bound"));
    assert_eq!(warrant["bound"], json!(true));
    assert_eq!(warrant["evidence"]["flag_warrant_supplied"], json!(true));
}

#[test]
fn mutation_family_reports_validated_diversity_without_dumping_worlds_by_default() {
    let mut server = server();
    let payload = call(
        &mut server,
        "mutation_family",
        json!({ "world": "fixtures/generated/discriminating_world.json" }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["counts"]["attempted"], json!(8));
    assert!(payload["accepted"].is_array());
    assert!(payload["rejected"].is_array());
    assert!(payload["diversity"]["equivalence_classes"].is_number());
    assert!(payload.get("worlds").is_none());
}

#[test]
fn measurement_compare_records_conversion_and_blocks_dimension_or_binding_mismatch() {
    let mut server = server();
    let left = serde_json::to_value(Measurement::scalar(
        "left",
        Quantity::parse(10.0, "mm").unwrap(),
    ))
    .unwrap();
    let right = serde_json::to_value(Measurement::scalar(
        "right",
        Quantity::parse(1.0, "cm").unwrap(),
    ))
    .unwrap();
    let converted = call(
        &mut server,
        "measurement_compare",
        json!({ "left": left, "right": right }),
    );
    assert_eq!(converted["ok"], json!(true));
    assert_eq!(converted["comparable"], json!(true));
    assert_eq!(
        converted["report"]["verdict"]["verdict"],
        json!("comparable")
    );
    assert_eq!(
        converted["report"]["conversions"].as_array().unwrap().len(),
        1
    );
    assert_eq!(converted["report"]["caveats"].as_array().unwrap().len(), 1);
    assert_eq!(converted["report_sha256"].as_str().unwrap().len(), 64);

    let blocked = call(
        &mut server,
        "measurement_compare",
        json!({
            "left": serde_json::to_value(Measurement::scalar(
                "length",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "right": serde_json::to_value(Measurement::scalar(
                "volume",
                Quantity::parse(1.0, "mL").unwrap()
            )).unwrap()
        }),
    );
    assert_eq!(blocked["comparable"], json!(false));
    assert_eq!(blocked["report"]["verdict"]["verdict"], json!("blocked"));
    assert_eq!(
        blocked["report"]["verdict"]["reason"]["blocked_by"],
        json!("dimension_mismatch")
    );

    let unbound = call(
        &mut server,
        "measurement_compare",
        json!({
            "left": serde_json::to_value(Measurement::scalar(
                "left",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "right": serde_json::to_value(Measurement::scalar(
                "right",
                Quantity::parse(1.0, "mm").unwrap()
            )).unwrap(),
            "require_bound_terms": true
        }),
    );
    assert_eq!(unbound["comparable"], json!(false));
    assert_eq!(unbound["report"]["verdict"]["verdict"], json!("blocked"));
}

#[test]
fn tabular_ingest_runs_independent_conformance_and_keeps_loss_visible() {
    let mut server = server();
    let profile = TabularProfile::new("RG-DEMO-001")
        .scope("subject", "subject")
        .variable(
            "age",
            VariableMapping::new("age_at_diagnosis").typed(ValueType::Integer),
        );
    let payload = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "cohort.csv",
            "format": "text/csv",
            "csv": "subject,age,comment\nS1,41,ok\n",
            "profile": serde_json::to_value(profile).unwrap(),
            "provenance": { "accession": "RG-DEMO-001", "version": "v1", "retrieved_at": "2026-08-14T00:00:00Z" },
            "include_facts": true,
            "max_items": 2
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["fact_count"], json!(1));
    assert_eq!(payload["facts"].as_array().unwrap().len(), 1);
    assert_eq!(payload["conformance"]["verified"], json!(true));
    assert!(payload["semantic_loss"].is_object());
    assert_eq!(payload["manifest"]["source_id"], json!("cohort.csv"));
    assert_eq!(
        payload["manifest"]["provenance"]["accession"],
        json!("RG-DEMO-001")
    );

    let refused = call(
        &mut server,
        "tabular_ingest",
        json!({
            "source_id": "cohort.csv",
            "csv": "subject,age\nS1,41\n",
            "document": WORLD,
            "profile": serde_json::to_value(TabularProfile::new("RG-DEMO-001")).unwrap()
        }),
    );
    assert_eq!(refused["__isError"], json!(true));
    assert!(
        refused["error"]
            .as_str()
            .unwrap()
            .contains("either csv or document")
    );
}

#[test]
fn observed_world_and_claim_tools_enforce_pinning_rungs_and_selection() {
    let mut server = server();
    let source = SourceRef::new("cohort", "v1").under(Access::Controlled {
        policy: "reviewer-only".into(),
    });
    let design = StudyDesign::new(
        2,
        Selection::Consecutive {
            criterion: "all eligible participants".into(),
        },
    )
    .with_stratum(Stratum::new("all", 2))
    .standing_for("RG-DEMO population");
    let declared = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "observed-demo",
            "sources": [serde_json::to_value(source).unwrap()],
            "design": serde_json::to_value(design).unwrap(),
            "outcome_labels": ["positive", "negative"]
        }),
    );
    assert_eq!(declared["ok"], json!(true));
    assert_eq!(declared["world_id"], json!("observed-demo"));
    assert_eq!(declared["controlled_sources"], json!(["cohort"]));
    assert_eq!(declared["provenance"]["top"], json!("observed"));

    let unpinned = call(
        &mut server,
        "observed_world_declare",
        json!({
            "id": "bad-world",
            "sources": [serde_json::to_value(SourceRef::unpinned("cohort")).unwrap()],
            "design": serde_json::to_value(StudyDesign::new(
                1,
                Selection::Consecutive { criterion: "eligible".into() }
            )).unwrap(),
            "outcome_labels": ["positive"]
        }),
    );
    assert_eq!(unpinned["__isError"], json!(true));
    assert!(unpinned["error"].as_str().unwrap().contains("pinned"));

    let observed_provenance = Provenance::observed(Selection::Consecutive {
        criterion: "all eligible participants".into(),
    });
    let supported = call(
        &mut server,
        "world_claim_check",
        json!({
            "provenance": serde_json::to_value(&observed_provenance).unwrap(),
            "claim": serde_json::to_value(Claim::new(ClaimKind::Biology, "observed outcome")).unwrap()
        }),
    );
    assert_eq!(supported["ok"], json!(true));
    assert_eq!(supported["supported"], json!(true));
    assert!(
        supported["caveat"]
            .as_str()
            .unwrap()
            .contains("observed world")
    );

    let simulated = Provenance::mechanistic(["tumour growth rate"]);
    let refused = call(
        &mut server,
        "world_claim_check",
        json!({
            "provenance": serde_json::to_value(&simulated).unwrap(),
            "claim": serde_json::to_value(Claim::new(ClaimKind::Biology, "tumour growth rate")).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["supported"], json!(false));
    assert!(
        refused["refusal"]
            .as_str()
            .unwrap()
            .contains("construction")
    );
}

#[test]
fn lineage_audit_separates_identity_gaps_from_material_and_ancestry_findings() {
    let mut server = server();
    let mut artifact = Artifact::new("slide-1", "s1", "pathology");
    artifact.observed_digest = Some("stale-digest".into());
    let registry = SpecimenRegistry::new()
        .with_specimen(
            SpecimenNode::new("s1", "donor-a", 10)
                .with_content("marker", json!("same-material"))
                .fingerprinted("donor-b"),
        )
        .with_specimen(
            SpecimenNode::new("s2", "donor-a", 10).with_content("marker", json!("same-material")),
        )
        .with_specimen(SpecimenNode::new("s3", "donor-a", 11).derived_from("s1"))
        .with_artifact(artifact);

    let payload = call(
        &mut server,
        "lineage_audit",
        json!({ "registry": serde_json::to_value(&registry).unwrap(), "max_items": 20 }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["clean"], json!(false));
    assert_eq!(payload["identity_complete"], json!(false));
    assert!(payload["finding_count"].as_u64().unwrap() >= 3);
    assert_eq!(payload["unchecked_identity_count"], json!(2));
    assert!(
        payload["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["finding"] == json!("mass_not_conserved"))
    );
    assert!(
        payload["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|finding| finding["finding"] == json!("identity_mismatch"))
    );
}

#[test]
fn preanalytic_apply_preserves_biology_and_reports_false_positive_and_response_contracts() {
    let mut server = server();
    let specimen = Specimen::new("sp-1")
        .with_biology("state", json!("stable"))
        .with_qc("drift", 0)
        .with_measurability("rna", 10_000);
    let mutation = PreanalyticMutation::new(
        "cold-30",
        "cold-family",
        FaultKind::ColdIschaemia { minutes: 30 },
        Intensity::FULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "drift".into(),
        delta: 5,
    })
    .editing(Edit::Handling {
        stage: bioprism_worldfactory::preanalytic::Stage::Collection,
        field: "minutes".into(),
        value: json!(30),
    });
    let null = PreanalyticMutation::new(
        "cold-null",
        "cold-family",
        FaultKind::ColdIschaemia { minutes: 30 },
        Intensity::NULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "drift".into(),
        delta: 5,
    });

    let payload = call(
        &mut server,
        "preanalytic_apply",
        json!({
            "specimen": serde_json::to_value(&specimen).unwrap(),
            "mutation": serde_json::to_value(&mutation).unwrap(),
            "family": [serde_json::to_value(&null).unwrap(), serde_json::to_value(&mutation).unwrap()],
            "available_actions": ["detect"],
            "qc_field": "drift",
            "alert_at": 3
        }),
    );
    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["applied"], json!(true));
    assert_eq!(payload["biology_unchanged"], json!(true));
    assert_eq!(payload["has_signature"], json!(true));
    assert_eq!(payload["response_check"]["ok"], json!(true));
    assert_eq!(payload["family_validation"]["ok"], json!(true));
    assert_eq!(payload["detectability"]["intensity"], json!(10_000));

    let damaging = mutation.editing(Edit::Biology {
        field: "state".into(),
        value: json!("changed"),
    });
    let refused = call(
        &mut server,
        "preanalytic_apply",
        json!({
            "specimen": serde_json::to_value(&specimen).unwrap(),
            "mutation": serde_json::to_value(&damaging).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["applied"], json!(false));
    assert!(refused["refusal"].as_str().unwrap().contains("biological"));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn contradiction_review_keeps_hypotheses_and_resolution_states_explicit() {
    let mut server = server();
    let left = Reading::new(
        "imaging",
        "marker",
        Lens::new("mri", "macroscopic").over(SpatialExtent::Whole),
        ScopeKey::new().exact("specimen", "S1").exact("time", "T1"),
        Reported::Value(ReadingValue::interval(50, 60)),
    );
    let right = Reading::new(
        "pathology",
        "marker",
        Lens::new("slide", "sampled").over(SpatialExtent::Sampled {
            region: "core".into(),
        }),
        ScopeKey::new().exact("specimen", "S1").exact("time", "T1"),
        Reported::Value(ReadingValue::interval(0, 10)),
    );
    let hypotheses = vec![
        Hypothesis::new(
            "h-sampling",
            Discordance::SpatialSampling {
                modality: ModalityId::new("pathology"),
            },
        ),
        Hypothesis::new("h-assay", Discordance::AssayScope),
        Hypothesis::new("h-time", Discordance::DifferentTime),
    ];
    let actions = vec![
        DiscriminatingAction::new("pathology-review", 1)
            .refuting("h-sampling")
            .refuting("h-time"),
        DiscriminatingAction::new("assay-review", 2).refuting("h-assay"),
    ];
    let args = json!({
        "left": serde_json::to_value(&left).unwrap(),
        "right": serde_json::to_value(&right).unwrap(),
        "intent": serde_json::to_value(DiscordanceClass::Resolvable).unwrap(),
        "hypotheses": serde_json::to_value(&hypotheses).unwrap(),
        "actions": serde_json::to_value(&actions).unwrap(),
        "references": [serde_json::to_value(ReferenceDiscordance::new("imaging", "pathology", 100, 10)).unwrap()],
        "notable_below_per_ten_thousand": 2_000
    });
    let pending = call(&mut server, "contradiction_review", args.clone());
    assert_eq!(pending["ok"], json!(true));
    assert_eq!(pending["validated"], json!(true));
    assert_eq!(pending["declared_hypothesis_count"], json!(3));
    assert_eq!(pending["admissible_hypothesis_count"], json!(2));
    assert_eq!(pending["state_name"], json!("not_yet_examined"));
    assert_eq!(
        pending["next_actions"][0]["evidence"],
        json!("pathology-review")
    );
    assert_eq!(pending["expectedness"]["ok"], json!(true));
    assert_eq!(
        pending["expectedness"]["value"]["expectedness"],
        json!("notable")
    );

    let narrowed = call(
        &mut server,
        "contradiction_review",
        json!({
            "left": args["left"].clone(),
            "right": args["right"].clone(),
            "intent": args["intent"].clone(),
            "hypotheses": args["hypotheses"].clone(),
            "actions": args["actions"].clone(),
            "examine": ["pathology-review"]
        }),
    );
    assert_eq!(narrowed["ok"], json!(true));
    assert_eq!(narrowed["state_name"], json!("not_yet_examined"));
    assert_eq!(narrowed["live_hypothesis_count"], json!(1));

    let over_narrowed = call(
        &mut server,
        "contradiction_review",
        json!({
            "left": args["left"].clone(),
            "right": args["right"].clone(),
            "intent": args["intent"].clone(),
            "hypotheses": args["hypotheses"].clone(),
            "actions": args["actions"].clone(),
            "examine": ["pathology-review", "assay-review"]
        }),
    );
    assert_eq!(over_narrowed["ok"], json!(false));
    assert_eq!(over_narrowed["fail_closed"], json!(true));
    assert!(
        over_narrowed["refusal"]
            .as_str()
            .unwrap()
            .contains("every remaining hypothesis")
    );
}

#[test]
fn domain_validate_returns_the_declared_checks_of_a_pack() {
    let mut server = server();
    let payload = call(
        &mut server,
        "domain_validate",
        json!({ "domain": TRADE_DOMAIN }),
    );

    assert_eq!(payload["ok"], json!(true));
    assert_eq!(payload["name"], json!("trade-surveillance"));
    assert_eq!(payload["oracle_kind"], json!("rule/trade-surveillance-v1"));
    assert_eq!(
        payload["required_variables"],
        json!(["self_match_conflicts", "reporting_window_closed"])
    );
    let checks = payload["checks"].as_array().unwrap();
    assert_eq!(
        checks
            .iter()
            .map(|check| check["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["self_cross", "late_reporting", "cancel_ratio_excessive"]
    );
    assert!(
        checks
            .iter()
            .all(|check| !check["description"].as_str().unwrap().is_empty())
    );
    assert_eq!(
        payload["protected_tags"],
        json!(["identity", "time", "protected"])
    );
    assert_eq!(payload["scope_dimensions_declared"], json!(true));
}

#[test]
fn atlas_report_preserves_holes_and_gates_composites() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }
    let ontology = CapabilityOntology::from_nodes(
        "atlas-test/1",
        [
            CapabilityNode::new(
                cap("agent"),
                "agent",
                CapabilityFamily::DomainReasoning,
                CapabilityDimension::Competence,
            ),
            CapabilityNode::new(
                cap("measured"),
                "measured",
                CapabilityFamily::Verification,
                CapabilityDimension::Reliability,
            )
            .with_parent(cap("agent")),
            CapabilityNode::new(
                cap("unmeasured"),
                "unmeasured",
                CapabilityFamily::ToolUse,
                CapabilityDimension::Efficiency,
            )
            .with_parent(cap("agent")),
        ],
    )
    .unwrap();
    let atlas = Atlas::builder(ontology)
        .evidence(EvidenceRecord::new(
            "trial-1",
            cap("measured"),
            "atlas-test/1",
            EvidenceTier::PublicObservedWorld,
            OracleTier::Deterministic,
            TrialOutcome::Pass,
        ))
        .build()
        .unwrap();
    let weighting = WeightingPolicy::declare(
        "test composite",
        [(cap("measured"), 1.0), (cap("unmeasured"), 1.0)],
    )
    .unwrap();
    let result = call(
        &mut server(),
        "atlas_report",
        json!({
            "atlas": serde_json::to_value(atlas).unwrap(),
            "weighting": serde_json::to_value(weighting).unwrap(),
            "max_items": 10
        }),
    );

    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["schema"], json!("bioprism-mcp/atlas-report/0.1"));
    assert_eq!(result["summary"]["measured"], json!(1));
    assert_eq!(result["summary"]["holes"], json!(2));
    assert_eq!(result["composite"]["ok"], json!(false));
    assert!(
        result["composite"]["refusal"]
            .as_str()
            .unwrap()
            .contains("unmeasured")
    );
}

#[test]
fn atlas_surface_audit_preserves_debt_browse_visibility_and_rate_denominators() {
    fn cap(id: &str) -> CapabilityId {
        CapabilityId::parse(id).unwrap()
    }
    fn conditions(label: &str) -> MeasurementConditions {
        MeasurementConditions::new(MetricsSubject::grid(label), ScoringRule::atlas_pass_rate())
    }
    fn measured(value: f64, effective_size: usize) -> GridCell {
        GridCell::point(
            value,
            NoIntervalReason::EstimatorNotAvailable,
            effective_size,
        )
        .unwrap()
    }
    fn record(id: &str, inducement: Inducement) -> FailureRecord {
        let chain = CausalChain::new(
            id,
            FailureLabel::new(FailureMechanism::RelevantEvidenceNotAcquired, 1),
            FailureLabel::new(FailureMechanism::StaleEvidenceTrusted, 2),
            vec![FailureLabel::new(
                FailureMechanism::UncertaintyMisreportedToCaller,
                3,
            )],
            FailureLabel::new(FailureMechanism::SuccessfulCommandMistakenForTaskSuccess, 4),
        )
        .unwrap();
        FailureRecord::new(
            id,
            RunId::parse(format!("run-{id}")).unwrap(),
            cap("identity.lineage"),
            "atlasx-test/1",
            chain,
            FailureAxes::new(
                EvidenceStatus::Preserved,
                Reversibility::Reversible,
                Detectability::DetectedByReview,
                Severity::Degraded,
                inducement,
            ),
            LabelDistribution::certain(
                FailureMechanism::StaleEvidenceTrusted,
                "protocol fixture diagnosis",
            ),
        )
    }

    let grid = CapabilityGrid::new("surface-system", conditions("surface-system"))
        .with_cell(cap("identity.lineage"), measured(0.8, 4))
        .with_cell(
            cap("causal.interpretation"),
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        )
        .with_cell(
            cap("cohort.statistics"),
            GridCell::unmeasured(UnmeasuredReason::NotAttempted),
        );
    let later_grid = CapabilityGrid::new("surface-system", conditions("surface-system"))
        .with_cell(cap("identity.lineage"), measured(0.9, 5))
        .with_cell(
            cap("causal.interpretation"),
            GridCell::unmeasured(UnmeasuredReason::OutOfScopeByDeclaredUse),
        )
        .with_cell(cap("cohort.statistics"), measured(0.7, 6));
    let result = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(grid).unwrap(),
            "later_grid": serde_json::to_value(later_grid).unwrap(),
            "failures": [
                serde_json::to_value(record("f-visible", Inducement::ModelInduced)).unwrap(),
                serde_json::to_value(record("f-withheld", Inducement::EvaluatorInduced)).unwrap()
            ],
            "facet": "mechanism",
            "visibility": [{ "failure_id": "f-withheld", "state": "under-review" }],
            "rate_capabilities": ["identity.lineage"],
            "max_items": 10
        }),
    );
    assert_eq!(result["__isError"], json!(false));
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/atlas-surface-audit/0.1")
    );
    assert_eq!(result["coverage"]["measured"], json!(1));
    assert_eq!(result["coverage"]["unmeasured"], json!(2));
    assert_eq!(
        result["debt_discharge"]["measured"]["rows"],
        json!(["cohort.statistics"])
    );
    assert_eq!(
        result["debt_discharge"]["declared_away"]["rows"],
        json!(["causal.interpretation"])
    );
    assert_eq!(result["failure_browse"]["records_browsed"], json!(2));
    assert_eq!(result["failure_browse"]["visible"], json!(1));
    assert_eq!(result["failure_browse"]["withheld"], json!(1));
    assert_eq!(result["surface_audits"]["sound"], json!(true));
    assert_eq!(
        result["rate_checks"]["rows"][0]["answer"]["outcome"],
        json!("answered")
    );
    assert_eq!(
        result["rate_checks"]["rows"][0]["answer"]["cell"]["kind"],
        json!("score")
    );
    assert!(
        (result["rate_checks"]["rows"][0]["answer"]["cell"]["value"]
            .as_f64()
            .unwrap()
            - 0.25)
            .abs()
            < 1e-9
    );

    let no_holes = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(CapabilityGrid::new(
                "surface-policy",
                conditions("surface-policy")
            ).with_cell(
                cap("causal.interpretation"),
                GridCell::unmeasured(UnmeasuredReason::NotAttempted)
            )).unwrap(),
            "require_no_holes": true
        }),
    );
    assert_eq!(no_holes["__isError"], json!(false));
    assert_eq!(no_holes["ok"], json!(false));
    assert_eq!(no_holes["stage"], json!("coverage_policy"));
    assert_eq!(no_holes["fail_closed"], json!(true));

    let mismatched = call(
        &mut server(),
        "atlas_surface_audit",
        json!({
            "grid": serde_json::to_value(CapabilityGrid::new(
                "surface-left",
                conditions("surface-left")
            )).unwrap(),
            "later_grid": serde_json::to_value(CapabilityGrid::new(
                "surface-right",
                conditions("surface-right")
            )).unwrap()
        }),
    );
    assert_eq!(mismatched["__isError"], json!(false));
    assert_eq!(mismatched["ok"], json!(false));
    assert_eq!(mismatched["stage"], json!("debt_discharge"));
}

#[test]
fn onco_boundary_check_releases_aggregate_work_and_escalates_individual_use() {
    let request = BoundaryRequest {
        purpose: "compare cohort response rates".into(),
        context: RequestContext::Research,
        claimed_role: "attending physician".into(),
        claimed_urgency: true,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: vec![
            OutputUse::CohortAnalysis,
            OutputUse::TreatmentRecommendation,
        ],
        direct_identifier_fields: Vec::new(),
    };
    let result = call(
        &mut server(),
        "onco_boundary_check",
        json!({ "request": serde_json::to_value(request).unwrap() }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-boundary-check/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("disposition"));
    assert_eq!(result["disposition_kind"], json!("release_partial"));
    assert_eq!(result["released"][0], json!("cohort_analysis"));
    assert_eq!(result["refused"][0], json!("treatment_recommendation"));
    assert_eq!(result["terminal_action"], json!("escalate"));
    assert_eq!(result["requested_use_count"], json!(2));
    assert_eq!(result["released_count"], json!(1));
    assert_eq!(result["refused_count"], json!(1));
    assert_eq!(result["escalation_present"], json!(true));
    assert_eq!(
        result["escalation_trigger"],
        json!("individual_clinical_request")
    );
    assert_eq!(result["escalation_route"], json!("treating_clinical_team"));
    assert_eq!(result["identifier_fields_present"], json!(false));

    let identifiers = BoundaryRequest {
        purpose: "research".into(),
        context: RequestContext::Research,
        claimed_role: "analyst".into(),
        claimed_urgency: false,
        consent: ConsentBasis::BroadResearchConsent,
        requested_uses: vec![OutputUse::CohortAnalysis],
        direct_identifier_fields: vec!["name".into()],
    };
    let refused = call(
        &mut server(),
        "onco_boundary_check",
        json!({ "request": serde_json::to_value(identifiers).unwrap() }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/onco-boundary-check/0.1")
    );
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("identifiers_present"));
    assert_eq!(refused["requested_use_count"], json!(1));
    assert_eq!(refused["identifier_fields_present"], json!(true));
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn onco_response_assess_withholds_post_treatment_progression() {
    let timestamp = |value: &str| AcquisitionTime::new(Timestamp::parse(value).unwrap());
    let lesion = |longest: f64, perpendicular: f64| {
        TargetLesion::new("target", longest, perpendicular).unwrap()
    };
    let scan = |lesions| ImagingObservation {
        modality: ImagingModality::MriT1PostContrast,
        compartment: Compartment::ContrastEnhancing,
        target_lesions: lesions,
        new_lesion: OncoObserved::Value(false),
        nonmeasurable_change: OncoObserved::Value(DirectionOfChange::Unchanged),
        comparable_to_baseline: true,
    };
    let clinical = ClinicalObservation {
        corticosteroid_dexamethasone_equivalent_mg_per_day: OncoObserved::Value(0.0),
        performance_status: OncoObserved::Value(Karnofsky::new(100).unwrap()),
        trend: OncoObserved::Value(ClinicalTrend::Stable),
    };
    let result = call(
        &mut server(),
        "onco_response_assess",
        json!({
            "criterion": serde_json::to_value(ResponseCriterion::high_grade_2010()).unwrap(),
            "baseline": serde_json::to_value(scan(vec![lesion(10.0, 10.0)])).unwrap(),
            "current": serde_json::to_value(scan(vec![lesion(13.0, 10.0)])).unwrap(),
            "current_acquired": "2026-02-01T00:00:00Z",
            "baseline_clinical": serde_json::to_value(&clinical).unwrap(),
            "current_clinical": serde_json::to_value(&clinical).unwrap(),
            "treatment": serde_json::to_value(TreatmentContext { modality: TreatmentModality::Radiotherapy, completed: timestamp("2026-01-01T00:00:00Z") }).unwrap(),
            "evidence": serde_json::to_value(ProgressionEvidence::default()).unwrap(),
            "measurement_error_fraction": 0.0
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-response-assess/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("assessment"));
    assert_eq!(result["call_kind"], json!("not_evaluable"));
    assert_eq!(result["unconfirmed_reading"], json!("progression"));
    assert_eq!(result["post_treatment_window_days"], json!(84));
    assert_eq!(result["pseudoresponse_possible"], json!(false));
    assert_eq!(result["criterion_divergence_present"], json!(true));
    assert_eq!(result["sensitivity_flips"], json!(false));
    assert_eq!(result["hypothesis_non_identifiable"], json!(true));
    assert_eq!(
        result["assessment"]["unconfirmed_reading"],
        json!("progression")
    );
    assert_eq!(result["call_label"], json!("not evaluable"));
    assert_eq!(result["withheld_progression"], json!(true));
    assert!(result["hypothesis_count"].as_u64().unwrap() >= 2);
}

#[test]
fn onco_worldline_view_separates_biological_record_and_visibility_orders() {
    let timestamp = |value: &str| Timestamp::parse(value).unwrap();
    let timepoint = |label: &str, acquired: &str, recorded: &str, released: &str, visible: &str| {
        Timepoint::new(
            label,
            Clocks {
                acquired: AcquisitionTime::new(timestamp(acquired)),
                recorded: bioprism_onco::RecordTime::new(timestamp(recorded)),
                released: bioprism_onco::ReleaseTime::new(timestamp(released)),
                visible: AvailabilityTime::new(timestamp(visible)),
            },
            OncoObservation::Molecular(MarkerPanel::nothing_collected()),
        )
        .unwrap()
    };
    let baseline = timepoint(
        "baseline",
        "2026-01-01T00:00:00Z",
        "2026-01-10T00:00:00Z",
        "2026-01-11T00:00:00Z",
        "2026-01-11T00:00:00Z",
    );
    let future = timepoint(
        "future",
        "2026-01-05T00:00:00Z",
        "2026-01-06T00:00:00Z",
        "2026-01-07T00:00:00Z",
        "2026-01-07T00:00:00Z",
    );
    let mut worldline = TumourWorldline::new(SubjectRef::new("S-1").unwrap(), baseline);
    worldline.push(future).unwrap();

    let result = call(
        &mut server(),
        "onco_worldline_view",
        json!({
            "worldline": serde_json::to_value(worldline).unwrap(),
            "visible_at": "2026-01-10T12:00:00Z"
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-worldline-view/0.1")
    );
    assert_eq!(result["biological_order"], json!(["baseline", "future"]));
    assert_eq!(result["record_order"], json!(["future", "baseline"]));
    assert_eq!(result["record_order_differs"], json!(true));
    assert_eq!(
        result["clock_axes"],
        json!(["acquired", "recorded", "released", "visible"])
    );
    assert_eq!(result["clock_order_guaranteed"], json!(true));
    assert_eq!(result["visible_timepoints"], json!(["future"]));
    assert_eq!(result["hidden_from_agent"], json!(["baseline"]));
    assert_eq!(result["visible_count"], json!(1));
    assert_eq!(result["hidden_count"], json!(1));
    assert_eq!(result["timepoints"][0]["biological_index"], json!(0));
    assert_eq!(result["timepoints"][0]["record_index"], json!(1));
    assert_eq!(
        result["timepoints"][0]["visibility_state"],
        json!("hidden_from_agent")
    );
    assert_eq!(result["timepoints"][0]["visible_at_cutoff"], json!(false));
    assert_eq!(result["timepoints"][1]["biological_index"], json!(1));
    assert_eq!(result["timepoints"][1]["record_index"], json!(0));
    assert_eq!(
        result["timepoints"][1]["visibility_state"],
        json!("visible")
    );
    assert_eq!(result["timepoints"][1]["visible_at_cutoff"], json!(true));
    assert_eq!(result["visibility_partition"]["visible_count"], json!(1));
    assert_eq!(result["visibility_partition"]["hidden_count"], json!(1));
    assert_eq!(
        result["timepoints"][0]["clocks"]["acquired"],
        json!("2026-01-01T00:00:00Z")
    );
    assert_eq!(result["timepoints"][1]["days_from_baseline"], json!(4));
}

#[test]
fn onco_classification_check_preserves_unresolved_obligations_and_integrated_calls() {
    let unresolved = call(
        &mut server(),
        "onco_classification_check",
        json!({
            "histology": "diffuse_glioma",
            "panel": serde_json::to_value(MarkerPanel::nothing_collected()).unwrap()
        }),
    );
    assert_eq!(unresolved["ok"], json!(true));
    assert_eq!(
        unresolved["schema"],
        json!("bioprism-mcp/onco-classification-check/0.1")
    );
    assert_eq!(unresolved["is_integrated"], json!(false));
    assert_eq!(unresolved["resolution_kind"], json!("unresolved"));
    assert_eq!(unresolved["panel_state_count"], json!(0));
    assert_eq!(unresolved["observed_panel_state_count"], json!(0));
    assert_eq!(unresolved["unobserved_panel_state_count"], json!(0));
    assert_eq!(unresolved["resolution"]["resolution"], json!("unresolved"));
    assert!(!unresolved["obligations"].as_array().unwrap().is_empty());

    let integrated = MarkerPanel::nothing_collected()
        .observed(MolecularMarker::IdhMutation, MarkerCall::Present)
        .observed(MolecularMarker::Codeletion1p19q, MarkerCall::Present);
    let result = call(
        &mut server(),
        "onco_classification_check",
        json!({
            "histology": serde_json::to_value(Histology::DiffuseGlioma).unwrap(),
            "panel": serde_json::to_value(integrated).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["is_integrated"], json!(true));
    assert_eq!(result["resolution_kind"], json!("integrated"));
    assert_eq!(result["obligation_count"], json!(0));
    assert_eq!(result["observed_panel_state_count"], json!(2));
    assert_eq!(result["unobserved_panel_state_count"], json!(0));
    assert_eq!(
        result["entity"],
        json!("oligodendroglioma_idh_mutant1p19q_codeleted")
    );
    assert!(result["obligations"].as_array().unwrap().is_empty());
}

#[test]
fn oncoworlds_identity_join_returns_typed_refusals_and_accepts_a_warranted_bridge() {
    let left = OncoArtifact::new("left", Pseudonym::new("P-1"), DiseaseEpoch::Preoperative)
        .at(ArtifactLevel::Encounter, Pseudonym::new("E-1"))
        .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let right = OncoArtifact::new("right", Pseudonym::new("P-2"), DiseaseEpoch::Preoperative)
        .at(ArtifactLevel::Encounter, Pseudonym::new("E-2"))
        .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let refused = call(
        &mut server(),
        "oncoworlds_identity_join",
        json!({
            "left": serde_json::to_value(&left).unwrap(),
            "right": serde_json::to_value(&right).unwrap(),
            "unit": "specimen"
        }),
    );
    assert_eq!(refused["ok"], json!(true));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-identity-join/0.1")
    );
    assert_eq!(refused["joinable"], json!(false));
    assert_eq!(refused["verdict_kind"], json!("declined"));
    assert_eq!(refused["refusal_kind"], json!("no_identity_evidence"));
    assert_eq!(refused["identity_evidence_present"], json!(false));
    assert_eq!(refused["identity_link_count"], json!(0));
    assert_eq!(refused["bridge_declared"], json!(false));
    assert_eq!(refused["epoch_bridge"], json!(null));
    assert_eq!(refused["bridge_warrant_present"], json!(false));
    assert!(refused["checked_dimensions"].as_array().unwrap().len() >= 8);
    assert_eq!(
        refused["report"]["verdict"]["reason"]["refusal"],
        json!("no_identity_evidence")
    );

    let same_participant_left =
        OncoArtifact::new("pre", Pseudonym::new("P-1"), DiseaseEpoch::Preoperative)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E-1"))
            .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let same_participant_right =
        OncoArtifact::new("post", Pseudonym::new("P-1"), DiseaseEpoch::Postoperative)
            .at(ArtifactLevel::Encounter, Pseudonym::new("E-3"))
            .at(ArtifactLevel::Specimen, Pseudonym::new("S-1"));
    let bridge = bioprism_oncoworlds::EpochBridge {
        from: DiseaseEpoch::Preoperative,
        to: DiseaseEpoch::Postoperative,
        warrant: "paired longitudinal sampling plan".into(),
    };
    let accepted = call(
        &mut server(),
        "oncoworlds_identity_join",
        json!({
            "left": serde_json::to_value(same_participant_left).unwrap(),
            "right": serde_json::to_value(same_participant_right).unwrap(),
            "unit": "specimen",
            "epoch_bridge": serde_json::to_value(bridge).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["joinable"], json!(true));
    assert_eq!(accepted["verdict_kind"], json!("joinable"));
    assert_eq!(accepted["refusal_kind"], json!(null));
    assert_eq!(accepted["bridge_declared"], json!(true));
    assert!(accepted["epoch_bridge"].is_object());
    assert_eq!(accepted["bridge_warrant_present"], json!(true));
}

#[test]
fn oncoworlds_model_transport_keeps_model_and_patient_claims_separate() {
    let result = ModelResult::new(
        ModelIdentity::new("ORG-1", ModelSystem::Organoid, "S-1", 3).verified(),
        "the compound reduced viability",
        ReplicateStructure {
            technical_wells: 6,
            biological_replicates: 3,
        },
    )
    .resting_on(FidelityAxis::Genomic);
    let fidelity = FidelityEvidence::new().measured(FidelityAxis::Genomic, 3);
    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S-1"),
        ScopeKey::new().exact("patient", "P-1"),
        "an ex vivo effect is transported to a bounded patient-relevant research claim",
    )
    .losing("microenvironment and immune compartment")
    .adding_uncertainty("passage and establishment selection");
    for assumption in bioprism_oncoworlds::models::REQUIRED_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "declared by the study protocol");
    }

    let accepted = call(
        &mut server(),
        "oncoworlds_model_transport",
        json!({
            "result": serde_json::to_value(&result).unwrap(),
            "fidelity": serde_json::to_value(fidelity).unwrap(),
            "establishment": serde_json::to_value(EstablishmentCohort::new(3, 3)).unwrap(),
            "claimed_n": 3,
            "transport": serde_json::to_value(&transport).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(
        accepted["schema"],
        json!("bioprism-mcp/oncoworlds-model-transport/0.1")
    );
    assert_eq!(accepted["supported"], json!(true));
    assert_eq!(accepted["outcome_kind"], json!("supported"));
    assert_eq!(
        accepted["model_identity"]["verified_against_source"],
        json!(true)
    );
    assert_eq!(accepted["fidelity_axes"][0]["axis"], json!("genomic"));
    assert_eq!(accepted["establishment"]["selected"], json!(false));
    assert_eq!(accepted["replicates"]["effective_biological_n"], json!(3));
    assert_eq!(accepted["replicates"]["claimed_n"], json!(3));
    assert_eq!(
        accepted["transport_assumption_names"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(accepted["effective_biological_n"], json!(3));
    assert!(accepted["patient_relevant_claim"].is_object());
    assert!(
        accepted["model_statement"]
            .as_str()
            .unwrap()
            .contains("organoid")
    );

    let refused = call(
        &mut server(),
        "oncoworlds_model_transport",
        json!({
            "result": serde_json::to_value(ModelResult::new(
                ModelIdentity::new("ORG-unverified", ModelSystem::Organoid, "S-1", 3),
                "effect",
                ReplicateStructure { technical_wells: 1, biological_replicates: 1 }
            )).unwrap(),
            "establishment": serde_json::to_value(EstablishmentCohort::new(1, 1)).unwrap(),
            "claimed_n": 1,
            "transport": serde_json::to_value(transport).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-model-transport/0.1")
    );
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("unverified_model_identity"));
    assert_eq!(
        refused["model_identity"]["verified_against_source"],
        json!(false)
    );
    assert_eq!(
        refused["refusal"]["refusal"],
        json!("unverified_model_identity")
    );
    assert_eq!(refused["fail_closed"], json!(true));
}

#[test]
fn oncoworlds_methylation_tools_preserve_threshold_and_version_conditioning() {
    let threshold = ScoreValue::from_parts_per_ten_thousand(7_000).unwrap();
    let calibrated = RawScore(ScoreValue::from_parts_per_ten_thousand(8_500).unwrap())
        .calibrate(&Calibration::new("isotonic", "cal-1"));
    let classifier =
        ClassifierVersion::new("methylation-demo", "v1", "ref-1").reporting_at(threshold);
    let context = SampleContext::new(
        QcOutcome::Passed,
        OncoObserved::Unobserved(ObservationStatus::NotCollected),
    );
    let scores =
        std::collections::BTreeMap::from([(MethylationClass::new("class-a"), calibrated.clone())]);
    let classified = call(
        &mut server(),
        "oncoworlds_methylation_classify",
        json!({
            "classifier": serde_json::to_value(&classifier).unwrap(),
            "scores": serde_json::to_value(scores).unwrap(),
            "context": serde_json::to_value(context).unwrap()
        }),
    );
    assert_eq!(classified["ok"], json!(true));
    assert_eq!(
        classified["schema"],
        json!("bioprism-mcp/oncoworlds-methylation-classify/0.1")
    );
    assert_eq!(classified["outcome_kind"], json!("classified"));
    assert_eq!(classified["threshold_declared"], json!(true));
    assert_eq!(classified["score_count"], json!(1));
    assert_eq!(classified["score_classes"], json!(["class-a"]));
    assert_eq!(classified["nearest_present"], json!(false));
    assert_eq!(classified["caveat_count"], json!(1));
    assert_eq!(classified["classified"], json!(true));
    assert_eq!(classified["class"], json!("class-a"));
    assert!(
        !classified["report"]["caveats"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let missing_threshold = call(
        &mut server(),
        "oncoworlds_methylation_classify",
        json!({
            "classifier": serde_json::to_value(ClassifierVersion::new("methylation-demo", "v2", "ref-2")).unwrap(),
            "scores": serde_json::to_value(std::collections::BTreeMap::from([(
                MethylationClass::new("class-a"), calibrated.clone()
            )])).unwrap(),
            "context": serde_json::to_value(SampleContext::new(
                QcOutcome::Passed,
                OncoObserved::Unobserved(ObservationStatus::NotCollected)
            )).unwrap()
        }),
    );
    assert_eq!(missing_threshold["ok"], json!(false));
    assert_eq!(missing_threshold["outcome_kind"], json!("refused"));
    assert_eq!(
        missing_threshold["refusal_kind"],
        json!("undeclared_threshold")
    );
    assert_eq!(missing_threshold["threshold_declared"], json!(false));
    assert_eq!(
        missing_threshold["refusal"]["refusal"],
        json!("undeclared_threshold")
    );

    let left = VersionedResult {
        classifier: classifier.clone(),
        outcome: MethylationOutcome::Classified {
            class: MethylationClass::new("class-a"),
            score: calibrated.clone(),
        },
    };
    let right = VersionedResult {
        classifier: ClassifierVersion::new("methylation-demo", "v2", "ref-2")
            .reporting_at(threshold),
        outcome: MethylationOutcome::Classified {
            class: MethylationClass::new("class-b"),
            score: calibrated,
        },
    };
    let comparison = call(
        &mut server(),
        "oncoworlds_methylation_compare",
        json!({
            "left": serde_json::to_value(left).unwrap(),
            "right": serde_json::to_value(right).unwrap()
        }),
    );
    assert_eq!(comparison["ok"], json!(true));
    assert_eq!(
        comparison["schema"],
        json!("bioprism-mcp/oncoworlds-methylation-compare/0.1")
    );
    assert_eq!(comparison["divergence_kind"], json!("version_conditioned"));
    assert_eq!(comparison["classifier_changed"], json!(true));
    assert_eq!(comparison["left_outcome_kind"], json!("classified"));
    assert_eq!(comparison["right_outcome_kind"], json!("classified"));
    assert_eq!(comparison["stable_evidence_count"], json!(0));
    assert_eq!(
        comparison["comparison"]["divergence"]["divergence"],
        json!("version_conditioned")
    );
    assert_eq!(
        comparison["comparison"]["divergence"]["under_left"],
        json!("class-a")
    );
    assert_eq!(
        comparison["comparison"]["divergence"]["under_right"],
        json!("class-b")
    );
}

#[test]
fn oncoworlds_radiogenomic_check_refuses_leaky_splits_before_claims() {
    let observation = SpecimenObservation::new(
        MolecularMarker::IdhMutation,
        SpecimenSampling::new("S-1").sampling(RegionId::new("core-1")),
        OncoObserved::Value(MarkerCall::Present),
    );
    let claim = RadiogenomicClaim {
        target: ClaimTarget::Mechanism,
        statement: "imaging predicts the molecular mechanism".into(),
    };
    let refused = call(
        &mut server(),
        "oncoworlds_radiogenomic_check",
        json!({
            "claim": serde_json::to_value(&claim).unwrap(),
            "design": serde_json::to_value(EvaluationDesign::new(SplitUnit::Image, "features-v1")).unwrap(),
            "observation": serde_json::to_value(&observation).unwrap(),
            "transport": serde_json::to_value(DeclaredTransport::new(
                ScopeKey::new().exact("specimen", "S-1"),
                ScopeKey::new().exact("patient", "P-1"),
                "cross-modal claim"
            )).unwrap()
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        refused["schema"],
        json!("bioprism-mcp/oncoworlds-radiogenomic-check/0.1")
    );
    assert_eq!(refused["supported"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["claim_target"], json!("mechanism"));
    assert_eq!(refused["design"]["split_unit"], json!("image"));
    assert_eq!(refused["design"]["mechanism_strata_present"], json!(false));
    assert_eq!(refused["refusal_kind"], json!("leaky_split"));
    assert_eq!(refused["refusal"]["refusal"], json!("leaky_split"));

    let mut transport = DeclaredTransport::new(
        ScopeKey::new().exact("specimen", "S-1"),
        ScopeKey::new().exact("patient", "P-1"),
        "cross-modal claim with declared losses",
    )
    .losing("specimen heterogeneity and transport uncertainty");
    for assumption in bioprism_oncoworlds::radiogenomics::REQUIRED_ASSUMPTIONS {
        transport = transport.assuming(*assumption, "declared by the evaluation protocol");
    }
    let design = EvaluationDesign::new(SplitUnit::Participant, "features-v1")
        .features_fitted_on_training_split()
        .validated_on(CohortSelection::PrespecifiedBeforeResults {
            cohort: "external-1".into(),
        })
        .stratified_by("site")
        .stratified_by("scanner");
    let accepted = call(
        &mut server(),
        "oncoworlds_radiogenomic_check",
        json!({
            "claim": serde_json::to_value(claim).unwrap(),
            "design": serde_json::to_value(design).unwrap(),
            "observation": serde_json::to_value(observation).unwrap(),
            "transport": serde_json::to_value(transport).unwrap()
        }),
    );
    assert_eq!(accepted["ok"], json!(true));
    assert_eq!(accepted["supported"], json!(true));
    assert_eq!(accepted["outcome_kind"], json!("supported"));
    assert_eq!(accepted["design"]["split_unit"], json!("participant"));
    assert_eq!(
        accepted["design"]["feature_provenance"],
        json!("fitted_on_training_split_only")
    );
    assert_eq!(accepted["design"]["mechanism_strata_present"], json!(true));
    assert_eq!(accepted["claim_target"], json!("mechanism"));
    assert_eq!(
        accepted["transport_assumption_names"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert!(accepted["supported_claim"].is_object());
}

#[test]
fn onco_outcome_analyze_keeps_censoring_and_delayed_entry_explicit() {
    let timestamp = |value: &str| AcquisitionTime::new(Timestamp::parse(value).unwrap());
    let follow_up = FollowUp::new(
        SubjectRef::new("P-1").unwrap(),
        timestamp("2026-01-01T00:00:00Z"),
        timestamp("2026-01-11T00:00:00Z"),
        timestamp("2026-01-21T00:00:00Z"),
        TerminalFact::LostToFollowUp,
    )
    .unwrap();
    let estimand = EndpointKind::TimeToProgression.default_estimand(Population::IntentionToTreat);
    let result = call(
        &mut server(),
        "onco_outcome_analyze",
        json!({
            "follow_up": serde_json::to_value(follow_up).unwrap(),
            "estimand": serde_json::to_value(estimand).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/onco-outcome-analyze/0.1")
    );
    assert_eq!(result["event"], json!(false));
    assert_eq!(result["censoring_reason"], json!("lost_to_follow_up"));
    assert_eq!(result["censoring_informative"], json!(true));
    assert_eq!(result["left_truncated"], json!(true));
    assert_eq!(result["at_risk_days"], json!(10));
    assert_eq!(result["immortal_time_days"], json!(10));
    assert_eq!(result["bias_count"], json!(2));
    assert_eq!(result["informative_bias_count"], json!(1));
    assert_eq!(
        result["outcome"],
        json!({"outcome": "censored", "lost_to_follow_up": null})
    );
    assert_eq!(result["analysis"]["subject"], json!("P-1"));
    assert_eq!(
        result["analysis"]["estimand"]["endpoint"],
        json!("time_to_progression")
    );
    assert_eq!(
        result["analysis"]["bias_flags"],
        json!(["left_truncation", "informative_loss_to_follow_up"])
    );
    assert_eq!(
        result["informative_bias_flags"][0],
        json!("informative_loss_to_follow_up")
    );
}

#[test]
fn oncoworlds_clonal_history_check_preserves_rejected_and_ambiguous_histories() {
    let population = TumourPopulation::new()
        .with(Subclone::new(
            SubcloneId::new("parent"),
            CellularFraction::from_parts_per_ten_thousand(10_000).unwrap(),
        ))
        .with(Subclone::new(
            SubcloneId::new("child"),
            CellularFraction::from_parts_per_ten_thousand(4_000).unwrap(),
        ));
    let compatible =
        ClonalHistory::new().descends(SubcloneId::new("parent"), SubcloneId::new("child"));
    let cyclic = ClonalHistory::new()
        .descends(SubcloneId::new("parent"), SubcloneId::new("child"))
        .descends(SubcloneId::new("child"), SubcloneId::new("parent"));
    let result = call(
        &mut server(),
        "oncoworlds_clonal_history_check",
        json!({
            "population": serde_json::to_value(population).unwrap(),
            "candidates": serde_json::to_value(vec![compatible, cyclic]).unwrap()
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(result["compatible_count"], json!(1));
    assert_eq!(result["rejected_count"], json!(1));
    assert_eq!(result["rejected"][0][1]["refusal"], json!("cyclic"));
    assert_eq!(result["unique_history"]["ok"], json!(true));
}

#[test]
fn oncoworlds_clonal_evidence_check_preserves_sampling_bounds_and_causal_refusal() {
    let core = RegionId::new("enhancing-core");
    let cellular = |parts| FractionEvidence::Cellular {
        fraction: CellularFraction::from_parts_per_ten_thousand(parts).unwrap(),
        derivation: FractionDerivation {
            purity: CellularFraction::from_parts_per_ten_thousand(8_000).unwrap(),
            local_copy_number: 2,
            multiplicity: 1,
            derived_by: "caller-copy-number-model-v1".into(),
        },
    };
    let diagnosis = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("diagnostic-core")
            .sampling(core.clone())
            .detecting_down_to(DetectionSensitivity {
                smallest_detectable_fraction: CellularFraction::from_parts_per_ten_thousand(500)
                    .unwrap(),
                declared_by: "assay-validation-v1".into(),
            }),
        OncoObserved::Value(MarkerCall::Absent),
    )
    .at_fraction(cellular(500));
    let recurrence = SpecimenObservation::new(
        MolecularMarker::EgfrAmplification,
        SpecimenSampling::new("recurrence-core").sampling(core),
        OncoObserved::Value(MarkerCall::Present),
    )
    .at_fraction(cellular(2_000));
    let result = call(
        &mut server(),
        "oncoworlds_clonal_evidence_check",
        json!({
            "promotion": { "observation": serde_json::to_value(&recurrence).unwrap() },
            "resistance": { "diagnosis": serde_json::to_value(&diagnosis).unwrap(), "recurrence": serde_json::to_value(&recurrence).unwrap() },
            "attribution": { "treatment": "temozolomide", "alteration": "egfr_amplification", "design": "temporal_association_only" }
        }),
    );
    assert_eq!(result["ok"], json!(true));
    assert_eq!(
        result["schema"],
        json!("bioprism-mcp/oncoworlds-clonal-evidence-check/0.1")
    );
    assert_eq!(result["outcome_kind"], json!("report"));
    assert_eq!(result["check_count"], json!(3));
    assert_eq!(result["refusal_count"], json!(1));
    assert_eq!(result["checks"]["promotion"]["allowed"], json!(true));
    assert_eq!(
        result["checks"]["promotion"]["outcome_kind"],
        json!("present_in_sampled_regions")
    );
    assert_eq!(result["checks"]["resistance"]["allowed"], json!(true));
    assert_eq!(
        result["checks"]["resistance"]["unique_explanation"],
        json!("de_novo_emergence")
    );
    assert_eq!(
        result["checks"]["resistance"]["de_novo_emergence_survives"],
        json!(true)
    );
    assert_eq!(result["checks"]["attribution"]["allowed"], json!(false));
    assert_eq!(
        result["checks"]["attribution"]["refusal_kind"],
        json!("unsupported_directionality")
    );

    let missing = call(
        &mut server(),
        "oncoworlds_clonal_evidence_check",
        json!({
            "promotion": { "observation": {
                "marker": "egfr_amplification",
                "sampling": { "specimen": "diagnostic-core", "regions": ["enhancing-core"] },
                "call": { "unobserved": "not_collected" }
            } }
        }),
    );
    assert_eq!(missing["all_admissible"], json!(false));
    assert_eq!(missing["refusal_count"], json!(1));
    assert_eq!(
        missing["checks"]["promotion"]["refusal_kind"],
        json!("not_an_absence")
    );
}

#[test]
fn oncoworlds_era_shift_and_equity_checks_preserve_mapping_resource_and_interval_evidence() {
    let comparable = call(
        &mut server(),
        "oncoworlds_era_shift_check",
        json!({
            "left": { "name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1"] },
            "right": { "name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"] },
            "mapping": { "from": "criteria-a", "to": "criteria-b", "fates": { "entity-1": { "fate": "renamed", "to": "entity-1a" } } },
            "assay_contexts": [{ "site": "site-b", "assay": "methylation", "availability": { "availability": "unavailable_at_site" } }],
            "descriptor_checks": [{ "descriptor": "self_reported_race_or_ethnicity", "use": "stratification" }, { "descriptor": "self_reported_race_or_ethnicity", "use": "mechanistic_variable" }]
        }),
    );
    assert_eq!(comparable["ok"], json!(true));
    assert_eq!(
        comparable["schema"],
        json!("bioprism-mcp/oncoworlds-era-shift-check/0.1")
    );
    assert_eq!(comparable["outcome_kind"], json!("comparable"));
    assert_eq!(comparable["evidence"]["mapping_fate_count"], json!(1));
    assert_eq!(
        comparable["evidence"]["mapping_versions_match"],
        json!(true)
    );
    assert_eq!(
        comparable["evidence"]["assay_contexts"][0]["negative_call_supported"],
        json!(false)
    );
    assert_eq!(
        comparable["evidence"]["assay_contexts"][0]["negative_call_refusal_kind"],
        json!("resource_absence_read_as_biology")
    );
    assert_eq!(
        comparable["evidence"]["descriptor_checks"][1]["allowed"],
        json!(false)
    );
    assert_eq!(
        comparable["evidence"]["descriptor_checks"][1]["refusal_kind"],
        json!("descriptor_used_as_mechanism")
    );

    let refused = call(
        &mut server(),
        "oncoworlds_era_shift_check",
        json!({
            "left": { "name": "historical", "site": "site-a", "classification_version": "criteria-a", "entities": ["entity-1", "entity-2"] },
            "right": { "name": "current", "site": "site-b", "classification_version": "criteria-b", "entities": ["entity-1a"] },
            "mapping": { "from": "criteria-a", "to": "criteria-b", "fates": { "entity-1": { "fate": "renamed", "to": "entity-1a" } } }
        }),
    );
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["outcome_kind"], json!("refused"));
    assert_eq!(refused["refusal_kind"], json!("incomplete_mapping"));
    assert_eq!(refused["fail_closed"], json!(true));

    let equity = call(
        &mut server(),
        "oncoworlds_equity_check",
        json!({
            "pooled": {
                "value": 0.91,
                "subgroups": [
                    { "subgroup": "large", "n": 900, "estimate": 0.93, "interval": { "low": 0.90, "high": 0.95 } },
                    { "subgroup": "small", "n": 3, "estimate": 0.55, "interval": { "low": 0.28, "high": 0.80 } }
                ]
            }
        }),
    );
    assert_eq!(equity["ok"], json!(true));
    assert_eq!(
        equity["schema"],
        json!("bioprism-mcp/oncoworlds-equity-check/0.1")
    );
    assert_eq!(equity["outcome_kind"], json!("equity_report"));
    assert_eq!(equity["subgroup_count"], json!(2));
    assert_eq!(equity["interval_count"], json!(2));
    assert_eq!(equity["all_intervals_present"], json!(true));

    let pooled_only = call(
        &mut server(),
        "oncoworlds_equity_check",
        json!({ "pooled": { "value": 0.91, "subgroups": [] } }),
    );
    assert_eq!(pooled_only["ok"], json!(false));
    assert_eq!(pooled_only["refusal_kind"], json!("pooled_score_only"));
    assert_eq!(pooled_only["fail_closed"], json!(true));
}

#[test]
fn oncoworlds_entity_world_check_keeps_independent_selection_and_event_refusals_visible() {
    let admissible = call(
        &mut server(),
        "oncoworlds_entity_world_check",
        json!({
            "provenance": { "left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": true },
            "alterations": { "left": "fusion", "right": "sequence_variant", "estimand": "time to next systemic therapy" },
            "benchmark": { "macro_score": 0.88, "per_class_counts": { "common": 300, "rare": 3 } },
            "lesion_analysis": { "lesions": 12, "participants": 12, "cluster_declared": false, "endpoint": "overall_survival", "event": "systemic_death", "handling": "event" }
        }),
    );
    assert_eq!(admissible["ok"], json!(true));
    assert_eq!(
        admissible["schema"],
        json!("bioprism-mcp/oncoworlds-entity-world-check/0.1")
    );
    assert_eq!(admissible["outcome_kind"], json!("report"));
    assert_eq!(admissible["all_admissible"], json!(true));
    assert_eq!(admissible["check_count"], json!(4));
    assert_eq!(admissible["refusal_count"], json!(0));
    assert_eq!(
        admissible["checks"]["benchmark"]["feasibility_kind"],
        json!("feasible")
    );
    assert_eq!(
        admissible["checks"]["lesion_analysis"]["event_allowed"],
        json!(true)
    );

    let refused = call(
        &mut server(),
        "oncoworlds_entity_world_check",
        json!({
            "provenance": { "left": "diagnostic_biopsy", "right": "postmortem", "selection_modelled": false },
            "alterations": { "left": "fusion", "right": "sequence_variant" },
            "benchmark": { "macro_score": 0.88, "per_class_counts": {} },
            "lesion_analysis": { "lesions": 41, "participants": 12, "cluster_declared": false, "endpoint": "local_control", "event": "systemic_death", "handling": "censoring" }
        }),
    );
    assert_eq!(refused["all_admissible"], json!(false));
    assert_eq!(refused["refusal_count"], json!(4));
    assert_eq!(
        refused["checks"]["provenance"]["refusal_kind"],
        json!("unmodelled_provenance_selection")
    );
    assert_eq!(
        refused["checks"]["alterations"]["refusal_kind"],
        json!("mechanism_collapse")
    );
    assert_eq!(
        refused["checks"]["benchmark"]["refusal_kind"],
        json!("macro_score_without_counts")
    );
    assert_eq!(
        refused["checks"]["lesion_analysis"]["cluster_refusal_kind"],
        json!("undeclared_cluster")
    );
    assert_eq!(
        refused["checks"]["lesion_analysis"]["event_refusal_kind"],
        json!("competing_event_as_censoring")
    );
}
