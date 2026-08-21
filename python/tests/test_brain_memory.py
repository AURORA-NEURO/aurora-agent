from __future__ import annotations

import hashlib
import json
import sqlite3
from tempfile import TemporaryDirectory
import unittest

from prism_sdk.brain import (
    AutonomousBrain,
    BrainEvaluatorDecision,
    BrainLearningLedger,
    BrainMissionResult,
    BrainOutcomeEvaluator,
    BrainRunError,
    BrainRunResult,
)
from prism_sdk.llm_runtime import LLMRuntime
from prism_sdk.memory import (
    BrainEpisodicMemory,
    BrainMemoryError,
    MemoryQuery,
    task_facet_digests,
)


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _packet(*, episode_id: str, domain: str = "engineering", status: str = "mission_approval_required") -> dict[str, object]:
    return {
        "episode_id": episode_id,
        "run_id": episode_id + "-run",
        "result_kind": "mission",
        "status": status,
        "task_digest": _digest("inspect the platform"),
        "context": {
            "domain": domain,
            "capability": "release_audit",
            "risk_class": "release_review",
        },
        "selected_model": {"provider": "openai", "model": "test-model"},
        "digests": {
            "selection_digest": "a" * 64,
            "prompt_digest": "b" * 64,
            "plan_digest": "c" * 64,
            "outcome_digest": "d" * 64,
        },
        "tags": ["release", "bounded"],
        "lesson": "Keep the release evidence review bounded and explicit.",
        "provenance": {"source": "unit-test", "catalog_version": "test-v1"},
    }


