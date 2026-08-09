//! Invariants of the standards layer.
//!
//! Each test states a claim that section 28 or 39.05 makes about biological data, and fails if
//! the crate would let that claim be violated silently.

use bioprism_scope::{MappingCheck, ScopeClass};
use bioprism_standards::{
    comparable, comparable_under, reconcile, report, BuildBinding, BuildTransport,
    ComparabilityPolicy, CoordinateConvention, CoordinateSpace, Dimension, Extent, Frame,
    FrameBinding, GenomeBuild, GenomicInterval, GenomicPosition, Handedness, Incomparability,
    LiftOutcome, MappingPrecision, Measurement, OntologyError, OntologyId, Orientation, Position,
    Quantity, ReferenceError, ReferenceSpace, TermBinding, TermCatalog, Unit, UnitError, Verdict,
};

fn mm() -> Unit {
    Unit::parse("mm").expect("mm is in the table")
}

fn mni() -> ReferenceSpace {
    ReferenceSpace::Template {
        name: "MNI152NLin2009cAsym".to_string(),
        version: "1.0".to_string(),
    }
}

fn ras_frame() -> Frame {
    Frame::world("mni-ras", Orientation::RAS, mni())
}

fn lps_frame() -> Frame {
    Frame::world("mni-lps", Orientation::LPS, mni())
}

fn mondo_gbm(release: &str) -> OntologyId {
    OntologyId::parse("MONDO:0018177", release).expect("well-formed CURIE")
}

#[test]
fn millimetres_and_millilitres_cannot_be_added() {
    let length = Quantity::parse(24.0, "mm").unwrap();
    let volume = Quantity::parse(12.5, "mL").unwrap();

    let error = length.add(&volume).unwrap_err();

    assert!(matches!(error, UnitError::DimensionMismatch { .. }));
}

#[test]
fn millimetres_and_centimetres_need_an_explicit_conversion() {
    let millimetres = Quantity::parse(24.0, "mm").unwrap();
    let centimetres = Quantity::parse(2.4, "cm").unwrap();

    let error = millimetres.add(&centimetres).unwrap_err();

    assert!(
        matches!(error, UnitError::UnitMismatch { .. }),
        "same dimension, different unit is a missing conversion, not a modelling mistake: {error}"
    );
}

#[test]
fn converting_millimetres_to_centimetres_records_the_conversion() {
    let millimetres = Quantity::parse(24.0, "mm").unwrap();

    let converted = millimetres.convert_to(&Unit::parse("cm").unwrap()).unwrap();

    assert!((converted.quantity.value - 2.4).abs() < 1e-12);
    assert_eq!(converted.record.from, "mm");
    assert_eq!(converted.record.to, "cm");
    assert!(converted.record.is_exact());
}

#[test]
fn an_exact_conversion_declares_no_loss() {
    let converted = Quantity::parse(1000.0, "mm3")
        .unwrap()
        .convert_to(&Unit::parse("mL").unwrap())
        .unwrap();

    assert!((converted.quantity.value - 1.0).abs() < 1e-12, "1000 mm3 is 1 mL");
    assert!(converted.record.loss_ledger().is_empty());
}

#[test]
fn months_convert_only_under_a_stated_convention() {
    let survival = Quantity::parse(14.6, "month").unwrap();

    let converted = survival.convert_to(&Unit::parse("day").unwrap()).unwrap();

    assert!(!converted.record.is_exact(), "a month is not a fixed number of days");
    let ledger = converted.record.loss_ledger();
    assert!(!ledger.is_empty());
    assert!(ledger.uncertainty_added[0].contains("30.436875"));
}

#[test]
fn an_unknown_unit_symbol_is_an_error_not_a_new_unit() {
    let error = Unit::parse("millimetre").unwrap_err();

    assert!(matches!(error, UnitError::UnknownUnit { .. }));
}

#[test]
fn mass_per_body_mass_and_a_plain_percent_are_not_commensurable() {
    let dose = Unit::parse("mg/kg").unwrap();
    let percent = Unit::parse("%").unwrap();

    assert_eq!(dose.dimension, percent.dimension, "both are dimensionless");

    let error = dose.commensurable_with(&percent).unwrap_err();
    assert!(matches!(error, UnitError::NotCommensurable { .. }));
}

