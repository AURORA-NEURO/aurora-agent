//! Filesystem, process and service virtualization (blueprint 05.06).
//!
//! The semantics 05.06 actually specifies are an immutable base with copy-on-write deltas, a change
//! journal, a portable state manifest, and services that must be declared before they answer. These
//! tests hold the in-process world to those semantics — which is the level at which a container
//! provider would later have to agree with it.

use bioprism_ids::RunId;
use bioprism_runtime::{
    EffectKind, EffectPolicy, EffectRequest, EffectSource, Host, InProcessWorld, RecordingHost,
    RuntimeError, Sandbox,
};

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn policy() -> EffectPolicy {
    EffectPolicy::evaluation_default()
        .declaring([
            EffectKind::FileRead,
            EffectKind::FileWrite,
            EffectKind::ProcessSpawn,
            EffectKind::ServiceCall,
        ])
        .allowing_path("/work/")
}

fn world() -> InProcessWorld {
    InProcessWorld::new().with_base_file("/work/in.txt", "base-content")
}

#[test]
fn a_write_lands_in_the_overlay_and_the_immutable_base_still_answers_for_untouched_paths() {
    let mut host = RecordingHost::new(run("run-cow"), world(), policy());

    assert_eq!(
        host.read_file("/work/in.txt").expect("allowed").as_deref(),
        Some("base-content")
    );
    host.write_file("/work/in.txt", "overlaid")
        .expect("allowed");
    assert_eq!(
        host.read_file("/work/in.txt").expect("allowed").as_deref(),
        Some("overlaid"),
        "the overlay shadows the base for the paths it covers"
    );

    let (_, world, _, _) = host.into_parts();
    assert_eq!(world.file("/work/in.txt"), Some("overlaid"));
}

#[test]
fn a_missing_file_is_an_answer_rather_than_an_error() {
    let mut host = RecordingHost::new(run("run-missing"), world(), policy());

    assert_eq!(host.read_file("/work/absent.txt").expect("allowed"), None);
    assert_eq!(
        host.tape().len(),
        1,
        "absence is recorded, so a program that branches on it replays"
    );
}

#[test]
fn the_change_journal_separates_a_create_from_an_overwrite() {
    let mut host = RecordingHost::new(run("run-journal"), world(), policy());
    host.write_file("/work/in.txt", "overwritten")
        .expect("allowed");
    host.write_file("/work/new.txt", "created")
        .expect("allowed");

    let (_, world, _, _) = host.into_parts();
    let journal = world.journal();
    assert_eq!(journal.len(), 2);
    assert!(
        journal[0].existed_before,
        "a base file that is written to was overwritten, not created"
    );
    assert!(!journal[1].existed_before);
    assert_eq!(journal[1].path, "/work/new.txt");
    assert_eq!(journal[1].bytes, "created".len() as u64);
}

#[test]
fn the_state_manifest_digests_every_visible_path() {
    let mut host = RecordingHost::new(run("run-manifest"), world(), policy());
    host.write_file("/work/new.txt", "created")
        .expect("allowed");

    let (_, world, _, _) = host.into_parts();
    let manifest = world.state_manifest();
    assert_eq!(manifest.len(), 2);
    assert!(manifest.contains_key("/work/in.txt"));
    assert!(manifest.contains_key("/work/new.txt"));
    assert!(
        manifest["/work/new.txt"].len() == 64,
        "a manifest entry is a content digest, not a size"
    );
}

#[test]
fn a_spawned_process_answers_deterministically_and_is_recorded() {
    let mut host = RecordingHost::new(run("run-spawn"), world(), policy());

    let outcome = host
        .spawn("build", &["--release", "-j2"])
        .expect("declared");
    assert_eq!(outcome.integer("exit_code"), Some(0));
    assert_eq!(outcome.text("stdout"), Some("build --release -j2"));
    assert_eq!(host.tape().len(), 1);
}

#[test]
fn a_service_that_was_never_declared_fails_closed() {
    let mut host = RecordingHost::new(run("run-service"), world(), policy());

    let error = host
        .call_service("registry", "read", serde_json::json!({}))
        .expect_err("an undeclared service has no answer to give");
    match error {
        RuntimeError::SourceFailure { reason, .. } => {
            assert!(reason.contains("registry.read"), "{reason}");
        }
        other => panic!("expected a source failure, got {other}"),
    }
    assert!(host.tape().is_empty());
}

#[test]
fn a_declared_service_answers_from_its_declaration() {
    let world = world().with_service("registry", "read", serde_json::json!({ "version": 3 }));
    let mut host = RecordingHost::new(run("run-service-ok"), world, policy());

    let outcome = host
        .call_service("registry", "read", serde_json::json!({ "key": "a" }))
        .expect("declared");
    assert_eq!(
        outcome.field("response"),
        Some(&serde_json::json!({ "version": 3 }))
    );
}

#[test]
fn the_world_counts_only_the_effects_it_was_actually_asked_to_perform() {
    let mut world = world();

    world
        .perform(&EffectRequest::FileRead {
            path: "/work/in.txt".into(),
        })
        .expect("the base holds it");
    assert_eq!(world.calls(), 1);

    world
        .perform(&EffectRequest::ClockNow)
        .expect("the clock always answers");
    assert_eq!(world.calls(), 2);
}

#[test]
fn virtual_clock_overflow_is_refused_without_saturation() {
    let mut world = InProcessWorld::new().with_clock_start(u64::MAX);
    let error = world
        .perform(&EffectRequest::ClockSleep { millis: 1 })
        .expect_err("virtual time must not silently saturate");
    assert!(matches!(error, RuntimeError::InvariantViolation { .. }));
    assert_eq!(world.task_millis(), u64::MAX);
}

#[test]
fn unbounded_random_byte_requests_are_refused_before_allocation() {
    let mut world = InProcessWorld::new();
    let error = world
        .perform(&EffectRequest::RandomBytes { count: u32::MAX })
        .expect_err("randomness requests must stay within the sandbox bound");
    assert!(matches!(error, RuntimeError::InvariantViolation { .. }));
    assert_eq!(world.calls(), 1);
}
