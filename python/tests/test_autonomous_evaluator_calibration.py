from __future__ import annotations

import json

import pytest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousEvaluatorCalibrationRegistry,
    AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator,
    DomainEvaluatorRegistry,
    InMemoryAutonomousEvaluatorCalibrationPersistence,
    JsonAutonomousEvaluatorCalibrationPersistence,
    LLMRuntime,
    SQLiteAutonomousEvaluatorCalibrationPersistence,
    admit_autonomous_evaluator_calibration,
    calibrate_autonomous_evaluators,
    replay_autonomous_evaluator_calibration,
    validate_autonomous_evaluator_calibration_report,
)
from prism_sdk.errors import ArgumentError


class _TextStore:
    def __init__(self) -> None:
        self.value: str | None = None

    def read(self) -> str | None:
        return self.value

    def write(self, value: str) -> None:
        self.value = value

    def write_if_unchanged(self, expected_snapshot_digest: str | None, value: str) -> bool:
        observed = None if self.value is None else json.loads(self.value)["snapshot_digest"]
        if observed != expected_snapshot_digest:
            return False
        self.value = value
        return True


def _cases(*, domains=AUTONOMOUS_DOMAINS, holdout_label: int = 1, explicit_splits: bool = True):
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    result = []
    for domain in domains:
        adapter = registry.resolve_for_autonomous_domain(domain)
        evidence = {
            "domain": domain,
            "capability": "calibration-fixture",
            "risk_class": "read_only",
            "signals": {signal: 1.0 for signal in adapter.profile.required_signals},
        }
        for index in range(4):
            row = {
                "case_id": f"{domain}-{index}",
                "domain": domain,
                "evidence": evidence,
                "context": {"domain": domain, "fixture": "calibration"},
                "label": 1 if index < 2 else holdout_label,
            }
            if explicit_splits:
                row["split"] = "calibration" if index < 2 else "holdout"
            result.append(row)
    return result


def test_calibration_covers_all_domains_and_replays_without_retaining_cases():
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    cases = _cases()
    report = calibrate_autonomous_evaluators(
        cases,
        registry=registry,
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
    )

    assert report["status"] == "ready"
    assert report["gate"]["decision"] == "admit_learning"
    assert [row["domain"] for row in report["domains"]] == list(AUTONOMOUS_DOMAINS)
    assert all(row["status"] == "ready" for row in report["domains"])
    encoded = json.dumps(report, sort_keys=True)
    assert "calibration-fixture" not in encoded
    assert "signals" not in encoded
    assert validate_autonomous_evaluator_calibration_report(report) == report

    replay = replay_autonomous_evaluator_calibration(report, cases, registry=registry)
    assert replay["matches"] is True
    assert replay["mismatches"] == []

    for domain in AUTONOMOUS_DOMAINS:
        admission = admit_autonomous_evaluator_calibration(report, domain)
        assert admission["decision"] == "admit_learning"
        assert admission["report_digest"] == report["report_digest"]


def test_calibration_holds_on_bad_holdout_and_reports_missing_coverage():
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    bad = calibrate_autonomous_evaluators(
        _cases(domains=("coding",), holdout_label=0),
        registry=registry,
        domains=("coding",),
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
        max_expected_calibration_error=0.1,
        max_brier_score=0.1,
    )
    assert bad["status"] == "miscalibrated"
    assert bad["gate"]["decision"] == "hold_learning"
    assert admit_autonomous_evaluator_calibration(bad, "coding")["decision"] == "hold_learning"

    missing = calibrate_autonomous_evaluators(
        _cases(domains=("coding",)),
        registry=registry,
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
    )
    assert missing["status"] == "insufficient_coverage"
    assert set(missing["missing_domains"]) == set(AUTONOMOUS_DOMAINS) - {"coding"}


def test_calibration_deterministic_split_and_replay_detect_case_drift():
    registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
    cases = _cases(domains=("coding",), explicit_splits=False)
    report = calibrate_autonomous_evaluators(
        cases,
        registry=registry,
        domains=("coding",),
        holdout_fraction=0.5,
        min_calibration_cases_per_domain=1,
        min_holdout_cases_per_domain=1,
    )
    changed = [dict(case) for case in cases]
    changed[0]["label"] = 0
    replay = replay_autonomous_evaluator_calibration(report, changed, registry=registry)
    assert replay["matches"] is False
    assert "case_set_digest" in replay["mismatches"]