#[test]
fn body_surface_dosing_times_an_area_is_a_mass() {
    let dose = Quantity::parse(150.0, "mg/m2").unwrap();
    let surface = Quantity::parse(1.8, "m2").unwrap();

    let total = dose.mul(&surface).unwrap();

    assert_eq!(total.unit.dimension, Dimension::MASS);
    assert_eq!(total.unit.symbol, "mg");
    assert!((total.value - 270.0).abs() < 1e-9);
}

#[test]
fn gray_is_not_treated_as_a_squared_velocity() {
    let seconds = Unit::parse("s").unwrap();
    let squared_velocity = Unit::parse("m2")
        .unwrap()
        .checked_div(&seconds)
        .unwrap()
        .checked_div(&seconds)
        .unwrap();

    assert_ne!(
        squared_velocity.dimension,
        Unit::parse("Gy").unwrap().dimension,
        "the SI decomposition of the gray would make a dose comparable with a velocity squared"
    );
}

#[test]
fn a_ratio_unit_may_not_be_composed() {
    let error = Unit::parse("mg/kg")
        .unwrap()
        .checked_mul(&Unit::parse("kg").unwrap())
        .unwrap_err();

    assert!(matches!(error, UnitError::NonCompositional { .. }));
}

#[test]
fn counted_entities_are_not_moles() {
    let cells = Quantity::parse(4.0e6, "cell").unwrap();
    let substance = Quantity::parse(1.0, "mmol").unwrap();

    assert!(matches!(
        cells.add(&substance).unwrap_err(),
        UnitError::DimensionMismatch { .. }
    ));
}

#[test]
fn two_positions_in_unstated_frames_are_not_comparable() {
    let left = Position::unstated([62.0, -18.0, 31.0], mm());
    let right = Position::unstated([62.0, -18.0, 31.0], mm());

    let error = left.comparable_with(&right).unwrap_err();

    assert!(
        matches!(error, Incomparability::UnstatedFrame { .. }),
        "identical numbers in unrecorded frames are not evidence of agreement"
    );
}

#[test]
fn an_unstated_frame_is_not_comparable_even_with_itself() {
    let position = Position::unstated([0.0, 0.0, 0.0], mm());

    assert!(position.comparable_with(&position).is_err());
}

#[test]
fn ras_and_lps_are_both_right_handed() {
    assert_eq!(Orientation::RAS.handedness(), Handedness::Right);
    assert_eq!(Orientation::LPS.handedness(), Handedness::Right);
}

#[test]
fn ras_and_lps_disagree_on_two_anatomical_axes() {
    let disagreements = Orientation::RAS.disagreeing_axes(&Orientation::LPS);

    assert_eq!(
        disagreements.len(),
        2,
        "handedness agrees while left-right and anterior-posterior do not"
    );
}

#[test]
fn rai_is_left_handed() {
    assert_eq!(
        Orientation::parse("RAI").unwrap().handedness(),
        Handedness::Left
    );
}

#[test]
fn positions_in_ras_and_lps_are_not_comparable() {
    let left = Position::new([62.0, -18.0, 31.0], mm(), FrameBinding::Declared(ras_frame()));
    let right = Position::new([62.0, -18.0, 31.0], mm(), FrameBinding::Declared(lps_frame()));

    let error = left.comparable_with(&right).unwrap_err();

    assert!(matches!(error, Incomparability::OrientationMismatch { .. }));
}

#[test]
fn an_orientation_naming_one_axis_twice_is_rejected() {
    let error = Orientation::parse("RAR").unwrap_err();

    assert!(matches!(
        error,
        bioprism_standards::FrameError::DegenerateOrientation { .. }
    ));
}

#[test]
fn an_orientation_letter_outside_the_six_directions_is_rejected() {
    assert!(Orientation::parse("RAX").is_err());
    assert!(Orientation::parse("RA").is_err());
}

#[test]
fn voxel_indices_are_not_comparable_with_world_coordinates() {
    let voxel = Position::new(
        [60.0, 60.0, 40.0],
        mm(),
        FrameBinding::Declared(Frame::new(
            "grid",
            CoordinateSpace::Voxel {
                grid: "t1-1mm".to_string(),
            },
            Orientation::RAS,
            mni(),
        )),
    );
    let world = Position::new([60.0, 60.0, 40.0], mm(), FrameBinding::Declared(ras_frame()));

    assert!(matches!(
        voxel.comparable_with(&world).unwrap_err(),
        Incomparability::SpaceMismatch { .. }
    ));
}

