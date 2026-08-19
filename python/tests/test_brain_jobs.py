from __future__ import annotations

import hashlib
from tempfile import TemporaryDirectory
import unittest

from prism_sdk.brain import (
    AutonomousBrain,
    BrainEvaluatorDecision,
    BrainLearningCycleResult,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
)
from prism_sdk.evaluators import (
    DomainEvaluatorAdapter,
    DomainEvaluatorRegistry,
    DomainEvaluatorProfile,
    builtin_domain_profiles,
    builtin_autonomous_domain_evaluator_profiles,
)
from prism_sdk.jobs import BrainJobError, BrainJobStore
from prism_sdk.control_plane import BrainApprovalRouter
from prism_sdk.llm_runtime import LLMRuntime


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


class _Clock:
    def __init__(self) -> None:
        self.value = 100.0

    def __call__(self) -> float:
        return self.value

    def advance(self, seconds: float) -> None:
        self.value += seconds


def _job_packet(job_id: str = "job-1") -> dict[str, object]:
    return {
        "job_id": job_id,
        "idempotency_key": "idempotency-" + job_id,
        "spec_digest": _digest("spec-" + job_id),
        "domain": "engineering",
        "capability": "release_audit",
        "risk_class": "release_review",
        "priority": 10,
        "max_attempts": 3,
        "checkpoint": {"phase": "submitted", "caller_owned": True},
    }


class _OutcomeWorkspace:
    def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
        if name != "brain_outcome_record":
            raise AssertionError(name)
        return {
            "ok": True,
            "status": "recorded_evaluator_reward",
            "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
            "learning_evidence": {
                "schema": "bioprism-brain-learning-evidence/0.1",
                "evidence_digest": "f" * 64,
            },
        }


class BrainJobStoreTests(unittest.TestCase):
    def test_idempotency_persistence_and_safe_preflight_lease_recovery(self) -> None:
        clock = _Clock()
        with TemporaryDirectory() as directory:
            path = f"{directory}/jobs.sqlite3"
            with BrainJobStore(path, clock=clock) as store:
                first, receipt = store.submit(_job_packet())
                duplicate, duplicate_receipt = store.submit(_job_packet())
                self.assertEqual(first.job_id, duplicate.job_id)
                self.assertFalse(receipt.idempotent)
                self.assertTrue(duplicate_receipt.idempotent)
                self.assertEqual(receipt.event_digest, duplicate_receipt.event_digest)
                leased = store.claim("job-1", "worker-a", lease_seconds=1)
                self.assertEqual(leased.state, "leased")
                checkpointed = store.checkpoint(
                    "job-1",
                    "worker-a",
                    phase="mission_preflight",
                    checkpoint={"plan_digest": "a" * 64},
                    side_effect_boundary="preflight",
                )
                self.assertEqual(checkpointed.state, "running")
                clock.advance(2)
                recovered = store.recover_expired()
                self.assertEqual(len(recovered), 1)
                self.assertEqual(recovered[0].state, "queued")
                self.assertTrue(recovered[0].recovered_after_restart)
                reclaimed = store.claim("job-1", "worker-b", lease_seconds=10)
                self.assertEqual(reclaimed.attempts, 2)
                completed = store.complete("job-1", "worker-b", result_metadata={"cycle_status": "completed"})
                self.assertTrue(completed.terminal)
                self.assertTrue(store.verify_integrity()["ok"])
            with BrainJobStore(path, clock=clock) as reopened:
                self.assertEqual(reopened.get("job-1").state, "succeeded")  # type: ignore[union-attr]
                self.assertEqual(reopened.stats()["event_count"], 6)

    def test_dispatch_boundary_quarantines_expired_lease_and_rejects_secrets(self) -> None:
        clock = _Clock()
        with TemporaryDirectory() as directory:
            with BrainJobStore(f"{directory}/jobs.sqlite3", clock=clock) as store:
                store.submit(_job_packet())
                store.claim("job-1", "worker-a", lease_seconds=1)
                store.checkpoint(
                    "job-1",
                    "worker-a",
                    phase="dispatch_started",
                    checkpoint={"dispatch": "execute_true"},
                    side_effect_boundary="dispatched",
                )
                clock.advance(2)
                [recovered] = store.recover_expired()
                self.assertEqual(recovered.state, "reconciliation_required")
                with self.assertRaises(BrainJobError):
                    store.claim("job-1", "worker-b")
                with self.assertRaises(BrainJobError):
                    store.submit({**_job_packet("job-secret"), "checkpoint": {"api_key": "never-store"}})
                with self.assertRaises(BrainJobError):
                    store.submit({**_job_packet("job-different"), "idempotency_key": "idempotency-job-1", "spec_digest": _digest("other")})
                self.assertTrue(store.verify_integrity()["ok"])

    def test_waiting_approval_requires_explicit_release_before_a_new_lease(self) -> None:
        with TemporaryDirectory() as directory:
            with BrainJobStore(f"{directory}/jobs.sqlite3") as store:
                store.submit(_job_packet())
                store.claim("job-1", "worker-a")
                waiting = store.checkpoint(
                    "job-1",
                    "worker-a",
                    phase="approval_required",
                    checkpoint={"preflight_digest": "a" * 64},
                    side_effect_boundary="preflight",
                    waiting_for_approval=True,
                )
                self.assertEqual(waiting.state, "waiting_approval")
                with self.assertRaises(BrainJobError):
                    store.claim("job-1", "worker-b")
                released = store.resume_waiting("job-1", approver="operator-1")
                self.assertEqual(released.state, "queued")
                claimed = store.claim("job-1", "worker-b")
                self.assertEqual(claimed.state, "leased")

    def test_cooperative_release_preserves_checkpoint_and_requires_owner(self) -> None:
        with TemporaryDirectory() as directory:
            with BrainJobStore(f"{directory}/jobs.sqlite3") as store:
                store.submit(_job_packet())
                store.claim("job-1", "worker-a")
                store.checkpoint(
                    "job-1",
                    "worker-a",
                    phase="workflow_stage_checkpointed",
                    checkpoint={
                        "job_kind": "autonomous_workflow",
                        "completed_stage_ids": ["scope"],
                        "workflow_checkpoint_digest": "a" * 64,
                    },
                    side_effect_boundary="preflight",
                )
                with self.assertRaises(BrainJobError):
                    store.release("job-1", "worker-b")
                released = store.release("job-1", "worker-a", reason="stage handed to next worker")
                self.assertEqual(released.state, "queued")
                self.assertIsNone(released.lease_owner)
                self.assertEqual(released.checkpoint["completed_stage_ids"], ["scope"])
                self.assertEqual(released.checkpoint["phase"], "released")
                self.assertEqual(store.events(job_id="job-1")[-1].event_type, "job_released")
                claimed = store.claim("job-1", "worker-c")
                self.assertEqual(claimed.checkpoint["workflow_checkpoint_digest"], "a" * 64)