class BrainMemoryTests(unittest.TestCase):
    def test_digest_only_task_facets_retrieve_related_work_without_retaining_vocabulary(self) -> None:
        related_task = "review the release evidence and validate the implementation contract"
        unrelated_task = "compare imaging modalities and quantify signal reproducibility"
        related_facets = task_facet_digests(related_task)
        self.assertTrue(related_facets)
        self.assertEqual(related_facets, task_facet_digests(related_task))
        self.assertNotIn("release", json.dumps(related_facets))

        with TemporaryDirectory() as directory:
            memory = BrainEpisodicMemory(f"{directory}/episodes.sqlite3")
            memory.record_episode(
                {
                    **_packet(episode_id="episode-related", domain="engineering"),
                    "task_digest": _digest(related_task),
                    "task_facets": related_facets,
                }
            )
            memory.record_episode(
                {
                    **_packet(episode_id="episode-unrelated", domain="engineering"),
                    "task_digest": _digest(unrelated_task),
                    "task_facets": task_facet_digests(unrelated_task),
                }
            )
            recalled = memory.retrieve(
                MemoryQuery(
                    domain="engineering",
                    task_facets=related_facets,
                    limit=4,
                )
            )
            self.assertEqual([row["episode_id"] for row in recalled], ["episode-related"])
            self.assertEqual(recalled[0]["task_facets"], list(related_facets))
            self.assertNotIn(related_task, json.dumps(recalled))
            self.assertNotIn(unrelated_task, json.dumps(recalled))
            self.assertTrue(memory.verify_integrity()["ok"])
            memory.close()

    def test_legacy_memory_schema_migrates_the_derived_facet_index(self) -> None:
        with TemporaryDirectory() as directory:
            path = f"{directory}/legacy.sqlite3"
            connection = sqlite3.connect(path)
            connection.executescript(
                """
                CREATE TABLE memory_events (
                    sequence INTEGER PRIMARY KEY AUTOINCREMENT,
                    event_type TEXT NOT NULL,
                    episode_id TEXT NOT NULL,
                    payload_json TEXT NOT NULL,
                    previous_digest TEXT NOT NULL,
                    event_digest TEXT NOT NULL UNIQUE,
                    created_ns INTEGER NOT NULL
                );
                CREATE TABLE memory_episodes (
                    episode_id TEXT PRIMARY KEY,
                    run_id TEXT NOT NULL,
                    result_kind TEXT NOT NULL,
                    status TEXT NOT NULL,
                    task_digest TEXT NOT NULL,
                    domain TEXT,
                    capability TEXT,
                    risk_class TEXT,
                    tags_json TEXT NOT NULL,
                    packet_json TEXT NOT NULL,
                    evaluation_json TEXT,
                    record_sequence INTEGER NOT NULL,
                    record_digest TEXT NOT NULL,
                    created_ns INTEGER NOT NULL,
                    updated_ns INTEGER NOT NULL
                );
                """
            )
            connection.commit()
            connection.close()
            with BrainEpisodicMemory(path) as memory:
                columns = {
                    row[1]
                    for row in memory._connection.execute("PRAGMA table_info(memory_episodes)").fetchall()
                }
                self.assertIn("task_facets_json", columns)
                memory.record_episode(
                    {
                        **_packet(episode_id="migrated"),
                        "task_facets": task_facet_digests("release evidence validation"),
                    }
                )
                self.assertTrue(memory.verify_integrity()["ok"])

    def test_memory_persists_queries_evaluations_and_integrity_across_restart(self) -> None:
        with TemporaryDirectory() as directory:
            path = f"{directory}/episodes.sqlite3"
            with BrainEpisodicMemory(path, clock=lambda: 100.0) as memory:
                first = memory.record_episode(_packet(episode_id="episode-1"))
                second = memory.record_episode(
                    {
                        **_packet(episode_id="episode-2", domain="science"),
                        "status": "completed_without_replan",
                        "tags": ["science"],
                    }
                )
                self.assertFalse(first.idempotent)
                self.assertFalse(second.idempotent)
                evaluation = memory.record_evaluation(
                    "episode-1",
                    {
                        "evaluator_id": "release-evaluator",
                        "evaluator_version": "1",
                        "reward": 0.9,
                        "passed": True,
                        "failed": False,
                        "decision_digest": "e" * 64,
                    },
                )
                self.assertFalse(evaluation.idempotent)
                recalled = memory.retrieve(
                    MemoryQuery(domain="engineering", capability="release_audit", limit=4)
                )
                self.assertEqual([row["episode_id"] for row in recalled], ["episode-1"])
                self.assertEqual(recalled[0]["evaluation"]["passed"], True)  # type: ignore[index]
                self.assertTrue(memory.verify_integrity()["ok"])
                self.assertEqual(memory.stats()["evaluation_count"], 1)

            with BrainEpisodicMemory(path, clock=lambda: 101.0) as reopened:
                self.assertEqual(reopened.get("episode-1")["lesson"], _packet(episode_id="episode-1")["lesson"])  # type: ignore[index]
                self.assertEqual(len(reopened.retrieve({"tags": ["science"], "limit": 4})), 1)
                self.assertTrue(reopened.verify_integrity()["ok"])

    def test_memory_is_idempotent_but_rejects_secret_and_raw_content_fields(self) -> None:
        with TemporaryDirectory() as directory:
            memory = BrainEpisodicMemory(f"{directory}/episodes.sqlite3")
            first = memory.record_episode(_packet(episode_id="episode-1"))
            replay = memory.record_episode(_packet(episode_id="episode-1"))
            self.assertTrue(replay.idempotent)
            self.assertEqual(first.event_digest, replay.event_digest)
            with self.assertRaises(BrainMemoryError):
                memory.record_episode({**_packet(episode_id="episode-1"), "status": "different"})
            with self.assertRaises(BrainMemoryError):
                memory.record_episode({**_packet(episode_id="episode-secret"), "provenance": {"api_key": "do-not-store"}})
            with self.assertRaises(BrainMemoryError):
                memory.record_episode({**_packet(episode_id="episode-raw"), "lesson": "Authorization: Bearer abcdefghijklmnop"})
            with self.assertRaises(BrainMemoryError):
                memory.record_evaluation("missing", {"reward": 1.0, "passed": True})
            with self.assertRaises(BrainRunError):
                BrainEvaluatorDecision(
                    evaluator_id="evaluator",
                    evaluator_version="1",
                    reward=0.0,
                    passed=False,
                    failed=True,
                    replan_requested=True,
                    replan_instruction="Use api_key: do-not-forward",
                )
            memory.close()

    def test_memory_integrity_detects_event_tampering(self) -> None:
        with TemporaryDirectory() as directory:
            path = f"{directory}/episodes.sqlite3"
            memory = BrainEpisodicMemory(path)
            memory.record_episode(_packet(episode_id="episode-1"))
            memory.close()
            connection = sqlite3.connect(path)
            connection.execute("UPDATE memory_events SET event_digest = ? WHERE sequence = 1", ("0" * 64,))
            connection.commit()
            connection.close()
            tampered = BrainEpisodicMemory(path)
            report = tampered.verify_integrity()
            self.assertFalse(report["ok"])
            self.assertIn("digest mismatch", report["reason"])
            tampered.close()

    def test_learning_cycle_replans_before_dispatch_and_persists_each_attempt(self) -> None:
        class Workspace:
            def __init__(self) -> None:
                self.outcome_calls = 0

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                self.assert_name = name
                if name != "brain_outcome_record":
                    raise AssertionError(f"unexpected tool {name}")
                self.outcome_calls += 1
                return {
                    "ok": True,
                    "status": "recorded_evaluator_reward",
                    "next_state": {
                        "schema": "bioprism-brain-bandit/0.1",
                        "generation": self.outcome_calls,
                        "arms": [],
                    },
                    "learning_evidence": {
                        "schema": "bioprism-brain-learning-evidence/0.1",
                        "evidence_digest": "f" * 64,
                    },
                }

        workspace = Workspace()
        brain = AutonomousBrain(workspace, LLMRuntime())
        prompts: list[dict[str, object]] = []

        def fake_mission(**kwargs: object) -> BrainMissionResult:
            prompts.append(kwargs["prompt"])  # type: ignore[arg-type]
            attempt = len(prompts)
            run = BrainRunResult(
                run_id=f"run-{attempt}",
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
            return BrainMissionResult(
                brain_run=run,
                status="mission_approval_required",
                mission={"steps": []},
                preflight={"execution": "not_started"},
                execution=None,
            )

        brain.run_adaptive_mission = fake_mission  # type: ignore[method-assign]
        evaluator_calls = 0

        def evaluate(_input: dict[str, object]) -> dict[str, object]:
            nonlocal evaluator_calls
            evaluator_calls += 1
            if evaluator_calls == 1:
                return {
                    "reward": 0.1,
                    "passed": False,
                    "failed": True,
                    "failure_class": "insufficient_evidence",
                    "replan_requested": True,
                    "replan_instruction": "Add the missing release evidence step.",
                }
            return {"reward": 0.95, "passed": True, "failed": False}

        evaluator = BrainOutcomeEvaluator(
            evaluate,
            evaluator_id="mission-quality",
            evaluator_version="1",
        )
        with TemporaryDirectory() as directory:
            memory = BrainEpisodicMemory(f"{directory}/episodes.sqlite3", clock=lambda: 100.0)
            result = brain.run_adaptive_mission_learning_cycle(
                task="inspect the platform",
                model_candidates=[],
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"]},
                credentials={},
                mission_policy={"allowed_tools": ["developer_platform_status"]},
                evaluator=evaluator,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "generation": 0, "arms": []},
                memory=memory,
                memory_tags=("release",),
                max_replans=1,
                trajectory_discount=0.5,
                trajectory_terminal_reward=0.2,
                mission_options={
                    "context": {
                        "domain": "engineering",
                        "capability": "release_audit",
                        "risk_class": "release_review",
                    }
                },
            )
            self.assertEqual(result.status, "completed")
            self.assertEqual(result.replan_count, 1)
            self.assertEqual(len(result.attempts), 2)
            self.assertEqual(workspace.outcome_calls, 2)
            self.assertEqual(len(memory.retrieve({"domain": "engineering", "limit": 8})), 2)
            self.assertIsNotNone(result.trajectory_result)
            self.assertEqual(len(result.trajectory_result.credited_rewards), 2)  # type: ignore[union-attr]
            self.assertIn("trajectory_id", result.evaluations[0]["recording"])
            self.assertTrue(memory.verify_integrity()["ok"])
            self.assertTrue(any(chunk["id"] == "brain-replan" for chunk in prompts[1]["context"]))  # type: ignore[index]
            self.assertNotIn("Authorization", json.dumps(result.to_dict()))
            memory.close()

    def test_learning_cycle_blocks_replan_after_dispatch(self) -> None:
        class Workspace:
            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name != "brain_outcome_record":
                    raise AssertionError(name)
                return {
                    "ok": True,
                    "status": "recorded_evaluator_reward",
                    "next_state": {"schema": "bioprism-brain-bandit/0.1", "generation": 1, "arms": []},
                    "learning_evidence": {"schema": "bioprism-brain-learning-evidence/0.1", "evidence_digest": "f" * 64},
                }

        brain = AutonomousBrain(Workspace(), LLMRuntime())
        run = BrainRunResult(
            run_id="run-dispatched",
            status="completed_provider_call",
            selection={"selected_model": {"provider": "openai", "model": "test-model"}, "decision_digest": "a" * 64},
            prompt={"prompt_digest": "b" * 64},
            plan={"plan": {"plan_digest": "c" * 64}},
            response=None,
            outcome_digest="d" * 64,
        )
        dispatched = BrainMissionResult(
            brain_run=run,
            status="mission_dispatched",
            mission={"steps": []},
            preflight={"execution": "not_started"},
            execution={"execution": "executed"},
        )
        calls = 0

        def fake_mission(**_: object) -> BrainMissionResult:
            nonlocal calls
            calls += 1
            return dispatched

        brain.run_adaptive_mission = fake_mission  # type: ignore[method-assign]
        evaluator = BrainOutcomeEvaluator(
            lambda _input: {
                "reward": 0.0,
                "passed": False,
                "failed": True,
                "failure_class": "external_effect_uncertain",
                "replan_requested": True,
            },
            evaluator_id="safety-evaluator",
            evaluator_version="1",
        )
        with TemporaryDirectory() as directory:
            memory = BrainEpisodicMemory(f"{directory}/episodes.sqlite3")
            result = brain.run_adaptive_mission_learning_cycle(
                task="perform the release",
                model_candidates=[],
                prompt={"max_input_tokens": 100},
                plan={"allowed_tools": ["provider.invoke"]},
                credentials={},
                mission_policy={"allowed_tools": ["release_publish"]},
                evaluator=evaluator,
                bandit_state={"schema": "bioprism-brain-bandit/0.1", "arms": []},
                memory=memory,
                max_replans=2,
            )
            self.assertEqual(result.status, "replan_blocked_after_dispatch")
            self.assertEqual(result.replan_count, 0)
            self.assertEqual(calls, 1)
            memory.close()


if __name__ == "__main__":
    unittest.main()