def test_calibration_rejects_secrets_and_tampered_reports():
    with pytest.raises(ArgumentError):
        calibrate_autonomous_evaluators([
            {
                "case_id": "secret",
                "domain": "coding",
                "evidence": {
                    "domain": "coding",
                    "capability": "fixture",
                    "risk_class": "read_only",
                    "signals": {"schema_valid": 1, "tests_passed": 1, "evidence_complete": 1},
                    "api_key": "must-not-cross-boundary",
                },
                "label": 1,
            }
        ], domains=("coding",))

    report = calibrate_autonomous_evaluators(
        _cases(domains=("coding",)),
        domains=("coding",),
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
    )
    tampered = dict(report)
    tampered["status"] = "miscalibrated"
    with pytest.raises(ArgumentError):
        validate_autonomous_evaluator_calibration_report(tampered)


def test_calibration_registry_json_cas_and_sqlite_roundtrip(tmp_path):
    report = calibrate_autonomous_evaluators(
        _cases(domains=("coding",)),
        domains=("coding",),
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
    )
    registry = AutonomousEvaluatorCalibrationRegistry()
    registry.register(report)
    snapshot = registry.snapshot()
    assert registry.get(report["report_digest"])["report_digest"] == report["report_digest"]

    memory = InMemoryAutonomousEvaluatorCalibrationPersistence()
    coordinator = AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(registry, memory)
    assert coordinator.restore()["status"] == "empty"
    flushed = coordinator.flush()
    assert flushed["snapshot_digest"] == memory.read()["snapshot_digest"]
    stale = AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(AutonomousEvaluatorCalibrationRegistry(), memory)
    assert stale.restore()["status"] == "restored"
    registry.register({**report, "report_digest": report["report_digest"]})
    assert len(stale.flush()["reports"]) == 1

    text_store = _TextStore()
    json_persistence = JsonAutonomousEvaluatorCalibrationPersistence(text_store)
    json_persistence.write(snapshot)
    assert json_persistence.read() == snapshot
    assert text_store.value == json.dumps(snapshot, sort_keys=True, separators=(",", ":"))

    sqlite_path = tmp_path / "calibration.sqlite"
    with SQLiteAutonomousEvaluatorCalibrationPersistence(sqlite_path) as sqlite_persistence:
        sqlite_persistence.write(snapshot)
        assert sqlite_persistence.read() == snapshot
        assert sqlite_persistence.write_if_unchanged("0" * 64, snapshot) is False


def test_agent_composes_calibration_registry_restore_flush_and_digest_readiness():
    report = calibrate_autonomous_evaluators(
        _cases(),
        min_calibration_cases_per_domain=2,
        min_holdout_cases_per_domain=2,
    )
    registry = AutonomousEvaluatorCalibrationRegistry()
    persistence = InMemoryAutonomousEvaluatorCalibrationPersistence()
    coordinator = AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(registry, persistence)
    agent = AutonomousAgent(
        object(),
        LLMRuntime(),
        evaluator_calibration_registry=registry,
        evaluator_calibration_persistence=coordinator,
    )

    assert agent.register_evaluator_calibration(report) == report["report_digest"]
    assert agent.evaluator_calibration_report(report["report_digest"]) == report
    assert agent.evaluator_calibration_reports() == [report]
    flushed = agent.flush_evaluator_calibration()
    assert flushed["snapshot_digest"] == persistence.read()["snapshot_digest"]

    restarted_registry = AutonomousEvaluatorCalibrationRegistry()
    restarted_coordinator = AutonomousEvaluatorCalibrationRegistryPersistenceCoordinator(
        restarted_registry,
        persistence,
    )
    restarted = AutonomousAgent(
        object(),
        LLMRuntime(),
        evaluator_calibration_registry=restarted_registry,
        evaluator_calibration_persistence=restarted_coordinator,
    )
    assert restarted.restore_evaluator_calibration()["status"] == "restored"
    readiness = restarted.readiness(calibration_report_digest=report["report_digest"])
    assert readiness["evaluator_calibration"]["report_digest"] == report["report_digest"]
    assert readiness["evaluator_calibration"]["decision"] == "admit_learning"