class DomainEvaluatorTests(unittest.TestCase):
    def _result(self) -> BrainRunResult:
        return BrainRunResult(
            run_id="run-evaluator",
            status="completed_provider_call",
            selection={
                "selected_model": {"provider": "openai", "model": "test-model"},
                "decision_digest": "a" * 64,
                "context_digest": "b" * 64,
            },
            prompt={"prompt_digest": "c" * 64},
            plan={"plan": {"plan_digest": "d" * 64}},
            response=None,
            outcome_digest="e" * 64,
        )

    def test_builtin_profiles_cover_all_major_domains_with_one_contract(self) -> None:
        registry = DomainEvaluatorRegistry.with_builtin_profiles()
        self.assertEqual(
            [entry["domain"] for entry in registry.catalogue()],
            ["biomedical", "data", "engineering", "operations", "research"],
        )
        for profile in builtin_domain_profiles():
            adapter = registry.resolve(profile.domain)
            evidence = adapter.normalize_evidence(
                {
                    "schema": "bioprism-brain-domain-evidence/0.1",
                    "domain": profile.domain,
                    "capability": "bounded_task",
                    "risk_class": "review",
                    "signals": {signal: True for signal in profile.required_signals},
                    "references": ["a" * 64],
                    "limitations": ["caller-declared signal evidence only"],
                }
            )
            decision = adapter.assess(self._result(), evidence=evidence.to_dict())
            self.assertTrue(decision.passed, profile.domain)
            self.assertFalse(decision.failed, profile.domain)
            self.assertEqual(decision.reward, 1.0)

    def test_domain_adapter_requests_replan_for_missing_signal_and_rejects_cross_domain_input(self) -> None:
        profile = DomainEvaluatorProfile(
            domain="research",
            evaluator_id="research-quality",
            evaluator_version="1",
            required_signals=("evidence_traceable", "uncertainty_reported"),
            signal_weights={"evidence_traceable": 1.0, "uncertainty_reported": 1.0},
        )
        adapter = DomainEvaluatorAdapter(profile)
        decision = adapter.assess(
            self._result(),
            evidence={
                "domain": "research",
                "capability": "literature_review",
                "risk_class": "high_review",
                "signals": {"evidence_traceable": True},
            },
        )
        self.assertFalse(decision.passed)
        self.assertTrue(decision.failed)
        self.assertTrue(decision.replan_requested)
        self.assertIn("uncertainty_reported", decision.replan_instruction or "")
        self.assertIsNotNone(decision.evidence_digest)
        with self.assertRaises(BrainRunError):
            adapter.normalize_evidence(
                {
                    "domain": "biomedical",
                    "capability": "clinical",
                    "risk_class": "high_review",
                    "signals": {"evidence_traceable": True},
                }
            )
        with self.assertRaises(BrainRunError):
            adapter.normalize_evidence(
                {
                    "domain": "research",
                    "capability": "literature_review",
                    "risk_class": "high_review",
                    "signals": {"evidence_traceable": True},
                    "limitations": ["Authorization: Bearer abcdefghijklmnop"],
                }
            )

    def test_domain_adapter_accepts_workflow_metadata_but_scores_only_value_signals(self) -> None:
        profile = next(
            profile
            for profile in builtin_autonomous_domain_evaluator_profiles()
            if profile.domain == "coding"
        )
        adapter = DomainEvaluatorAdapter(profile)
        normalized = adapter.normalize_evidence(
            {
                "schema": "bioprism-python-autonomous-workflow-evaluator/0.1",
                "workflow_id": "coding_delivery",
                "workflow_digest": "a" * 64,
                "stage_id": "verify",
                "required_signals": ["tests_passed"],
                "domain": "coding",
                "capability": "testing",
                "risk_class": "review",
                "signals": {signal: True for signal in profile.required_signals},
            }
        )
        self.assertEqual(set(normalized.signals), set(profile.required_signals))
        self.assertTrue(adapter.assess(self._result(), evidence=normalized.to_dict()).passed)
        with self.assertRaises(BrainRunError):
            adapter.normalize_evidence({
                "domain": "coding",
                "capability": "testing",
                "risk_class": "review",
                "workflow_id": "x" * 257,
                "signals": {signal: True for signal in profile.required_signals},
            })

    def test_autonomous_profiles_cover_all_twelve_domains_and_keep_legacy_aliases(self) -> None:
        registry = DomainEvaluatorRegistry.with_builtin_autonomous_profiles()
        profiles = builtin_autonomous_domain_evaluator_profiles()
        domains = {profile.domain for profile in profiles}
        self.assertEqual(len(profiles), 12)
        self.assertEqual(
            domains,
            {
                "coding", "browser", "data", "science", "biomedical", "neuroscience",
                "operations", "enterprise", "multi_agent", "multimodal", "cross_domain", "evaluation",
            },
        )
        self.assertEqual(len(registry.catalogue()), 17)
        for profile in profiles:
            adapter = registry.resolve_for_autonomous_domain(profile.domain)
            evidence = adapter.normalize_evidence(
                {
                    "domain": profile.domain,
                    "capability": "bounded_task",
                    "risk_class": "review",
                    "signals": {signal: True for signal in profile.required_signals},
                }
            )
            decision = adapter.assess(self._result(), evidence=evidence.to_dict())
            self.assertTrue(decision.passed, profile.domain)
            self.assertEqual(decision.evaluator_id, profile.evaluator_id)
        self.assertIs(
            registry.resolve_for_autonomous_domain("coding").profile,
            registry.resolve("coding").profile,
        )
        self.assertEqual(
            registry.resolve_for_autonomous_domain("coding", fallback_domain="engineering").evaluator_id,
            "autonomous-coding-quality",
        )
        custom = DomainEvaluatorRegistry(
            [
                DomainEvaluatorAdapter(
                    DomainEvaluatorProfile(
                        domain="coding",
                        evaluator_id="custom-coding-quality",
                        evaluator_version="1",
                        required_signals=("tests_passed",),
                        signal_weights={"tests_passed": 1.0},
                    )
                )
            ]
        )
        self.assertEqual(
            custom.resolve_for_autonomous_domain("coding", fallback_domain="engineering").evaluator_id,
            "custom-coding-quality",
        )
        legacy = DomainEvaluatorRegistry.with_builtin_profiles().resolve("engineering")
        self.assertTrue(
            legacy.normalize_evidence(
                {
                    "domain": "coding",
                    "capability": "implementation",
                    "risk_class": "review",
                    "signals": {signal: True for signal in legacy.profile.required_signals},
                }
            )
        )
    def test_resumable_job_rehydrates_spec_without_persisting_task_or_credentials(self) -> None:
        brain = AutonomousBrain(_OutcomeWorkspace(), LLMRuntime())
        run = self._result()
        mission = BrainMissionResult(
            brain_run=run,
            status="mission_approval_required",
            mission={"steps": []},
            preflight={"execution": "not_started"},
            execution=None,
        )
        cycle = BrainLearningCycleResult(
            status="completed",
            final_result=mission,
            attempts=(mission,),
            evaluations=(),
            memory_receipts=(),
            recalled_memory=(),
            replan_count=0,
        )
        dispatched = BrainMissionResult(
            brain_run=run,
            status="mission_dispatched",
            mission={"steps": []},
            preflight={"execution": "ready"},
            execution={"status": "dispatched"},
        )
        dispatched_cycle = BrainLearningCycleResult(
            status="completed",
            final_result=dispatched,
            attempts=(dispatched,),
            evaluations=(),
            memory_receipts=(),
            recalled_memory=(),
            replan_count=0,
        )
        calls: list[dict[str, object]] = []
        cycles = [cycle, dispatched_cycle]

        def fake_cycle(**kwargs: object) -> BrainLearningCycleResult:
            calls.append(kwargs)
            return cycles.pop(0)

        brain.run_adaptive_mission_learning_cycle = fake_cycle  # type: ignore[method-assign]
        evaluator = BrainOutcomeEvaluator(
            lambda _input: BrainEvaluatorDecision(
                evaluator_id="job-evaluator",
                evaluator_version="1",
                reward=1.0,
                passed=True,
            ),
            evaluator_id="job-evaluator",
            evaluator_version="1",
        )
        with TemporaryDirectory() as directory:
            with BrainJobStore(f"{directory}/jobs.sqlite3") as store:
                store.submit(_job_packet())
                resolved = {
                    "task": "private task text never persisted",
                    "model_candidates": [],
                    "prompt": {},
                    "plan": {},
                    "credentials": {"openai": "in-memory-handle"},
                    "mission_policy": {"allowed_tools": []},
                }
                seen_metadata: list[dict[str, object]] = []

                def resolve(metadata: dict[str, object]) -> dict[str, object]:
                    seen_metadata.append(metadata)
                    return resolved

                result = brain.run_resumable_learning_job(
                    store,
                    job_id="job-1",
                    worker_id="worker-a",
                    resolver=resolve,
                    evaluator=evaluator,
                    bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                )
                self.assertEqual(result.status, "waiting_approval")
                self.assertEqual(result.cycle, cycle)
                self.assertEqual(len(calls), 1)
                self.assertEqual(len(seen_metadata), 1)
                pending = BrainApprovalRouter(store).get("job-1")
                self.assertIsNotNone(pending)
                self.assertEqual(pending.state, "pending")  # type: ignore[union-attr]
                BrainApprovalRouter(store).approve("job-1", approver="operator-1")
                self.assertEqual(store.get("job-1").state, "queued")  # type: ignore[union-attr]
                resumed = brain.run_resumable_learning_job(
                    store,
                    job_id="job-1",
                    worker_id="worker-b",
                    resolver=resolve,
                    evaluator=evaluator,
                    bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                )
                self.assertEqual(resumed.status, "succeeded")
                self.assertTrue(calls[1]["mission_options"]["approve_mission_dispatch"])  # type: ignore[index]
                serialized = str(store.get("job-1").to_dict())  # type: ignore[union-attr]
                self.assertNotIn("private task text", serialized)
                self.assertNotIn("in-memory-handle", serialized)
                self.assertTrue(store.verify_integrity()["ok"])

    def test_resumable_job_marks_uncertain_execution_for_reconciliation(self) -> None:
        brain = AutonomousBrain(_OutcomeWorkspace(), LLMRuntime())

        def failing_cycle(**_: object) -> BrainLearningCycleResult:
            raise RuntimeError("provider failure after unknown boundary")

        brain.run_adaptive_mission_learning_cycle = failing_cycle  # type: ignore[method-assign]
        evaluator = BrainOutcomeEvaluator(
            lambda _input: {"reward": 0.0, "passed": False, "failed": True},
            evaluator_id="job-evaluator",
            evaluator_version="1",
        )
        with TemporaryDirectory() as directory:
            with BrainJobStore(f"{directory}/jobs.sqlite3") as store:
                store.submit(_job_packet())
                result = brain.run_resumable_learning_job(
                    store,
                    job_id="job-1",
                    worker_id="worker-a",
                    resolver=lambda _metadata: {
                        "task": "task",
                        "model_candidates": [],
                        "prompt": {},
                        "plan": {},
                        "credentials": {},
                        "mission_policy": {"allowed_tools": []},
                    },
                    evaluator=evaluator,
                    bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                )
                self.assertEqual(result.status, "reconciliation_required")
                self.assertEqual(store.get("job-1").side_effect_boundary, "unknown")  # type: ignore[union-attr]


if __name__ == "__main__":
    unittest.main()