#[test]
fn two_voxel_grids_are_two_spaces() {
    let grid = |name: &str| {
        Frame::new(
            name,
            CoordinateSpace::Voxel {
                grid: name.to_string(),
            },
            Orientation::RAS,
            mni(),
        )
    };
    let coarse = Position::new([60.0, 0.0, 0.0], mm(), FrameBinding::Declared(grid("t1-1mm")));
    let fine = Position::new([60.0, 0.0, 0.0], mm(), FrameBinding::Declared(grid("t1-0.5mm")));

    assert!(coarse.comparable_with(&fine).is_err());
}

#[test]
fn two_subjects_native_spaces_are_different_frames() {
    let native = |subject: &str| {
        Frame::world(
            "native",
            Orientation::RAS,
            ReferenceSpace::SubjectNative {
                subject: subject.to_string(),
            },
        )
    };
    let left = Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(native("P01")));
    let right = Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(native("P02")));

    assert!(matches!(
        left.comparable_with(&right).unwrap_err(),
        Incomparability::FrameMismatch { .. }
    ));
}

#[test]
fn a_length_is_frame_independent_but_a_position_is_not() {
    let diameter = Extent::new(Quantity::parse(24.0, "mm").unwrap()).unwrap();
    let other_diameter = Extent::new(Quantity::parse(31.0, "mm").unwrap()).unwrap();
    let in_ras = Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(ras_frame()));
    let in_lps = Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(lps_frame()));

    assert!(diameter.comparable_with(&other_diameter).is_ok());
    assert!(in_ras.comparable_with(&in_lps).is_err());
}

#[test]
fn an_extent_must_be_a_spatial_magnitude() {
    let error = Extent::new(Quantity::parse(70.0, "kg").unwrap()).unwrap_err();

    assert!(matches!(error, Incomparability::DimensionMismatch { .. }));
}

#[test]
fn a_locus_without_a_build_is_not_a_location() {
    let left = GenomicPosition::new(
        BuildBinding::Unstated,
        "chr7",
        140_753_336,
        CoordinateConvention::OneBasedInclusive,
    );

    let error = left.comparable_with(&left).unwrap_err();

    assert!(matches!(error, Incomparability::UnstatedBuild { .. }));
    assert!(error.is_silence());
}

#[test]
fn hg19_and_grch37_are_not_the_same_build() {
    let locus = |build: GenomeBuild| {
        GenomicPosition::new(
            BuildBinding::Declared(build),
            "chrM",
            8993,
            CoordinateConvention::OneBasedInclusive,
        )
    };

    let error = locus(GenomeBuild::Hg19)
        .comparable_with(&locus(GenomeBuild::Grch37))
        .unwrap_err();

    assert!(
        matches!(error, Incomparability::BuildMismatch { .. }),
        "the assemblies differ in mitochondrial sequence and contig naming"
    );
}

#[test]
fn an_unrecognised_assembly_name_is_kept_verbatim() {
    let build = GenomeBuild::parse("GRCh38.p13");

    assert_eq!(build.label(), "GRCh38.p13");
    assert_ne!(build, GenomeBuild::Grch38);
}

#[test]
fn the_same_integers_span_different_lengths_under_different_conventions() {
    let interval = |convention| {
        GenomicInterval::new(
            BuildBinding::Declared(GenomeBuild::Grch38),
            "chr7",
            1000,
            1010,
            convention,
        )
    };

    assert_eq!(interval(CoordinateConvention::ZeroBasedHalfOpen).length(), 10);
    assert_eq!(interval(CoordinateConvention::OneBasedInclusive).length(), 11);
}

#[test]
fn a_vcf_locus_and_a_bed_locus_are_not_comparable() {
    let locus = |convention| {
        GenomicPosition::new(
            BuildBinding::Declared(GenomeBuild::Grch38),
            "chr7",
            140_753_336,
            convention,
        )
    };

    let error = locus(CoordinateConvention::OneBasedInclusive)
        .comparable_with(&locus(CoordinateConvention::ZeroBasedHalfOpen))
        .unwrap_err();

    assert!(matches!(error, Incomparability::ConventionMismatch { .. }));
}

