from __future__ import annotations

import unittest

from prism_sdk import (
    ArgumentError,
    MissionBinding,
    MissionRouteSelection,
    MissionRequest,
    MissionStep,
    ToolCatalogue,
    mission_from_route,
    preflight_mission,
)


def catalogue() -> ToolCatalogue:
    return ToolCatalogue.from_definitions(
        [
            {
                "name": "echo",
                "description": "fixture",
                "inputSchema": {
                    "type": "object",
                    "required": ["value"],
                    "properties": {"value": {"type": "integer"}},
                },
            },
            {
                "name": "audit",
                "description": "fixture",
                "inputSchema": {
                    "type": "object",
                    "required": ["value"],
                    "properties": {"value": {"type": "integer"}},
                },
            },
        ]
    )


class MissionPreflightTests(unittest.TestCase):
    def test_preflight_returns_digest_bound_waves_and_binding_checks(self) -> None:
        request = MissionRequest(
            "mission-1",
            "check a value",
            [
                MissionStep("acquire", "data", "read", "obtain value", "echo", {"value": 3}),
                MissionStep(
                    "audit",
                    "evaluation",
                    "verify",
                    "verify value",
                    "audit",
                    {"value": 0},
                    depends_on=("acquire",),
                    bindings=(MissionBinding("acquire", "/echo/value", "/value"),),
                ),
            ],
        )
        report = preflight_mission(request, catalogue())
        self.assertTrue(report.ok)
        self.assertTrue(report.fully_checked)
        self.assertEqual(report.waves, (("acquire",), ("audit",)))
        self.assertEqual(report.ordered_steps, ("acquire", "audit"))
        self.assertEqual(len(report.request_digest), 64)
        self.assertEqual(len(report.catalogue_digest), 64)
        self.assertEqual(report.to_dict()["limitations"][-1], "no step is executed by this report")

    def test_preflight_preserves_cycle_and_schema_failures(self) -> None:
        request = MissionRequest(
            "mission-cycle",
            "cycle",
            [
                MissionStep("a", "data", "read", "a", "echo", {"value": 1}, depends_on=("b",)),
                MissionStep("b", "evaluation", "check", "b", "audit", {"value": "bad"}, depends_on=("a",)),
            ],
        )
        report = preflight_mission(request, catalogue())
        self.assertFalse(report.ok)
        self.assertTrue(any("dependency cycle" in issue for issue in report.issues))
        self.assertTrue(any(step.status == "blocked" for step in report.steps))
        self.assertTrue(any("type" in issue for step in report.steps for issue in step.issues))

        unknown = preflight_mission(
            MissionRequest(
                "mission-unknown",
                "unknown",
                [MissionStep("one", "data", "read", "one", "missing_tool", {})],
            ),
            catalogue(),
        )
        self.assertFalse(unknown.ok)
        self.assertTrue(any("absent from the live" in issue for step in unknown.steps for issue in step.issues))

    def test_preflight_checks_execution_authority_and_request_shape(self) -> None:
        request = MissionRequest(
            "mission-execute",
            "execute",
            [MissionStep("one", "data", "read", "one", "echo", {"value": 1})],
            policy={"execute": True},
        )
        report = preflight_mission(request, catalogue())
        self.assertFalse(report.ok)
        self.assertTrue(any("allowed_tools" in issue for issue in report.issues))

        with self.assertRaises(ArgumentError):
            MissionRequest("bad", "missing step field", [{"id": "one"}])
        with self.assertRaises(ArgumentError):
            MissionBinding("one", "/valid", "/invalid~2")

    def test_parallel_waves_are_budgeted_and_reported_before_execution(self) -> None:
        request = MissionRequest(
            "mission-parallel",
            "run independent checks",
            [
                MissionStep("echo", "data", "read", "echo a value", "echo", {"value": 1}),
                MissionStep("audit", "evaluation", "verify", "audit a value", "audit", {"value": 2}),
            ],
            policy={
                "execute": True,
                "execution_mode": "parallel_waves",
                "max_parallelism": 2,
                "allowed_tools": ["echo", "audit"],
                "max_step_output_bytes": 2_000_000,
                "max_total_output_bytes": 4_000_000,
            },
        )
        report = preflight_mission(request, catalogue())
        self.assertTrue(report.ok)
        self.assertEqual(report.execution_mode, "parallel_waves")
        self.assertEqual(report.max_parallelism, 2)
        self.assertEqual(report.waves, (("audit", "echo"),))
        self.assertEqual(report.to_dict()["execution_mode"], "parallel_waves")

        invalid_mode = preflight_mission(
            MissionRequest(
                "mission-bad-mode",
                "reject an unknown execution mode",
                [MissionStep("one", "data", "read", "one", "echo", {"value": 1})],
                policy={"execution_mode": "distributed"},
            ),
            catalogue(),
        )
        self.assertFalse(invalid_mode.ok)
        self.assertTrue(any("execution_mode" in issue for issue in invalid_mode.issues))

        invalid_parallelism = preflight_mission(
            MissionRequest(
                "mission-bad-parallelism",
                "reject an unsafe concurrency ceiling",
                [MissionStep("one", "data", "read", "one", "echo", {"value": 1})],
                policy={"execution_mode": "parallel_waves", "max_parallelism": 17},
            ),
            catalogue(),
        )
        self.assertFalse(invalid_parallelism.ok)
        self.assertTrue(any("max_parallelism" in issue for issue in invalid_parallelism.issues))

        under_budget = MissionRequest(
            "mission-under-budget",
            "reject an unsafe wave reservation",
            [
                MissionStep("echo", "data", "read", "echo a value", "echo", {"value": 1}),
                MissionStep("audit", "evaluation", "verify", "audit a value", "audit", {"value": 2}),
            ],
            policy={
                "execution_mode": "parallel_waves",
                "max_step_output_bytes": 2_000_000,
                "max_total_output_bytes": 3_000_000,
            },
        )
        under_budget_report = preflight_mission(under_budget, catalogue())
        self.assertFalse(under_budget_report.ok)
        self.assertTrue(any("worst-case wave" in issue for issue in under_budget_report.issues))

    def test_route_assembly_requires_explicit_candidates_and_preserves_provenance(self) -> None:
        route = {
            "workflow": "capability_route",
            "route_id": "route-123",
            "catalog_digest": "c" * 64,
            "goal": "compose a checked route",
            "needs": [
                {"id": "acquire", "resolution": "explicit", "candidate_tools": ["echo"]},
                {"id": "audit", "resolution": "ranked_candidates", "candidate_tools": ["audit", "echo"]},
            ],
            "unresolved_needs": [],
        }
        assembly = mission_from_route(
            route,
            "mission-from-route",
            [
                MissionRouteSelection("acquire", "echo", "data", "read", "acquire", {"value": 3}),
                MissionRouteSelection("audit", "audit", "evaluation", "verify", "audit", {"value": 0}, depends_on=("acquire",)),
            ],
        )
        self.assertEqual(assembly.route_id, "route-123")
        self.assertEqual(assembly.selected_tools, ("echo", "audit"))
        report = preflight_mission(assembly.request, catalogue())
        self.assertTrue(report.ok)
        self.assertEqual(assembly.to_dict()["mission"]["mission_id"], "mission-from-route")
        with self.assertRaises(ArgumentError):
            mission_from_route(
                route,
                "bad-route",
                [
                    MissionRouteSelection("acquire", "missing", "data", "read", "acquire", {"value": 3}),
                    MissionRouteSelection("audit", "audit", "evaluation", "verify", "audit", {"value": 0}),
                ],
            )


if __name__ == "__main__":
    unittest.main()
