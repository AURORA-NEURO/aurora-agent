//! The relational catalog constraints, exercised by violating them.

use bioprism_dataops::{
    Actor, AliasName, AuditContext, Basis, Catalog, CatalogError, ContentHash, Epoch, EventTimes,
    MediaType, Namespace, ObjectHeader, ObjectId, OutboxCursor, PublicationId, PublicationRequest,
    RecordTime, RevisionId, Status, ValidTime, Visibility,
};
use std::collections::BTreeSet;

fn namespace(name: &str) -> Namespace {
    Namespace::parse(name).expect("a plain name is a valid namespace")
}

fn object(catalog: &mut Catalog, id: &str, ns: &str, media: &str) -> ObjectId {
    let id = ObjectId::parse(id).expect("a plain name is a valid object id");
    catalog
        .declare_object(ObjectHeader {
            id: id.clone(),
            namespace: namespace(ns),
            media_type: MediaType::parse(media).expect("a plain name is a valid media type"),
        })
        .expect("declaring an object succeeds");
    id
}

fn digest(seed: &str) -> ContentHash {
    ContentHash::of_bytes(seed.as_bytes())
}

fn audit() -> AuditContext {
    AuditContext {
        actor: Actor::new("publisher", "curator").expect("a plain actor"),
        times: EventTimes::published_on_record(
            ValidTime::parse("2026-01-01T00:00:00Z").expect("a valid instant"),
            RecordTime::parse("2026-01-02T00:00:00Z").expect("a valid instant"),
        ),
        reason: "release".to_string(),
        trace: "trace-1".to_string(),
    }
}

fn request(id: &str, revisions: impl IntoIterator<Item = RevisionId>) -> PublicationRequest {
    PublicationRequest {
        id: PublicationId::parse(id).expect("a plain publication id"),
        revisions: revisions.into_iter().collect::<BTreeSet<_>>(),
        at: Epoch::new(1),
    }
}

#[test]
fn the_same_digest_twice_under_one_media_type_names_the_revision_that_already_holds_it() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let first = catalog
        .add_revision(&pack, digest("bytes"), Visibility::Public, Epoch::new(1))
        .expect("the first revision is accepted");

    let error = catalog
        .add_revision(&pack, digest("bytes"), Visibility::Public, Epoch::new(2))
        .expect_err("the same bytes under the same media type are one object");

    match error {
        CatalogError::DuplicateDigest { existing, .. } => {
            assert_eq!(existing, first.to_string())
        }
        other => panic!("expected a duplicate digest, got {other}"),
    }
}

#[test]
fn the_same_digest_under_a_different_media_type_is_a_different_row() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let blob = object(&mut catalog, "blob-a", "lab", "application/octet-stream");
    catalog
        .add_revision(&pack, digest("bytes"), Visibility::Public, Epoch::new(1))
        .expect("the pack revision is accepted");

    catalog
        .add_revision(&blob, digest("bytes"), Visibility::Public, Epoch::new(1))
        .expect("uniqueness is scoped by media type");
}

#[test]
fn an_alias_has_one_active_target_and_keeps_every_previous_one() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let first = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the first revision");
    let second = catalog
        .add_revision(&pack, digest("two"), Visibility::Public, Epoch::new(2))
        .expect("the second revision");
    let scope = namespace("lab");
    let alias = AliasName::parse("latest").expect("a plain alias name");

    catalog
        .set_alias(&scope, &alias, &first, Epoch::new(1))
        .expect("the first binding");
    catalog
        .set_alias(&scope, &alias, &second, Epoch::new(2))
        .expect("the retarget");

    let binding = catalog.alias(&scope, &alias).expect("the alias exists");
    assert_eq!(binding.target(), &second);
    assert_eq!(binding.history().len(), 1);
    assert_eq!(binding.history()[0].0, first);
}

#[test]
fn an_alias_may_not_resolve_into_another_namespace() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");

    let error = catalog
        .set_alias(
            &namespace("other-lab"),
            &AliasName::parse("latest").expect("a plain alias name"),
            &revision,
            Epoch::new(1),
        )
        .expect_err("a slug is unique only within its namespace");

    assert!(matches!(error, CatalogError::AliasCrossesNamespace { .. }));
}

#[test]
fn an_alias_pointing_at_nothing_is_a_dangling_reference() {
    let mut catalog = Catalog::new();
    object(&mut catalog, "pack-a", "lab", "application/pack");

    let error = catalog
        .set_alias(
            &namespace("lab"),
            &AliasName::parse("latest").expect("a plain alias name"),
            &RevisionId::parse("pack-a@9").expect("a plain revision id"),
            Epoch::new(1),
        )
        .expect_err("foreign-key closure");

    assert!(matches!(error, CatalogError::DanglingReference { .. }));
}

#[test]
fn a_lineage_edge_that_closes_a_cycle_is_refused_at_insert() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let one = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("one");
    let two = catalog
        .add_revision(&pack, digest("two"), Visibility::Public, Epoch::new(1))
        .expect("two");
    catalog.add_lineage(&two, &one).expect("two derives from one");

    let error = catalog
        .add_lineage(&one, &two)
        .expect_err("that would close a cycle");

    assert_eq!(
        error,
        CatalogError::LineageCycle {
            child: one.to_string(),
            parent: two.to_string()
        }
    );
}

#[test]
fn a_revision_referenced_by_a_publication_cannot_be_retired() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");
    catalog
        .publish(request("pub-1", [revision.clone()]), &audit())
        .expect("the publication");

    let error = catalog
        .retire(&revision, Epoch::new(2))
        .expect_err("a published revision is not deletable");

    assert_eq!(
        error,
        CatalogError::ReferencedByPublication {
            revision: revision.to_string(),
            publication: "pub-1".to_string()
        }
    );
}

