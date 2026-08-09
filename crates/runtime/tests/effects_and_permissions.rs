//! Effects, permissions and the secret broker (blueprint 05.08).
//!
//! Two families of claim. First: an effect the plan did not declare, or that sits above the tier
//! the plan permits, never reaches the world — and the refusal is recorded as evidence rather than
//! swallowed. Second: benchmark code receives capability tokens, never credentials, and no secret
//! reaches a serialized artefact.

use bioprism_ids::RunId;
use bioprism_runtime::{
    DecisionOutcome, EffectKind, EffectPolicy, EffectRequest, ExternalActions, Host, InProcessWorld,
    MaterializationPolicy, Network, NetworkMode, Provenance, RecordingHost, RuntimeError, Sandbox,
    SecretBroker, SecretRef,
};
use std::collections::BTreeSet;

fn run(id: &str) -> RunId {
    RunId::parse(id).expect("well-formed run id")
}

fn world() -> InProcessWorld {
    InProcessWorld::new()
        .with_base_file("/work/in.txt", "input")
        .with_fixture("GET", "https://allowed.test/a", "ok")
        .with_fixture("GET", "https://evil.test/a", "gotcha")
        .with_fixture("POST", "https://allowed.test/a", "written")
}

fn hosts(policy: EffectPolicy) -> RecordingHost<InProcessWorld> {
    RecordingHost::new(run("run-policy"), world(), policy)
}

fn allowlist(hosts: [&str; 1]) -> NetworkMode {
    NetworkMode::Allowlist {
        hosts: hosts.iter().map(|h| (*h).to_string()).collect::<BTreeSet<_>>(),
    }
}

#[test]
fn an_undeclared_effect_is_refused_before_it_reaches_the_world() {
    let mut host = hosts(EffectPolicy::evaluation_default().allowing_path("/work/"));

    let error = host
        .read_file("/work/in.txt")
        .expect_err("file_read was never declared");
    assert!(matches!(
        error,
        RuntimeError::UndeclaredEffect {
            kind: EffectKind::FileRead
        }
    ));
    assert_eq!(
        host.source().calls(),
        0,
        "the world must not be asked about a refused effect"
    );
}

#[test]
fn a_refused_effect_leaves_no_trace_on_the_tape() {
    let mut host = hosts(EffectPolicy::evaluation_default().allowing_path("/work/"));
    host.read_file("/work/in.txt").expect_err("undeclared");

    assert!(
        host.tape().is_empty(),
        "the tape records effects, and nothing happened"
    );
}

#[test]
fn every_authorization_verdict_is_journalled_including_the_refusals() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileRead, EffectKind::FileWrite])
        .allowing_path("/work/");
    let mut host = hosts(policy);

    host.read_file("/work/in.txt").expect("declared and allowed");
    host.write_file("/etc/passwd", "nope")
        .expect_err("outside the allowlist");

    let journal = host.journal();
    assert_eq!(journal.len(), 2);
    assert_eq!(journal[0].outcome, DecisionOutcome::Permitted);
    match &journal[1].outcome {
        DecisionOutcome::Denied { reason } => assert!(reason.contains("/etc/passwd"), "{reason}"),
        other => panic!("expected a denial, got {other:?}"),
    }
}

#[test]
fn a_write_outside_the_path_allowlist_is_refused() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileWrite])
        .allowing_path("/work/");
    let mut host = hosts(policy);

    let error = host
        .write_file("/etc/shadow", "nope")
        .expect_err("the path does not start with an allowed prefix");
    assert!(matches!(error, RuntimeError::PathDenied { .. }));
    assert_eq!(host.source().calls(), 0);
}

#[test]
fn a_path_that_traverses_out_of_the_allowlist_is_refused_despite_its_prefix() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileWrite, EffectKind::FileRead])
        .allowing_path("/work/");
    let mut host = hosts(policy);

    for escape in [
        "/work/../etc/shadow",
        "/work/./../../etc/shadow",
        "/work//../etc/shadow",
        "/work/..\\etc\\shadow",
    ] {
        let error = host
            .write_file(escape, "nope")
            .expect_err("a traversal must not be permitted by its prefix");
        assert!(
            matches!(error, RuntimeError::PathDenied { .. }),
            "{escape} was not refused"
        );
    }
    assert_eq!(
        host.source().calls(),
        0,
        "no traversal attempt may reach the world"
    );
}

