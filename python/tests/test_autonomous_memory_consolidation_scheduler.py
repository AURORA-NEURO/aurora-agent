from __future__ import annotations

import hashlib
import json
import unittest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousMemoryConsolidationError,
    AutonomousMemoryConsolidator,
    AutonomousMemoryConsolidationScheduler,
    AutonomousMemoryConsolidationSchedulerPersistenceCoordinator,
    JsonAutonomousMemoryConsolidationSchedulerPersistence,
    TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence,
    validate_autonomous_memory_consolidation_scheduler_snapshot,
)


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _observation(episode_id: str, domain: str, *, reward: float = 1.0) -> dict[str, object]:
    return {
        "episode_id": episode_id,
        "lesson_id": "lesson-scheduler",
        "concept_id": "scheduler-lesson",
        "variant_id": "v1",
        "domain": domain,
        "capability": "evidence_review",
        "risk_class": "read_only",
        "evaluator_id": f"evaluator-{episode_id}",
        "evaluator_version": "v1",
        "reward": reward,
        "passed": reward > 0,
        "evidence_digest": _digest(f"evidence-{episode_id}"),
        "lesson_digest": _digest("lesson-scheduler-v1"),
        "decision_digest": _digest(f"decision-{episode_id}"),
        "observed_at": 100.0,
        "transferable": True,
    }


class _CasStore:
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


class AutonomousMemoryConsolidationSchedulerTests(unittest.TestCase):
    def test_priority_worker_loop_and_all_domain_coverage_are_deterministic(self) -> None:
        scheduler = AutonomousMemoryConsolidationScheduler(
            AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0),
            default_max_attempts=2,
            lease_seconds=10.0,
        )
        observations = [_observation(f"domain-{index}", domain) for index, domain in enumerate(AUTONOMOUS_DOMAINS)]
        first = scheduler.submit("all-domains", observations, priority=0.4, submitted_at=10.0)
        replay = scheduler.submit("all-domains", list(observations), priority=0.4, submitted_at=10.0)
        self.assertEqual(first["job_digest"], replay["job_digest"])
        scheduler.submit("high-priority", [_observation("high", "coding")], priority=0.9, submitted_at=99.0)

        claim = scheduler.claim_next("worker-a", now=100.0)
        self.assertEqual(claim.job_id, "high-priority")
        scheduler.complete(claim.job_id, claim.worker_id, claim.lease_digest, _digest("report-high"), now=100.0)
        result = scheduler.run_next("worker-a", now=100.0)
        self.assertEqual(result["status"], "completed")
        self.assertEqual(result["observation_count"], len(observations))

        snapshot = scheduler.snapshot()
        self.assertEqual([row["domain"] for row in snapshot["coverage"]], list(AUTONOMOUS_DOMAINS))
        self.assertEqual([row["observation_count"] for row in snapshot["coverage"]], [2 if domain == "coding" else 1 for domain in AUTONOMOUS_DOMAINS])
        self.assertNotIn("provider_output", json.dumps(snapshot).lower())
        self.assertNotIn("api_key", json.dumps(snapshot).lower())

    def test_expired_leases_fence_old_workers_and_retry_quarantines_failures(self) -> None:
        scheduler = AutonomousMemoryConsolidationScheduler(
            AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0),
            default_max_attempts=2,
            lease_seconds=5.0,
        )
        scheduler.submit("lease", [_observation("lease", "operations")], submitted_at=1.0)
        first = scheduler.claim_next("worker-a", now=10.0)
        second = scheduler.claim_next("worker-b", now=16.0)
        self.assertEqual(second.attempt, 2)
        with self.assertRaises(AutonomousMemoryConsolidationError):
            scheduler.complete(first.job_id, first.worker_id, first.lease_digest, _digest("stale"), now=16.0)
        scheduler.submit("contradiction", [_observation("same", "evaluation"), _observation("same", "evaluation", reward=0.0)], max_attempts=2, submitted_at=20.0)
        first_failure = scheduler.run_next("worker-c", now=20.0)
        self.assertEqual(first_failure["status"], "queued")
        failure = scheduler.run_next("worker-c", now=20.0)
        self.assertEqual(failure["status"], "quarantined")
        self.assertEqual(failure["error_class"], "memory_consolidation_failure")
        self.assertNotIn("contradictory", json.dumps(scheduler.snapshot()).lower())

    def test_snapshot_rehydration_tamper_fencing_and_cas(self) -> None:
        source = AutonomousMemoryConsolidationScheduler(AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0), lease_seconds=10.0)
        source.submit("persist", [_observation("persist", "science")], submitted_at=100.0)
        store = _CasStore()
        coordinator = AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(source, TransactionalJsonAutonomousMemoryConsolidationSchedulerPersistence(store))
        snapshot = coordinator.flush()
        self.assertEqual(validate_autonomous_memory_consolidation_scheduler_snapshot(snapshot)["snapshot_digest"], snapshot["snapshot_digest"])

        restored = AutonomousMemoryConsolidationScheduler(AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0), lease_seconds=10.0)
        restored_coordinator = AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(restored, JsonAutonomousMemoryConsolidationSchedulerPersistence(store))
        self.assertEqual(restored_coordinator.restore()["snapshot_digest"], snapshot["snapshot_digest"])
        self.assertEqual(restored.get("persist")["job_digest"], snapshot["jobs"][0]["job_digest"])

        tampered = json.loads(json.dumps(snapshot))
        tampered["jobs"][0]["observations"][0]["reward"] = 0.25
        with self.assertRaises(AutonomousMemoryConsolidationError):
            validate_autonomous_memory_consolidation_scheduler_snapshot(tampered)
        extra = json.loads(json.dumps(snapshot))
        extra["unexpected"] = True
        with self.assertRaises(AutonomousMemoryConsolidationError):
            validate_autonomous_memory_consolidation_scheduler_snapshot(extra)
        source.submit("second", [_observation("second", "science")], submitted_at=101.0)
        competing = AutonomousMemoryConsolidationScheduler(AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0), lease_seconds=10.0)
        competing_coordinator = AutonomousMemoryConsolidationSchedulerPersistenceCoordinator(competing, JsonAutonomousMemoryConsolidationSchedulerPersistence(store))
        competing_coordinator.restore()
        competing.submit("competing", [_observation("competing", "science")], submitted_at=102.0)
        competing_coordinator.flush()
        with self.assertRaises(AutonomousMemoryConsolidationError):
            coordinator.flush()


if __name__ == "__main__":
    unittest.main()