#[test]
fn a_build_transport_that_declares_no_loss_is_rejected() {
    let transport = BuildTransport::new(GenomeBuild::Grch37, GenomeBuild::Grch38, "UCSC liftOver");

    assert!(matches!(
        transport.check().unwrap_err(),
        ReferenceError::UndeclaredLoss { .. }
    ));
}

fn declared_transport() -> BuildTransport {
    BuildTransport::new(GenomeBuild::Grch37, GenomeBuild::Grch38, "UCSC liftOver").declaring(
        bioprism_scope::LossLedger::default()
            .discarding("regions absent from the target assembly")
            .adding_uncertainty("chain-file alignment ambiguity"),
    )
}

#[test]
fn a_declared_lift_produces_a_locus_on_the_target_build() {
    let source = GenomicPosition::new(
        BuildBinding::Declared(GenomeBuild::Grch37),
        "chr7",
        140_453_136,
        CoordinateConvention::OneBasedInclusive,
    );

    let outcome = declared_transport()
        .apply(&source, Some(("chr7".to_string(), 140_753_336)), None)
        .unwrap();

    let LiftOutcome::Mapped { position, record } = outcome else {
        panic!("expected a mapped outcome");
    };
    assert_eq!(
        position.build,
        BuildBinding::Declared(GenomeBuild::Grch38),
        "the lifted coordinate is on the target build, not the source one"
    );
    assert_eq!(position.convention, source.convention);
    assert!(!record.loss.is_empty());
}

#[test]
fn an_unmapped_lift_must_state_a_reason() {
    let source = GenomicPosition::new(
        BuildBinding::Declared(GenomeBuild::Grch37),
        "chr7",
        140_453_136,
        CoordinateConvention::OneBasedInclusive,
    );

    let error = declared_transport().apply(&source, None, None).unwrap_err();

    assert!(matches!(error, ReferenceError::UnmappedWithoutReason { .. }));

    let outcome = declared_transport()
        .apply(&source, None, Some("no chain alignment"))
        .unwrap();
    assert!(matches!(outcome, LiftOutcome::Unmapped { .. }));
}

#[test]
fn lifting_a_coordinate_from_the_wrong_source_build_is_refused() {
    let source = GenomicPosition::new(
        BuildBinding::Declared(GenomeBuild::T2tChm13),
        "chr7",
        1,
        CoordinateConvention::OneBasedInclusive,
    );

    assert!(matches!(
        declared_transport()
            .apply(&source, Some(("chr7".to_string(), 2)), None)
            .unwrap_err(),
        ReferenceError::TransportSourceMismatch { .. }
    ));
}

#[test]
fn a_build_lift_is_a_sound_scope_transport() {
    let mapping = declared_transport().to_scope_mapping();

    assert_eq!(declared_transport().scope_check(), MappingCheck::Sound);
    assert_eq!(mapping.from.len(), 1);
    assert!(mapping.from.dimensions().any(|d| d == "genome_build"));
}

#[test]
fn a_lift_with_no_ledger_is_an_undeclared_loss_transport_in_scope_terms() {
    let bare = BuildTransport::new(GenomeBuild::Grch37, GenomeBuild::Grch38, "unknown tool");

    assert_eq!(bare.scope_check(), MappingCheck::UndeclaredLoss);
}

#[test]
fn an_exact_binding_still_carries_the_local_term() {
    let binding = TermBinding::exact(
        "Glioblastoma multiforme, WHO grade IV",
        mondo_gbm("2026-06-04"),
    )
    .unwrap()
    .from_vocabulary("site pathology dictionary");

    assert_eq!(
        binding.local_term(),
        "Glioblastoma multiforme, WHO grade IV",
        "binding a code must not replace the sentence a pathologist wrote"
    );
    assert_eq!(binding.source_vocabulary(), Some("site pathology dictionary"));
    assert_eq!(binding.resolve().unwrap().curie(), "MONDO:0018177");
}

#[test]
fn an_unmapped_term_is_recorded_not_dropped() {
    let mut catalog = TermCatalog::new();
    catalog
        .bind(TermBinding::unmapped("gliomatosis cerebri", "no current WHO entity").unwrap())
        .unwrap();

    assert_eq!(catalog.len(), 1);
    let unmapped: Vec<&str> = catalog.unmapped().map(|b| b.local_term()).collect();
    assert_eq!(unmapped, vec!["gliomatosis cerebri"]);
}