#[test]
fn a_relative_path_is_refused_rather_than_guessed_at() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::FileRead])
        .allowing_path("work/");
    let mut host = hosts(policy);

    let error = host
        .read_file("work/in.txt")
        .expect_err("a relative path has no single meaning, so it has no permitted one");
    assert!(matches!(error, RuntimeError::PathDenied { .. }));
}

#[test]
fn network_mode_denied_refuses_every_outbound_request() {
    let policy = EffectPolicy::evaluation_default().declaring([EffectKind::NetworkFetch]);
    let mut host = hosts(policy);

    let error = host
        .get_body("https://allowed.test/a")
        .expect_err("the default network mode is no network");
    match error {
        RuntimeError::NetworkDenied { mode, host: target } => {
            assert_eq!(mode, "denied");
            assert_eq!(target, "allowed.test");
        }
        other => panic!("expected a network denial, got {other}"),
    }
}

#[test]
fn an_allowlist_permits_its_host_and_refuses_a_neighbour() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::NetworkFetch])
        .with_network(allowlist(["allowed.test"]));
    let mut host = hosts(policy);

    assert_eq!(host.get_body("https://allowed.test/a").expect("listed"), "ok");
    let error = host
        .get_body("https://evil.test/a")
        .expect_err("not on the list");
    assert!(matches!(error, RuntimeError::NetworkDenied { .. }));
}

#[test]
fn an_allowlist_is_not_fooled_by_userinfo_in_the_url() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::NetworkFetch])
        .with_network(allowlist(["allowed.test"]));
    let mut host = hosts(policy);

    let error = host
        .get_body("https://allowed.test@evil.test/a")
        .expect_err("the authority's host is what comes after the last @");
    match error {
        RuntimeError::NetworkDenied { host: target, .. } => assert_eq!(target, "evil.test"),
        other => panic!("expected a network denial, got {other}"),
    }
}

#[test]
fn an_allowlist_is_not_fooled_by_a_port_or_by_case() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::NetworkFetch])
        .with_network(allowlist(["allowed.test"]));
    let request = EffectRequest::NetworkFetch {
        method: "GET".into(),
        url: "https://ALLOWED.test:8443/a".into(),
    };

    assert_eq!(request.target_host().as_deref(), Some("allowed.test"));
    policy
        .authorize(&request)
        .expect("a port and an uppercase host do not change which host it is");
}

#[test]
fn a_write_method_is_classed_above_a_read_and_refused_by_default() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::NetworkFetch])
        .with_network(allowlist(["allowed.test"]));
    let mut host = hosts(policy);

    let error = host
        .fetch("POST", "https://allowed.test/a")
        .expect_err("a POST may not be a read, so it is not classed as one");
    assert!(matches!(error, RuntimeError::ClassForbidden { .. }));
}

#[test]
fn an_irreversible_effect_is_refused_under_the_evaluation_policy() {
    let policy = EffectPolicy::evaluation_default().declaring([EffectKind::Payment]);
    let mut host = hosts(policy);

    let error = host
        .pay("acct-1", 100_000)
        .expect_err("an evaluation run may not move money");
    assert!(matches!(
        error,
        RuntimeError::IrreversibleRefused {
            kind: EffectKind::Payment
        }
    ));
    assert_eq!(host.source().calls(), 0);
}

#[test]
fn a_simulated_irreversible_effect_is_labelled_and_never_reaches_the_world() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::OutboundMessage])
        .with_materialization(MaterializationPolicy::Simulate);
    let mut host = hosts(policy);

    let outcome = host
        .send_message("email", "someone@example.test", "hello")
        .expect("simulation answers without acting");
    assert_eq!(outcome.field("simulated"), Some(&serde_json::json!(true)));
    assert_eq!(
        host.source().calls(),
        0,
        "a simulated effect is answered by the runtime, not by the world"
    );

    let tape = host.into_tape();
    assert_eq!(tape.simulated_steps(), vec![0]);
    assert_eq!(tape.entries()[0].effect.provenance, Provenance::Simulated);
}

