from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAIN_NAMES,
    AutonomousAgent,
    AutonomousActionAdmissionPersistenceCoordinator,
    AutonomousActionAdmissionController,
    InMemoryAutonomousActionAdmissionLedger,
    JsonAutonomousActionAdmissionSnapshotPersistence,
    LLMRuntime,
    TransactionalJsonAutonomousActionAdmissionSnapshotPersistence,
    admit_autonomous_action_plan,
    create_autonomous_action_admission_record,
    validate_autonomous_action_admission_record,
)
from prism_sdk.errors import ArgumentError


_TASKS = {
    "coding": "debug a bounded repository change",
    "browser": "compare web sources and citation gaps",
    "data": "profile a dataset schema and missingness",
    "science": "design a reproducible experiment and uncertainty report",
    "biomedical": "review biomedical evidence with safety boundaries",
    "neuroscience": "analyze neural signal preprocessing and limitations",
    "operations": "prepare a reversible incident rollback runbook",
    "enterprise": "map governance ownership and approvals",
    "multi_agent": "delegate specialists and reconcile evidence",
    "multimodal": "align document image and audio observations",
    "cross_domain": "synthesize evidence across several disciplines",
    "evaluation": "replay a benchmark and analyze evaluator failures",
}


class _MemoryTextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value


class _TransactionalMemoryTextStore(_MemoryTextStore):
    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        if expected_snapshot_digest is None and self.value is not None:
            return False
        if expected_snapshot_digest is not None:
            if self.value is None or json.loads(self.value)["snapshot_digest"] != expected_snapshot_digest:
                return False
        self.value = value
        return True


def _agent() -> AutonomousAgent:
    return AutonomousAgent(object(), LLMRuntime())


def _all_approvals(plan: dict[str, object]) -> dict[str, bool]:
    return {approval: True for approval in plan["required_approvals"]}  # type: ignore[index]


def test_action_admission_ledger_covers_every_domain_and_cross_domain_metadata() -> None:
    agent = _agent()
    ledger = InMemoryAutonomousActionAdmissionLedger(max_records=32)
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        task = _TASKS[domain]
        plan = agent.action_plan(task=task, domain=domain, allow_cross_domain=False)
        admission = admit_autonomous_action_plan(plan)
        record = ledger.submit(plan, admission, action_id=f"domain-{domain}")
        assert record["status"] in {"pending_review", "blocked"}, domain
        assert task not in json.dumps(record)
        assert record["plan"]["plan_digest"] == record["admission"]["plan_digest"]

    cross_plan = agent.action_plan(
        task="coordinate coding and biomedical evidence",
        hints=("coding", "biomedical"),
        allow_cross_domain=True,
    )
    cross_admission = admit_autonomous_action_plan(cross_plan)
    cross = ledger.submit(cross_plan, cross_admission, action_id="cross-domain-review")
    assert cross["plan"]["cross_domain"] is True
    assert len(cross["plan"]["selected_domains"]) >= 2
    assert len(ledger.list()) == len(AUTONOMOUS_DOMAIN_NAMES) + 1


def test_action_admission_review_revisions_require_operator_identity_and_exact_predecessor() -> None:
    agent = _agent()
    plan = agent.action_plan(task=_TASKS["coding"], domain="coding", allow_cross_domain=False)
    pending = create_autonomous_action_admission_record(
        plan,
        admit_autonomous_action_plan(plan),
        action_id="review-transition",
    )
    ledger = InMemoryAutonomousActionAdmissionLedger()
    ledger.put(pending)
    reviewed = ledger.review(
        "review-transition",
        approvals=_all_approvals(plan),
        reviewed=True,
        reviewer_digest="a" * 64,
        reason="operator reviewed every explicit gate",
        expected_record_digest=pending["record_digest"],
    )
    assert reviewed["revision"] == 2
    assert reviewed["status"] == "admitted"
    assert reviewed["decision"] == "reviewed"
    assert reviewed["previous_record_digest"] == pending["record_digest"]
    with pytest.raises(ArgumentError, match="expected_record_digest"):
        ledger.review(
            "review-transition",
            approvals=_all_approvals(plan),
            reviewed=True,
            reviewer_digest="b" * 64,
            expected_record_digest=pending["record_digest"],
        )
    tampered = dict(reviewed, status="pending_review")
    with pytest.raises(ArgumentError, match="digest|status"):
        validate_autonomous_action_admission_record(tampered)
    assert _TASKS["coding"] not in json.dumps(reviewed)