#[test]
fn an_ambiguous_term_never_resolves_to_a_nearest_match() {
    let binding = TermBinding::ambiguous(
        "anaplastic astrocytoma",
        vec![mondo_gbm("2026-06-04"), OntologyId::parse("MONDO:0006107", "2026-06-04").unwrap()],
    )
    .unwrap();

    assert!(matches!(
        binding.resolve().unwrap_err(),
        OntologyError::NotResolvable { .. }
    ));
}

#[test]
fn an_identifier_without_a_release_is_rejected() {
    assert!(matches!(
        OntologyId::parse("MONDO:0018177", "  ").unwrap_err(),
        OntologyError::MissingRelease { .. }
    ));
}

#[test]
fn a_malformed_curie_is_rejected() {
    for bad in ["MONDO0018177", "MONDO: 0018177", ":0018177", "MONDO:", "9MONDO:1"] {
        assert!(
            OntologyId::parse(bad, "2026-06-04").is_err(),
            "{bad:?} should not parse"
        );
    }
}

#[test]
fn an_empty_local_term_is_refused() {
    assert!(matches!(
        TermBinding::exact("   ", mondo_gbm("2026-06-04")).unwrap_err(),
        OntologyError::EmptyLocalTerm { .. }
    ));
}

#[test]
fn the_same_curie_at_two_releases_is_version_drift() {
    let older = TermBinding::exact("glioblastoma", mondo_gbm("2024-01-03")).unwrap();
    let newer = TermBinding::exact("glioblastoma", mondo_gbm("2026-06-04")).unwrap();

    let error = older.comparable_with(&newer).unwrap_err();

    assert!(matches!(error, Incomparability::OntologyVersionDrift { .. }));
}

#[test]
fn a_broad_and_a_narrow_binding_are_not_equivalent() {
    let broad = TermBinding::mapped(
        "glioma NOS",
        OntologyId::parse("MONDO:0005070", "2026-06-04").unwrap(),
        MappingPrecision::Broader,
    )
    .unwrap();
    let narrow = TermBinding::exact("glioblastoma", mondo_gbm("2026-06-04")).unwrap();

    assert!(matches!(
        broad.comparable_with(&narrow).unwrap_err(),
        Incomparability::GranularityMismatch { .. }
    ));
}

#[test]
fn different_ontologies_are_a_namespace_mismatch() {
    let mondo = TermBinding::exact("glioblastoma", mondo_gbm("2026-06-04")).unwrap();
    let ncit = TermBinding::exact(
        "glioblastoma",
        OntologyId::parse("NCIt:C3058", "26.03d").unwrap(),
    )
    .unwrap();

    assert!(matches!(
        mondo.comparable_with(&ncit).unwrap_err(),
        Incomparability::NamespaceMismatch { .. }
    ));
}

#[test]
fn rebinding_a_local_term_to_a_different_target_conflicts() {
    let mut catalog = TermCatalog::new();
    catalog
        .bind(TermBinding::exact("glioblastoma", mondo_gbm("2024-01-03")).unwrap())
        .unwrap();

    let error = catalog
        .bind(TermBinding::exact("glioblastoma", mondo_gbm("2026-06-04")).unwrap())
        .unwrap_err();

    assert!(
        matches!(error, OntologyError::ConflictingBinding { .. }),
        "an overwrite is version drift performed in memory"
    );
}

#[test]
fn a_catalog_reports_terms_that_merge_onto_one_identifier() {
    let mut catalog = TermCatalog::new();
    for local in ["GBM", "Glioblastoma multiforme"] {
        catalog
            .bind(TermBinding::exact(local, mondo_gbm("2026-06-04")).unwrap())
            .unwrap();
    }

    let merges = catalog.merged_targets();

    assert_eq!(merges.len(), 1);
    assert_eq!(merges[0].0, "MONDO:0018177");
    assert_eq!(merges[0].1.len(), 2);
}

#[test]
fn a_catalog_digest_ignores_insertion_order() {
    let build = |order: [&str; 2]| {
        let mut catalog = TermCatalog::new();
        for local in order {
            catalog
                .bind(TermBinding::exact(local, mondo_gbm("2026-06-04")).unwrap())
                .unwrap();
        }
        catalog.digest().unwrap()
    };

    assert_eq!(build(["a", "b"]), build(["b", "a"]));
}