#[test]
fn the_in_process_world_refuses_an_irreversible_effect_even_when_policy_permitted_it() {
    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::Payment])
        .permitting_class(bioprism_runtime::EffectClass::Irreversible);
    let mut host = hosts(policy);

    let error = host
        .pay("acct-1", 1)
        .expect_err("this world has no outside and will not invent one");
    assert!(matches!(error, RuntimeError::SourceFailure { .. }));
}

#[test]
fn a_capability_token_never_carries_the_secret_value() {
    let mut broker = SecretBroker::new().with_secret("registry", "hunter2-super-secret");
    let capability = broker
        .issue("registry", "read", 0, 1_000)
        .expect("the resource is registered");

    let serialized = serde_json::to_string(&capability).expect("capabilities serialize");
    assert!(
        !serialized.contains("hunter2"),
        "a token that carries its secret is a secret, not a token: {serialized}"
    );
    assert_eq!(
        broker
            .redeem(&capability, "read", 0)
            .expect("in scope and unexpired")
            .expose(),
        "hunter2-super-secret"
    );
}

#[test]
fn a_secret_redacts_itself_in_any_rendering_that_is_not_an_explicit_expose() {
    let secret = SecretRef::new("hunter2-super-secret");
    assert_eq!(format!("{secret:?}"), "SecretRef(<redacted>)");
    assert_eq!(format!("{secret}"), "<redacted>");
    assert_eq!(secret.expose(), "hunter2-super-secret");
}

#[test]
fn an_expired_capability_cannot_be_redeemed() {
    let mut broker = SecretBroker::new().with_secret("registry", "value");
    let capability = broker
        .issue("registry", "read", 100, 50)
        .expect("registered");

    broker
        .redeem(&capability, "read", 150)
        .expect("exactly at expiry is still valid");
    let error = broker
        .redeem(&capability, "read", 151)
        .expect_err("one millisecond of task time past expiry is past expiry");
    assert!(matches!(error, RuntimeError::CapabilityExpired { .. }));
}

#[test]
fn a_capability_does_not_cover_an_operation_it_was_not_issued_for() {
    let mut broker = SecretBroker::new().with_secret("registry", "value");
    let capability = broker.issue("registry", "read", 0, 1_000).expect("registered");

    let error = broker
        .redeem(&capability, "write", 0)
        .expect_err("read does not imply write");
    assert!(matches!(error, RuntimeError::OperationNotCovered { .. }));
}

#[test]
fn a_capability_cannot_be_issued_for_a_resource_the_broker_does_not_hold() {
    let mut broker = SecretBroker::new().with_secret("registry", "value");
    let error = broker
        .issue("payments", "write", 0, 1_000)
        .expect_err("a token that cannot be redeemed is worse than none");
    assert!(matches!(error, RuntimeError::UnknownResource { .. }));
}

#[test]
fn cleanup_revokes_every_outstanding_capability() {
    let mut broker = SecretBroker::new().with_secret("registry", "value");
    let first = broker.issue("registry", "read", 0, 1_000).expect("registered");
    let second = broker
        .issue("registry", "write", 0, 1_000)
        .expect("registered");

    broker.revoke_all();

    for capability in [&first, &second] {
        let error = broker
            .redeem(capability, &capability.operation, 0)
            .expect_err("a credential must not outlive its trial");
        assert!(matches!(error, RuntimeError::CapabilityRevoked { .. }));
    }
}

#[test]
fn a_secret_never_reaches_the_tape() {
    let mut broker = SecretBroker::new().with_secret("registry", "hunter2-super-secret");
    let capability = broker.issue("registry", "read", 0, 1_000).expect("registered");

    let policy = EffectPolicy::evaluation_default()
        .declaring([EffectKind::ServiceCall, EffectKind::FileWrite])
        .allowing_path("/work/");
    let mut host = RecordingHost::new(
        run("run-secret"),
        world().with_service("registry", "read", serde_json::json!({ "ok": true })),
        policy,
    );

    host.call_service(
        "registry",
        "read",
        serde_json::json!({ "capability": capability.id }),
    )
    .expect("the tool passes its token, not its secret");

    let json = host.into_tape().to_json().expect("serializes");
    assert!(!json.contains("hunter2"), "the tape leaked a secret: {json}");
    assert!(json.contains("cap-000000"), "the token id is what travels");
}