#[test]
fn a_publication_carries_the_whole_lineage_closure_of_what_it_names() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let base = catalog
        .add_revision(&pack, digest("base"), Visibility::Public, Epoch::new(1))
        .expect("base");
    let derived = catalog
        .add_revision(&pack, digest("derived"), Visibility::Public, Epoch::new(1))
        .expect("derived");
    catalog
        .add_lineage(&derived, &base)
        .expect("derived comes from base");

    let receipt = catalog
        .publish(request("pub-1", [derived.clone()]), &audit())
        .expect("the publication");

    assert!(receipt.publication.revisions.contains(&base));
    assert!(receipt.publication.revisions.contains(&derived));
    assert_eq!(receipt.publication.revisions.len(), 2);
}

#[test]
fn a_failed_publication_leaves_the_catalog_byte_identical() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");
    let before = catalog.digest().expect("the catalog digests");

    let error = catalog
        .publish(
            request(
                "pub-1",
                [RevisionId::parse("pack-a@9").expect("a plain revision id")],
            ),
            &audit(),
        )
        .expect_err("that revision does not exist");

    assert!(matches!(error, CatalogError::UnknownRevision { .. }));
    assert_eq!(catalog.digest().expect("the catalog digests"), before);
}

#[test]
fn status_history_is_appended_to_and_never_rewritten() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");
    catalog
        .publish(request("pub-1", [revision.clone()]), &audit())
        .expect("the publication");

    let history = catalog
        .revision(&revision)
        .expect("the revision exists")
        .status_history();

    assert_eq!(history.len(), 2);
    assert_eq!(history[0].status, Status::Draft);
    assert_eq!(history[1].status, Status::Released);
}

#[test]
fn publishing_returns_an_audit_event_the_catalog_did_not_keep() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");

    let receipt = catalog
        .publish(request("pub-1", [revision]), &audit())
        .expect("the publication");

    assert_eq!(receipt.audit.kind.as_str(), "catalog.publication.released");
    assert_eq!(receipt.audit.actor.id, "publisher");
    let serialised = serde_json::to_string(&catalog).expect("the catalog serialises");
    assert!(!serialised.contains("catalog.publication.released"));
}

#[test]
fn a_projection_caught_up_with_the_outbox_is_still_derived() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");
    catalog
        .publish(request("pub-1", [revision]), &audit())
        .expect("the publication");

    let basis = catalog
        .projection_basis(OutboxCursor::new(catalog.outbox_emitted()))
        .expect("a caught-up cursor is legal");

    assert!(!basis.is_first_hand());
    assert_eq!(
        basis,
        Basis::Derived {
            source: "catalog-outbox".to_string(),
            lag_epochs: 0
        }
    );
}

#[test]
fn an_unconsumed_outbox_shows_up_as_lag_on_the_projection() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let revision = catalog
        .add_revision(&pack, digest("one"), Visibility::Public, Epoch::new(1))
        .expect("the revision");
    catalog
        .publish(request("pub-1", [revision]), &audit())
        .expect("the publication");

    let basis = catalog
        .projection_basis(OutboxCursor::default())
        .expect("a cursor at zero is legal");

    assert_eq!(
        basis,
        Basis::Derived {
            source: "catalog-outbox".to_string(),
            lag_epochs: 1
        }
    );
}

#[test]
fn a_cursor_ahead_of_the_outbox_is_refused_rather_than_clamped() {
    let catalog = Catalog::new();

    let error = catalog
        .projection_basis(OutboxCursor::new(3))
        .expect_err("nothing has been emitted");

    assert_eq!(
        error,
        CatalogError::OutboxCursorAhead {
            cursor: 3,
            emitted: 0
        }
    );
}

#[test]
fn a_private_revision_is_invisible_from_another_namespace() {
    let mut catalog = Catalog::new();
    let mine = object(&mut catalog, "pack-a", "lab", "application/pack");
    let theirs = object(&mut catalog, "pack-b", "other-lab", "application/pack");
    catalog
        .add_revision(&mine, digest("one"), Visibility::Private, Epoch::new(1))
        .expect("mine");
    catalog
        .add_revision(&theirs, digest("two"), Visibility::Private, Epoch::new(1))
        .expect("theirs");

    let visible = catalog.visible_to(&namespace("lab"));

    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].object(), &mine);
}

#[test]
fn a_public_revision_is_visible_from_every_namespace() {
    let mut catalog = Catalog::new();
    let theirs = object(&mut catalog, "pack-b", "other-lab", "application/pack");
    catalog
        .add_revision(&theirs, digest("two"), Visibility::Public, Epoch::new(1))
        .expect("theirs");

    assert_eq!(catalog.visible_to(&namespace("lab")).len(), 1);
}

#[test]
fn a_publication_that_depends_on_a_withdrawn_revision_is_refused() {
    let mut catalog = Catalog::new();
    let pack = object(&mut catalog, "pack-a", "lab", "application/pack");
    let base = catalog
        .add_revision(&pack, digest("base"), Visibility::Public, Epoch::new(1))
        .expect("base");
    let derived = catalog
        .add_revision(&pack, digest("derived"), Visibility::Public, Epoch::new(1))
        .expect("derived");
    catalog.add_lineage(&derived, &base).expect("lineage");
    catalog
        .retire(&base, Epoch::new(2))
        .expect("nothing references base yet");

    let error = catalog
        .publish(request("pub-1", [derived]), &audit())
        .expect_err("its lineage is withdrawn");

    assert!(matches!(error, CatalogError::ClosureWithdrawn { .. }));
}