#[test]
fn a_scalar_and_a_locus_are_different_kinds_of_thing() {
    let volume = Measurement::scalar("tumour volume", Quantity::parse(12.5, "mL").unwrap());
    let locus = Measurement::locus(
        "BRAF V600E",
        GenomicPosition::new(
            BuildBinding::Declared(GenomeBuild::Grch38),
            "chr7",
            140_753_336,
            CoordinateConvention::OneBasedInclusive,
        ),
    );

    assert!(matches!(
        comparable(&volume, &locus).unwrap_err(),
        Incomparability::KindMismatch { .. }
    ));
}

#[test]
fn the_frame_blocks_before_the_unit_does() {
    let left = Measurement::located(
        "centroid A",
        Position::new([6.2, -1.8, 3.1], Unit::parse("cm").unwrap(), FrameBinding::Declared(ras_frame())),
    );
    let right = Measurement::located(
        "centroid B",
        Position::new([62.0, -18.0, 31.0], mm(), FrameBinding::Declared(lps_frame())),
    );

    let error = comparable(&left, &right).unwrap_err();

    assert!(
        matches!(error, Incomparability::OrientationMismatch { .. }),
        "an agreeing number in a disagreeing frame is not evidence; the unit is the lesser problem"
    );
}

#[test]
fn an_unstated_frame_blocks_before_the_ontology_binding() {
    let term = TermBinding::exact("tumour centroid", mondo_gbm("2026-06-04")).unwrap();
    let unbound = Measurement::located("centroid A", Position::unstated([1.0, 2.0, 3.0], mm()));
    let bound = Measurement::located("centroid B", Position::unstated([1.0, 2.0, 3.0], mm()))
        .of(term);

    assert!(matches!(
        comparable(&unbound, &bound).unwrap_err(),
        Incomparability::UnstatedFrame { .. }
    ));
}

#[test]
fn reconcile_records_the_conversion_it_performed() {
    let left = Measurement::scalar("volume A", Quantity::parse(12.5, "mL").unwrap());
    let right = Measurement::scalar("volume B", Quantity::parse(9000.0, "mm3").unwrap());

    assert!(matches!(
        comparable(&left, &right).unwrap_err(),
        Incomparability::ConversionRequired { .. }
    ));

    let records = reconcile(&left, &right, ComparabilityPolicy::default()).unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].from, "mm3");
    assert_eq!(records[0].to, "mL");
}

#[test]
fn reconcile_refuses_to_invent_a_registration() {
    let left = Measurement::located(
        "centroid A",
        Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(ras_frame())),
    );
    let right = Measurement::located(
        "centroid B",
        Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(lps_frame())),
    );

    assert!(reconcile(&left, &right, ComparabilityPolicy::default()).is_err());
}

#[test]
fn asymmetric_ontology_binding_is_blocked() {
    let bound = Measurement::scalar("volume A", Quantity::parse(12.5, "mL").unwrap())
        .of(TermBinding::exact("tumour volume", mondo_gbm("2026-06-04")).unwrap());
    let unbound = Measurement::scalar("volume B", Quantity::parse(12.5, "mL").unwrap());

    assert!(matches!(
        comparable(&bound, &unbound).unwrap_err(),
        Incomparability::UnboundTerm { .. }
    ));
}

#[test]
fn two_unbound_measurements_pass_by_default_and_fail_under_a_strict_policy() {
    let left = Measurement::scalar("volume A", Quantity::parse(12.5, "mL").unwrap());
    let right = Measurement::scalar("volume B", Quantity::parse(9.1, "mL").unwrap());

    assert!(comparable(&left, &right).is_ok());
    assert!(matches!(
        comparable_under(&left, &right, ComparabilityPolicy::strict()).unwrap_err(),
        Incomparability::UnboundTerm { .. }
    ));
}

#[test]
fn comparability_refusals_are_symmetric() {
    let left = Measurement::located(
        "centroid A",
        Position::new([1.0, 2.0, 3.0], mm(), FrameBinding::Declared(ras_frame())),
    );
    let right = Measurement::located("centroid B", Position::unstated([1.0, 2.0, 3.0], mm()));

    assert_eq!(
        comparable(&left, &right).is_err(),
        comparable(&right, &left).is_err()
    );
}

