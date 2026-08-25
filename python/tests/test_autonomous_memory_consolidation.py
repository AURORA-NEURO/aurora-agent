from __future__ import annotations

import hashlib
import json
import unittest

from prism_sdk import (
    AUTONOMOUS_DOMAINS,
    AutonomousAgent,
    AutonomousMemoryConsolidationError,
    AutonomousMemoryConsolidationPersistenceCoordinator,
    AutonomousMemoryConsolidator,
    TransactionalJsonAutonomousMemoryConsolidationPersistence,
    LLMRuntime,
    validate_autonomous_memory_consolidation_report,
    validate_autonomous_memory_consolidation_snapshot,
)


def _digest(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()


def _observation(
    *,
    episode_id: str,
    domain: str,
    concept_id: str = "portable-review",
    variant_id: str = "bounded-v1",
    lesson_id: str = "lesson-bounded-review",
    lesson_digest: str | None = None,
    reward: float = 1.0,
    passed: bool = True,
    transferable: bool = True,
    observed_at: float = 100.0,
) -> dict[str, object]:
    return {
        "episode_id": episode_id,
        "lesson_id": lesson_id,
        "concept_id": concept_id,
        "variant_id": variant_id,
        "domain": domain,
        "capability": "evidence_review",
        "risk_class": "read_only",
        "evaluator_id": f"evaluator-{episode_id}",
        "evaluator_version": "v1",
        "reward": reward,
        "passed": passed,
        "evidence_digest": _digest(f"evidence-{episode_id}"),
        "lesson_digest": lesson_digest or _digest(f"{lesson_id}-{variant_id}"),
        "decision_digest": _digest(f"decision-{episode_id}"),
        "observed_at": observed_at,
        "transferable": transferable,
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


class AutonomousMemoryConsolidationTests(unittest.TestCase):
    def test_support_transfer_conflict_and_prompt_scope_cover_every_domain(self) -> None:
        consolidator = AutonomousMemoryConsolidator(
            min_observations=3,
            min_support_lower_bound=0.4,
            conflict_dominance=0.75,
            clock=lambda: 100.0,
        )
        observations = [
            _observation(episode_id=f"portable-{index}", domain=domain)
            for index, domain in enumerate(AUTONOMOUS_DOMAINS)
        ]
        observations.extend(
            _observation(
                episode_id=f"conflict-a-{index}",
                domain="evaluation",
                concept_id="conflicting-review",
                variant_id="variant-a",
                lesson_id="lesson-conflicting-review",
                reward=1.0,
            )
            for index in range(3)
        )
        observations.extend(
            _observation(
                episode_id=f"conflict-b-{index}",
                domain="evaluation",
                concept_id="conflicting-review",
                variant_id="variant-b",
                lesson_id="lesson-conflicting-review",
                reward=0.0,
            )
            for index in range(3)
        )

        report = consolidator.consolidate(observations)
        self.assertEqual(report["observation_count"], len(observations))
        self.assertEqual(report["deduplicated_observation_count"], len(observations))
        self.assertEqual([row["domain"] for row in report["domains"]], list(AUTONOMOUS_DOMAINS))
        self.assertEqual([row["lesson_count"] for row in report["domains"][:-1]], [1] * (len(AUTONOMOUS_DOMAINS) - 1))
        self.assertEqual(report["domains"][-1]["lesson_count"], 3)

        portable = consolidator.recall(domain="biomedical", capability="evidence_review")
        self.assertEqual(len(portable), 1)
        self.assertEqual(portable[0]["status"], "stable")
        self.assertEqual(portable[0]["scope"], "cross_domain")
        references = consolidator.prompt_references(
            domain="biomedical",
            capability="evidence_review",
            lesson_resolver=lambda digest: "Keep evaluator-backed evidence bounded." if digest == portable[0]["lesson_digest"] else None,
        )
        self.assertEqual(references[0]["source"], "evaluator_gated_memory_consolidation")
        self.assertNotIn("Keep evaluator-backed", json.dumps(report, sort_keys=True))
        self.assertEqual(len(report["conflicts"]), 1)
        self.assertEqual(report["conflicts"][0]["variant_ids"], ["variant-a", "variant-b"])
        self.assertTrue(all(row["status"] == "conflicted" for row in report["lessons"] if row["concept_id"] == "conflicting-review"))

    def test_duplicate_replay_is_idempotent_and_contradictions_are_rejected(self) -> None:
        consolidator = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        row = _observation(episode_id="replay-1", domain="coding")
        report = consolidator.consolidate([row, dict(row)])
        self.assertEqual(report["observation_count"], 2)
        self.assertEqual(report["deduplicated_observation_count"], 1)
        missing_optional = dict(row)
        missing_optional.pop("decision_digest")
        self.assertEqual(
            AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
            .consolidate([missing_optional])["deduplicated_observation_count"],
            1,
        )
        contradictory = dict(row, reward=0.0)
        with self.assertRaises(AutonomousMemoryConsolidationError):
            consolidator.consolidate([row, contradictory])

    def test_snapshot_validation_persistence_and_tamper_fencing(self) -> None:
        source = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        source.consolidate([_observation(episode_id="persist-1", domain="operations")])
        store = _CasStore()
        persistence = TransactionalJsonAutonomousMemoryConsolidationPersistence(store)
        coordinator = AutonomousMemoryConsolidationPersistenceCoordinator(source, persistence)
        snapshot = coordinator.flush()
        self.assertEqual(validate_autonomous_memory_consolidation_snapshot(snapshot)["snapshot_digest"], snapshot["snapshot_digest"])
        self.assertEqual(validate_autonomous_memory_consolidation_report(snapshot["report"])["report_digest"], snapshot["report"]["report_digest"])
        restored = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        restored_coordinator = AutonomousMemoryConsolidationPersistenceCoordinator(restored, persistence)
        self.assertEqual(restored_coordinator.restore()["snapshot_digest"], snapshot["snapshot_digest"])
        self.assertEqual(restored.recall(domain="operations")[0]["lesson_id"], "lesson-bounded-review")

        tampered = json.loads(json.dumps(snapshot))
        tampered["report"]["lessons"][0]["lesson_id"] = "tampered-lesson"
        with self.assertRaises(AutonomousMemoryConsolidationError):
            validate_autonomous_memory_consolidation_snapshot(tampered)
        source.consolidate([_observation(episode_id="persist-2", domain="operations")])
        persistence.write(source.snapshot())
        with self.assertRaises(AutonomousMemoryConsolidationError):
            restored_coordinator.flush()

    def test_high_level_agent_exposes_the_same_consolidation_boundary(self) -> None:
        consolidator = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        agent = AutonomousAgent(object(), LLMRuntime(), memory_consolidator=consolidator)
        report = agent.consolidate_memory([_observation(episode_id="agent-1", domain="science")])
        references = agent.memory_references(
            domain="science",
            lesson_resolver=lambda _: "Use a reproducible evidence trail.",
        )
        self.assertEqual(report["domains"][3]["domain"], "science")
        self.assertEqual(references[0]["lesson_id"], "lesson-bounded-review")

    def test_local_lessons_do_not_transfer_and_age_status_is_explicit(self) -> None:
        local = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        report = local.consolidate([
            _observation(episode_id="local-coding", domain="coding", transferable=False),
            _observation(episode_id="local-browser", domain="browser", transferable=False),
        ])
        self.assertEqual(len(local.recall(domain="coding")), 1)
        self.assertEqual(len(local.recall(domain="browser")), 1)
        stale = AutonomousMemoryConsolidator(min_observations=3, min_support_lower_bound=0.0, max_age_seconds=10.0, clock=lambda: 100.0)
        stale_report = stale.consolidate([_observation(episode_id="old", domain="coding", observed_at=80.0)])
        self.assertEqual(stale_report["lessons"][0]["status"], "stale")
        self.assertEqual(report["domains"][0]["portable_count"], 0)

    def test_high_level_approval_plans_recall_stable_lessons_across_every_domain_without_retaining_text(self) -> None:
        consolidator = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        consolidator.consolidate([
            _observation(episode_id=f"integrated-{index}", domain=domain)
            for index, domain in enumerate(AUTONOMOUS_DOMAINS)
        ])
        lesson_text = "Use current evaluator-backed evidence and state uncertainty before acting."
        runtime = LLMRuntime()
        runtime.register_in_memory_provider("offline", lambda _request: {"output_text": "unused"})
        class Workspace:
            def __init__(self) -> None:
                self.prompt_contexts: list[object] = []

            def tool(self, name: str, arguments: dict[str, object] | None = None) -> dict[str, object]:
                if name == "brain_model_select_contextual":
                    context = dict((arguments or {}).get("context", {}))
                    identity = {field: context.get(field) for field in ("domain", "capability", "risk_class", "task_family")}
                    context_digest = _digest(json.dumps(identity, ensure_ascii=False, separators=(",", ":")))
                    return {
                        "context_digest": context_digest,
                        "selection_status": "selected",
                        "selection": {
                            "selected_model": {"provider": "offline", "model": "offline-model"},
                            "decision_digest": "d" * 64,
                        },
                    }
                if name == "brain_prompt_assemble":
                    self.prompt_contexts.append((arguments or {}).get("context"))
                    return {
                        "messages": [
                            {"role": "system", "content": str((arguments or {}).get("system", ""))},
                            {"role": "user", "content": str((arguments or {}).get("task", ""))},
                        ],
                        "prompt_digest": "a" * 64,
                    }
                if name == "brain_plan":
                    return {
                        "ok": True,
                        "plan": {
                            "requires_approval": True,
                            "steps": [{"effect": "provider_call"}],
                            "plan_digest": "b" * 64,
                        },
                    }
                raise AssertionError(f"unexpected workspace tool: {name}")

        workspace = Workspace()
        agent = AutonomousAgent(workspace, runtime, memory_consolidator=consolidator)
        candidate = {
            "provider": "offline",
            "model": "offline-model",
            "capabilities": ["reasoning", "code", "science", "data", "web", "biomedical", "operations", "enterprise", "coordination", "multimodal", "evaluation", "structured_output"],
            "context_window_tokens": 16_000,
            "max_output_tokens": 2_048,
            "quality": 0.9,
            "latency_ms": 20,
            "cost_per_million_tokens": 10,
            "reliability": 0.95,
        }
        for domain in AUTONOMOUS_DOMAINS:
            result = agent.orchestrator.run(
                task=f"prepare a bounded {domain} review",
                domain=domain,
                model_candidates=[candidate],
                credentials={},
                memory_consolidator=consolidator,
                memory_lesson_resolver=lambda _digest: lesson_text,
                consolidated_memory_required=True,
                approve_provider_call=False,
            )
            self.assertEqual(result.status, "approval_required", domain)
            prompt = json.dumps(workspace.prompt_contexts[-1], sort_keys=True)
            self.assertIn(lesson_text, prompt, domain)
            self.assertNotIn(lesson_text, json.dumps(result.selection, sort_keys=True))
            self.assertIn(_digest("lesson-bounded-review-bounded-v1"), prompt)

    def test_required_consolidated_recall_fails_closed_when_the_resolver_is_unavailable(self) -> None:
        consolidator = AutonomousMemoryConsolidator(min_observations=1, min_support_lower_bound=0.0, clock=lambda: 100.0)
        consolidator.consolidate([_observation(episode_id="required-lesson", domain="coding")])
        agent = AutonomousAgent(object(), LLMRuntime(), memory_consolidator=consolidator)
        with self.assertRaisesRegex(Exception, "consolidated_memory_required"):
            agent.orchestrator.run(
                task="prepare a bounded coding review",
                domain="coding",
                model_candidates=[],
                credentials={},
                memory_consolidator=consolidator,
                consolidated_memory_required=True,
                approve_provider_call=False,
            )


if __name__ == "__main__":
    unittest.main()
