use bioprism_neurosurgery::{
    CaseRequest, NeurosurgeryError, NeurosurgicalAgent, NeurosurgicalSession, RealGliomaBundle,
    SessionStatus, ToolCapability, ToolRunStatus,
};

fn synthetic_request() -> CaseRequest {
    serde_json::from_str(include_str!(
        "../../../fixtures/neurosurgery/glioma_synthetic.json"
    ))
    .expect("synthetic contract request parses")
}

fn real_request() -> CaseRequest {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_real_request.json"
    ))
    .expect("real research request parses")
}

fn real_data() -> RealGliomaBundle {
    serde_json::from_str(include_str!(
        "../../../data/neurosurgery/glioma_public_snapshot.json"
    ))
    .expect("real public snapshot parses")
}

fn advance_to_end(
    agent: &NeurosurgicalAgent,
    mut session: NeurosurgicalSession,
    request: &CaseRequest,
    data: Option<&RealGliomaBundle>,
) -> NeurosurgicalSession {
    while session.next_ordinal as usize <= session.route.len() {
        session = agent
            .advance_session(&session, request, data)
            .expect("each route step advances");
    }
    session
}

#[test]
fn session_is_checkpointable_and_finishes_to_the_same_report() {
    let agent = NeurosurgicalAgent::default();
    let request = synthetic_request();
    let started = agent.start_session(&request, None).expect("session starts");
    assert_eq!(started.status, SessionStatus::Planned);
    assert_eq!(started.next_ordinal, 1);
    assert_eq!(started.route.first(), Some(&ToolCapability::SafetyGate));

    let session = advance_to_end(&agent, started, &request, None);
    assert_eq!(session.status, SessionStatus::AwaitingHumanReview);
    assert_eq!(session.events.len(), session.route.len());
    assert_eq!(
        session.events.last().unwrap().status,
        ToolRunStatus::HeldForHumanReview
    );

    let checkpoint: NeurosurgicalSession =
        serde_json::from_value(serde_json::to_value(&session).expect("checkpoint serialises"))
            .expect("checkpoint round-trips");
    let response = agent
        .finish_session(&checkpoint, &request, None)
        .expect("complete session finishes");
    assert_eq!(
        response,
        agent.run(&request).expect("single-shot run works")
    );
}

#[test]
fn session_refuses_tampering_and_request_drift() {
    let agent = NeurosurgicalAgent::default();
    let request = synthetic_request();
    let started = agent.start_session(&request, None).expect("session starts");
    let mut tampered = agent
        .advance_session(&started, &request, None)
        .expect("first step advances");
    tampered.events[0].finding_digest = "a".repeat(64);
    assert!(matches!(
        agent.advance_session(&tampered, &request, None),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));

    let mut changed_request = request.clone();
    changed_request.question.push_str(" changed");
    assert!(matches!(
        agent.advance_session(&started, &changed_request, None),
        Err(NeurosurgeryError::SessionRejected { .. })
    ));
}

#[test]
fn real_data_session_binds_the_bundle_digest_through_finish() {
    let agent = NeurosurgicalAgent::default();
    let request = real_request();
    let data = real_data();
    let started = agent
        .start_session(&request, Some(&data))
        .expect("real-data session starts");
    assert_eq!(
        started.real_data_digest.as_deref(),
        Some(
            data.summary()
                .expect("snapshot validates")
                .bundle_digest
                .as_str()
        )
    );
    let session = advance_to_end(&agent, started, &request, Some(&data));
    let response = agent
        .finish_session(&session, &request, Some(&data))
        .expect("real-data session finishes");
    assert_eq!(response.real_data.unwrap().record_count, 88);
}