#[test]
fn a_report_flags_a_conventional_conversion_as_a_caveat() {
    let left = Measurement::scalar("survival A", Quantity::parse(14.6, "month").unwrap());
    let right = Measurement::scalar("survival B", Quantity::parse(430.0, "day").unwrap());

    let outcome = report(&left, &right, ComparabilityPolicy::default());

    assert!(outcome.verdict.is_comparable());
    assert_eq!(outcome.conversions.len(), 1);
    assert!(outcome
        .caveats
        .iter()
        .any(|c| c.contains("conventional, not exact")));
}

#[test]
fn a_report_says_when_neither_side_is_bound_to_an_ontology() {
    let left = Measurement::scalar("volume A", Quantity::parse(12.5, "mL").unwrap());
    let right = Measurement::scalar("volume B", Quantity::parse(9.1, "mL").unwrap());

    let outcome = report(&left, &right, ComparabilityPolicy::default());

    assert!(outcome.verdict.is_comparable());
    assert!(outcome.caveats.iter().any(|c| c.contains("local labels")));
}

#[test]
fn a_blocked_report_carries_the_typed_reason_and_its_scope_class() {
    let left = Measurement::located("centroid A", Position::unstated([1.0, 2.0, 3.0], mm()));
    let right = Measurement::located("centroid B", Position::unstated([1.0, 2.0, 3.0], mm()));

    let outcome = report(&left, &right, ComparabilityPolicy::default());

    let Verdict::Blocked { reason } = &outcome.verdict else {
        panic!("expected a blocked verdict");
    };
    assert_eq!(reason.blocking_class(), ScopeClass::Coordinate);
    assert!(outcome.digest().is_ok());
}

#[test]
fn every_refusal_belongs_to_the_coordinate_or_ontology_scope_class() {
    let refusals = [
        Incomparability::UnstatedFrame {
            side: "left".to_string(),
        },
        Incomparability::UnstatedBuild {
            side: "left".to_string(),
        },
        Incomparability::ConventionMismatch {
            left: "a".to_string(),
            right: "b".to_string(),
        },
        Incomparability::ConversionRequired {
            left_unit: "mm".to_string(),
            right_unit: "cm".to_string(),
        },
        Incomparability::UnmappedTerm {
            local_term: "x".to_string(),
            reason: "y".to_string(),
        },
        Incomparability::OntologyVersionDrift {
            curie: "MONDO:1".to_string(),
            left_release: "a".to_string(),
            right_release: "b".to_string(),
        },
    ];

    for refusal in refusals {
        assert!(matches!(
            refusal.blocking_class(),
            ScopeClass::Coordinate | ScopeClass::Ontology
        ));
    }
}

#[test]
fn missing_declarations_are_distinguished_from_stated_disagreements() {
    let silence = Incomparability::UnstatedBuild {
        side: "left".to_string(),
    };
    let disagreement = Incomparability::BuildMismatch {
        left: "GRCh37".to_string(),
        right: "GRCh38".to_string(),
    };

    assert!(silence.is_silence());
    assert!(!disagreement.is_silence());
}

#[test]
fn a_refusal_round_trips_through_json() {
    let refusal = Incomparability::BuildMismatch {
        left: "GRCh37".to_string(),
        right: "GRCh38".to_string(),
    };

    let encoded = serde_json::to_string(&refusal).unwrap();
    let decoded: Incomparability = serde_json::from_str(&encoded).unwrap();

    assert_eq!(refusal, decoded);
    assert!(encoded.contains("build_mismatch"));
}

#[test]
fn the_check_order_puts_location_before_magnitude() {
    assert!(check_order_index("coordinate frame or reference build") < check_order_index("unit identity"));
    assert!(check_order_index("observable kind") < check_order_index("physical dimension"));
    assert!(check_order_index("unit identity") < check_order_index("ontology binding"));
}

#[test]
fn two_fully_declared_positions_in_one_frame_are_comparable() {
    let left = Position::new([62.0, -18.0, 31.0], mm(), FrameBinding::Declared(ras_frame()));
    let right = Position::new([58.0, -12.0, 29.0], mm(), FrameBinding::Declared(ras_frame()));

    assert!(
        left.comparable_with(&right).is_ok(),
        "the crate must say yes when everything is declared, or it says nothing at all"
    );
}