def test_action_admission_snapshots_are_canonical_restart_safe_cas_fenced_and_tamper_evident() -> None:
    agent = _agent()
    plan = agent.action_plan(task=_TASKS["science"], domain="science", allow_cross_domain=False)
    admission = admit_autonomous_action_plan(plan)
    ledger = InMemoryAutonomousActionAdmissionLedger()
    ledger.submit(plan, admission, action_id="persisted-science")
    text_store = _TransactionalMemoryTextStore()
    persistence = TransactionalJsonAutonomousActionAdmissionSnapshotPersistence(text_store)
    coordinator = AutonomousActionAdmissionPersistenceCoordinator(ledger, persistence)
    assert coordinator.restore() is None
    snapshot = coordinator.flush()
    assert snapshot["generation"] == 1
    assert len(snapshot["records"]) == 1
    assert _TASKS["science"] not in json.dumps(snapshot)

    restored_ledger = InMemoryAutonomousActionAdmissionLedger()
    restored_coordinator = AutonomousActionAdmissionPersistenceCoordinator(restored_ledger, persistence)
    restored = restored_coordinator.restore()
    assert restored is not None
    assert restored["snapshot_digest"] == snapshot["snapshot_digest"]
    assert restored_ledger.get("persisted-science")["record_digest"] == snapshot["records"][0]["record_digest"]

    stale_ledger = InMemoryAutonomousActionAdmissionLedger()
    stale_coordinator = AutonomousActionAdmissionPersistenceCoordinator(stale_ledger, persistence)
    stale_coordinator.restore()
    coordinator.flush()
    with pytest.raises(ArgumentError, match="compare-and-set conflict"):
        stale_coordinator.flush()

    raw = json.loads(text_store.value)
    raw["records"][0]["revision"] = 999
    tampered_store = _MemoryTextStore()
    tampered_store.value = json.dumps(raw, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    tampered_persistence = JsonAutonomousActionAdmissionSnapshotPersistence(tampered_store)
    with pytest.raises(ArgumentError, match="digest|record|metadata"):
        tampered_persistence.read()


def test_operator_controller_projects_every_domain_and_requires_authorized_review_before_handoff() -> None:
    agent = _agent()
    ledger = InMemoryAutonomousActionAdmissionLedger(max_records=32)
    controller = AutonomousActionAdmissionController(ledger)
    for domain in AUTONOMOUS_DOMAIN_NAMES:
        plan = agent.action_plan(task=_TASKS[domain], domain=domain, allow_cross_domain=False)
        ledger.submit(plan, admit_autonomous_action_plan(plan), action_id=f"operator-{domain}")
    queue = controller.queue()
    assert len(queue["rows"]) == len(AUTONOMOUS_DOMAIN_NAMES)
    assert set(queue["domain_counts"]) == set(AUTONOMOUS_DOMAIN_NAMES)
    assert all(len(row["plan_digest"]) == 64 for row in queue["rows"])
    assert _TASKS["coding"] not in json.dumps(queue)

    plan = agent.action_plan(task=_TASKS["data"], domain="data", allow_cross_domain=False)
    pending = ledger.submit(plan, admit_autonomous_action_plan(plan), action_id="operator-approved-data")
    with pytest.raises(ArgumentError, match="not ready"):
        controller.dispatch_handoff("operator-approved-data")
    reviewed = controller.review(
        "operator-approved-data",
        approvals=_all_approvals(plan),
        reviewed=True,
        authorization_digest="c" * 64,
        expected_record_digest=pending["record_digest"],
    )
    assert reviewed["status"] == "admitted"
    with pytest.raises(ArgumentError, match="expected_record_digest"):
        controller.review(
            "operator-approved-data",
            approvals=_all_approvals(plan),
            reviewed=True,
            authorization_digest="d" * 64,
            expected_record_digest=pending["record_digest"],
        )
    handoff = controller.dispatch_handoff("operator-approved-data")
    assert handoff["status"] == "ready_for_downstream_gates"
    assert handoff["requested_domains"] == ["data"]
    assert handoff["plan"]["plan_digest"] == handoff["plan_digest"]
    assert handoff["admission"]["plan_digest"] == handoff["plan_digest"]
    assert "credential_scope" in handoff["downstream_gates"]
    assert _TASKS["data"] not in json.dumps(handoff)

    cross_plan = agent.action_plan(
        task="coordinate coding and biomedical evidence",
        hints=("coding", "biomedical"),
        allow_cross_domain=True,
    )
    cross_admission = admit_autonomous_action_plan(cross_plan, approvals=_all_approvals(cross_plan), reviewed=True)
    ledger.submit(cross_plan, cross_admission, action_id="operator-cross", reviewer_digest="e" * 64)
    cross_handoff = controller.dispatch_handoff("operator-cross")
    assert cross_handoff["cross_domain"] is True
    assert len(cross_handoff["selected_domains"]) >= 2
