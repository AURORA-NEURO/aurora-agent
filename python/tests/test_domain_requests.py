from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    AsyncClient,
    AsyncWorkspace,
    Client,
    LabPlanRequest,
    RoutingDecisionRequest,
    WorldClaimCheckRequest,
    Workspace,
)
from prism_sdk.errors import ArgumentError


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


class DomainRequestModelTests(unittest.TestCase):
    def test_models_preserve_explicit_domain_payloads_and_bounds(self) -> None:
        world = WorldClaimCheckRequest({"top": "observed", "selection": {}}, {"kind": "biology", "quantity": "outcome"})
        lab = LabPlanRequest(
            {"obligations": []},
            [{"id": "assay-1", "targets": ["obligation-1"]}],
            {"tokens": 10, "latency_units": 4},
            marginal_value_floor=0.25,
            hypotheses={"hypotheses": []},
            observations={"observation-1": "value"},
            max_items=4,
        )
        routing = RoutingDecisionRequest(
            {"features": {"modality": "rna"}},
            [{"task_id": "other", "architecture": "arch-a", "outcome": "observed"}],
            {"approved_architectures": ["arch-a"], "safe_default": "abstain"},
            task_id="unseen-task",
        )

        self.assertEqual(world.to_mcp_arguments()["claim"]["kind"], "biology")
        self.assertEqual(lab.to_mcp_arguments()["marginal_value_floor"], 0.25)
        self.assertEqual(routing.to_mcp_arguments()["task_id"], "unseen-task")

    def test_models_fail_closed_on_unsafe_values_and_limits(self) -> None:
        with self.assertRaises(ArgumentError):
            LabPlanRequest({}, [], {}, marginal_value_floor=float("nan"))
        with self.assertRaises(ArgumentError):
            LabPlanRequest({}, [], {}, marginal_value_floor=True)  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            RoutingDecisionRequest({}, [{"x": float("nan")}], {})
        with self.assertRaises(ArgumentError):
            WorldClaimCheckRequest([], {})  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            LabPlanRequest({}, [{"id": str(index)} for index in range(1_001)], {})

    def test_sync_workspace_exposes_world_lab_and_routing_workflows(self) -> None:
        with Client(command(), timeout=2) as client:
            workspace = Workspace(client)
            claim = workspace.world_claim_check({"top": "observed"}, {"kind": "biology", "quantity": "outcome"})
            plan = workspace.lab_plan({"obligations": []}, [{"id": "assay"}], {"tokens": 1})
            route = workspace.routing_decide({"features": {}}, [{"task_id": "other"}], {"safe_default": "abstain"}, task_id="new")

        self.assertEqual(claim["echo"]["claim"]["kind"], "biology")
        self.assertEqual(plan["echo"]["actions"][0]["id"], "assay")
        self.assertEqual(route["echo"]["task_id"], "new")


class AsyncDomainWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_exposes_world_claim_check(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).world_claim_check(
                WorldClaimCheckRequest({"top": "observed"}, {"kind": "biology", "quantity": "outcome"})
            )

        self.assertEqual(result["echo"]["provenance"]["top"], "observed")


if __name__ == "__main__":
    unittest.main()