#[test]
fn two_loci_on_one_build_and_convention_are_comparable() {
    let locus = |position| {
        GenomicPosition::new(
            BuildBinding::Declared(GenomeBuild::Grch38),
            "chr7",
            position,
            CoordinateConvention::OneBasedInclusive,
        )
    };

    assert!(locus(140_753_336).comparable_with(&locus(140_753_400)).is_ok());
}

#[test]
fn two_exact_bindings_at_one_release_are_comparable() {
    let site_a = TermBinding::exact("GBM", mondo_gbm("2026-06-04")).unwrap();
    let site_b = TermBinding::exact(
        "Glioblastoma, IDH-wildtype",
        mondo_gbm("2026-06-04"),
    )
    .unwrap();

    assert!(site_a.comparable_with(&site_b).is_ok());
    assert_ne!(
        site_a.local_term(),
        site_b.local_term(),
        "agreeing on a code does not merge the two local terms"
    );
}

#[test]
fn quantities_in_one_unit_subtract_and_order() {
    let baseline = Quantity::parse(24.0, "mm").unwrap();
    let follow_up = Quantity::parse(31.0, "mm").unwrap();

    let change = follow_up.sub(&baseline).unwrap();

    assert_eq!(change.unit.symbol, "mm");
    assert!((change.value - 7.0).abs() < 1e-12);
    assert_eq!(
        follow_up.compare(&baseline).unwrap(),
        std::cmp::Ordering::Greater
    );
}

#[test]
fn dimensions_compose_and_render() {
    let volume = Dimension::LENGTH.checked_pow(3).unwrap();

    assert_eq!(volume, Dimension::VOLUME);
    assert_eq!(
        Dimension::VOLUME.checked_div(Dimension::AREA).unwrap(),
        Dimension::LENGTH
    );
    assert_eq!(Dimension::DIMENSIONLESS.to_string(), "1");
    assert_eq!(Dimension::MASS_PER_AREA.to_string(), "L^-2*M");
}

#[test]
fn a_frame_is_identified_by_what_it_declares_not_by_its_name() {
    let renamed = Frame::world("some-other-name", Orientation::RAS, mni());

    assert!(ras_frame().agrees_with(&renamed).is_ok());
    assert!(ras_frame().agrees_with(&lps_frame()).is_err());
}

#[test]
fn a_measurement_round_trips_through_json() {
    let measurement = Measurement::located(
        "tumour centroid",
        Position::new([62.0, -18.0, 31.0], mm(), FrameBinding::Declared(ras_frame())),
    )
    .of(TermBinding::exact("glioblastoma", mondo_gbm("2026-06-04")).unwrap());

    let encoded = serde_json::to_string(&measurement).unwrap();
    let decoded: Measurement = serde_json::from_str(&encoded).unwrap();

    assert_eq!(measurement, decoded);
    assert!(
        encoded.contains("MONDO") && encoded.contains("0018177"),
        "the serialized form keeps the external identifier"
    );
    assert!(
        encoded.contains("glioblastoma") && encoded.contains("2026-06-04"),
        "and keeps the local term and the ontology release beside it"
    );
}

#[test]
fn the_unit_table_covers_the_clinical_vocabulary() {
    let symbols: Vec<&str> = Unit::known_symbols().collect();

    for required in ["mm", "mm2", "mm3", "mL", "mg/m2", "Gy", "month"] {
        assert!(symbols.contains(&required), "{required} should be known");
    }
}

#[test]
fn a_catalog_separates_ambiguous_terms_from_unmapped_ones() {
    let mut catalog = TermCatalog::new();
    catalog
        .bind(TermBinding::unmapped("gliomatosis cerebri", "no current entity").unwrap())
        .unwrap();
    catalog
        .bind(
            TermBinding::ambiguous(
                "anaplastic astrocytoma",
                vec![mondo_gbm("2026-06-04"), OntologyId::parse("MONDO:0006107", "2026-06-04").unwrap()],
            )
            .unwrap(),
        )
        .unwrap();

    assert_eq!(catalog.unmapped().count(), 1);
    assert_eq!(catalog.ambiguous().count(), 1);
    assert_eq!(catalog.len(), 2, "neither is dropped");
}

fn check_order_index(name: &str) -> usize {
    bioprism_standards::CHECK_ORDER
        .iter()
        .position(|entry| *entry == name)
        .expect("named check is in the documented order")
}
