//! Storage architecture: what a store is allowed to promise, and what a read from it is worth.

use bioprism_dataops::{
    parity, reference_local, reference_team, Basis, Coverage, DataClass, Deployment, Epoch,
    Mutability, StoreProfile, TopologyDraft, TopologyError,
};

fn complete_draft() -> TopologyDraft {
    TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "sqlite", Mutability::Mutable).unwrap())
        .with_store(StoreProfile::canonical("cas", "filesystem-cas", Mutability::Immutable).unwrap())
        .with_store(
            StoreProfile::canonical("eventlog", "append-only-segments", Mutability::AppendOnly)
                .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "parquet-duckdb",
                Mutability::Mutable,
                [DataClass::Event],
            )
            .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Metadata],
            )
            .unwrap(),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index")
}

#[test]
fn a_topology_missing_a_data_class_is_refused_rather_than_defaulted() {
    let draft = TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "sqlite", Mutability::Mutable).unwrap())
        .assign(DataClass::Metadata, "catalog");

    let error = draft
        .check(Deployment::Local)
        .expect_err("four classes have no store");

    assert!(matches!(error, TopologyError::ClassUnassigned { .. }));
}

#[test]
fn a_class_assigned_to_a_store_nobody_declared_is_refused() {
    let draft = complete_draft().assign(DataClass::Search, "elsewhere");

    let error = draft
        .check(Deployment::Local)
        .expect_err("the store does not exist");

    assert_eq!(
        error,
        TopologyError::UndeclaredStore {
            class: "search",
            store: "elsewhere".to_string()
        }
    );
}

#[test]
fn two_stores_with_one_name_are_refused_before_anything_else_is_checked() {
    let draft = complete_draft().with_store(
        StoreProfile::canonical("catalog", "postgresql", Mutability::Mutable).unwrap(),
    );

    let error = draft
        .check(Deployment::Local)
        .expect_err("two stores answer to one name");

    assert_eq!(
        error,
        TopologyError::DuplicateStore {
            name: "catalog".to_string()
        }
    );
}

#[test]
fn evidence_in_a_store_that_permits_rewriting_is_refused() {
    let draft = TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "sqlite", Mutability::Mutable).unwrap())
        .with_store(StoreProfile::canonical("blobs", "mutable-fs", Mutability::Mutable).unwrap())
        .with_store(
            StoreProfile::canonical("eventlog", "append-only-segments", Mutability::AppendOnly)
                .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "parquet-duckdb",
                Mutability::Mutable,
                [DataClass::Event],
            )
            .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Metadata],
            )
            .unwrap(),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "blobs")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index");

    let error = draft
        .check(Deployment::Local)
        .expect_err("artifacts are evidence");

    assert_eq!(
        error,
        TopologyError::EvidenceStoreIsMutable {
            class: "artifact",
            store: "blobs".to_string()
        }
    );
}

#[test]
fn a_rebuild_chain_with_no_canonical_bottom_is_refused() {
    let draft = TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "sqlite", Mutability::Mutable).unwrap())
        .with_store(StoreProfile::canonical("cas", "filesystem-cas", Mutability::Immutable).unwrap())
        .with_store(
            StoreProfile::canonical("eventlog", "append-only-segments", Mutability::AppendOnly)
                .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "parquet-duckdb",
                Mutability::Mutable,
                [DataClass::Event],
            )
            .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Analytics],
            )
            .unwrap(),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index");

    let error = draft
        .check(Deployment::Local)
        .expect_err("the search index rebuilds from something rebuildable");

    assert_eq!(
        error,
        TopologyError::RebuildSourceNotCanonical {
            store: "index".to_string(),
            from: "analytics"
        }
    );
}

#[test]
fn a_store_that_claims_to_rebuild_from_its_own_class_is_a_cycle() {
    let draft = TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "sqlite", Mutability::Mutable).unwrap())
        .with_store(StoreProfile::canonical("cas", "filesystem-cas", Mutability::Immutable).unwrap())
        .with_store(
            StoreProfile::canonical("eventlog", "append-only-segments", Mutability::AppendOnly)
                .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "parquet-duckdb",
                Mutability::Mutable,
                [DataClass::Event],
            )
            .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Search],
            )
            .unwrap(),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index");

    let error = draft
        .check(Deployment::Local)
        .expect_err("the index rebuilds from itself");

    assert_eq!(
        error,
        TopologyError::RebuildCycle {
            store: "index".to_string()
        }
    );
}

#[test]
fn a_read_from_a_rebuildable_store_is_derived_and_never_first_hand() {
    let topology = reference_local().expect("the reference local topology checks");

    let canonical = topology.basis_for(DataClass::Metadata, Epoch::new(4), 0);
    let derived = topology.basis_for(DataClass::Search, Epoch::new(4), 0);

    assert!(canonical.is_first_hand());
    assert!(!derived.is_first_hand());
    assert!(matches!(derived, Basis::Derived { lag_epochs: 0, .. }));
}

#[test]
fn the_lag_a_caller_supplies_reaches_the_basis_it_stamps() {
    let topology = reference_local().expect("the reference local topology checks");

    let attested = topology.attest(
        DataClass::Analytics,
        7u64,
        Epoch::new(4),
        11,
        Coverage::Complete { observed: 7 },
    );

    assert!(matches!(
        attested.basis(),
        Basis::Derived { lag_epochs: 11, .. }
    ));
}

#[test]
fn local_and_team_promise_the_same_things_on_different_technologies() {
    let local = reference_local().expect("the reference local topology checks");
    let team = reference_team().expect("the reference team topology checks");

    let report = parity(&local, &team);

    assert!(report.holds());
    assert_eq!(report.compared, DataClass::ALL.len());
    assert_ne!(
        local.store_for(DataClass::Metadata).technology(),
        team.store_for(DataClass::Metadata).technology()
    );
}

#[test]
fn parity_names_the_class_whose_promises_diverged() {
    let local = reference_local().expect("the reference local topology checks");
    let divergent = TopologyDraft::new()
        .with_store(StoreProfile::canonical("catalog", "postgresql", Mutability::Mutable).unwrap())
        .with_store(
            StoreProfile::canonical("cas", "s3-compatible", Mutability::AppendOnly).unwrap(),
        )
        .with_store(
            StoreProfile::canonical("eventlog", "object-store-segments", Mutability::AppendOnly)
                .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "results",
                "analytical-engine",
                Mutability::Mutable,
                [DataClass::Event, DataClass::Metadata],
            )
            .unwrap(),
        )
        .with_store(
            StoreProfile::rebuildable(
                "index",
                "structured-index",
                Mutability::Mutable,
                [DataClass::Metadata, DataClass::Artifact],
            )
            .unwrap(),
        )
        .assign(DataClass::Metadata, "catalog")
        .assign(DataClass::Artifact, "cas")
        .assign(DataClass::Event, "eventlog")
        .assign(DataClass::Analytics, "results")
        .assign(DataClass::Search, "index")
        .check(Deployment::Team)
        .expect("append-only artifacts still preserve evidence");

    let report = parity(&local, &divergent);

    assert!(!report.holds());
    assert_eq!(report.differences.len(), 1);
    assert_eq!(report.differences[0].class, DataClass::Artifact);
}
