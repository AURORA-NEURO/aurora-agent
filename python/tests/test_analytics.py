from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    AdapterPlanRequest,
    AdapterRegistry,
    AnalyticsDirection,
    AnalyticsEvidence,
    AsyncClient,
    AsyncWorkspace,
    CalibrationObservation,
    CapabilityQuery,
    CapabilityRouteNeed,
    CapabilityRouteRequest,
    Client,
    MetricObservation,
    MissionBinding,
    MissionPolicy,
    MissionRequest,
    MissionStep,
    PairedObservation,
    WorkbenchRequest,
    Workspace,
    PlanStatus,
    SourceKind,
    analytics_request,
)
from prism_sdk.errors import ArgumentError


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def observation() -> MetricObservation:
    return MetricObservation(
        id="verification-1",
        dimension="verification",
        domain="oncology",
        system="agent-a",
        value=0.8,
        direction=AnalyticsDirection.HIGHER_IS_BETTER,
        unit="fraction",
        condition="pack/4",
        replicate_group="world-1",
        cost=4.0,
        latency_ms=20.0,
        evidence=AnalyticsEvidence.REPRODUCED,
    )


class AnalyticsModelTests(unittest.TestCase):
    def test_biological_adapter_registry_distinguishes_dependency_states(self) -> None:
        request = AdapterPlanRequest(
            "scan-1",
            SourceKind.BYTES,
            declared_format="APPLICATION/DICOM",
            available_dependencies=["pydicom"],
        )
        plan = AdapterRegistry().plan(request, check_environment=False)
        self.assertTrue(plan.executable)
        self.assertEqual(plan.selected_adapter.id, "bioprism.python.dicom")

        unknown = AdapterRegistry().plan(
            AdapterPlanRequest("scan-1", SourceKind.BYTES, declared_format="application/dicom"),
            check_environment=False,
        )
        self.assertEqual(unknown.candidates[0].status, PlanStatus.DEPENDENCY_UNKNOWN)

    def test_biological_adapter_request_refuses_implicit_format_sniffing(self) -> None:
        with self.assertRaises(ArgumentError):
            AdapterPlanRequest("", SourceKind.BYTES)
        plan = AdapterRegistry().plan(
            AdapterPlanRequest("variants", SourceKind.BYTES, declared_format="application/octet-stream"),
            check_environment=False,
        )
        self.assertFalse(plan.executable)

    def test_models_emit_the_exact_rust_wire_shape(self) -> None:
        request = analytics_request(
            [observation()],
            pairs=[
                PairedObservation(
                    "robustness-1",
                    "robustness",
                    "oncology",
                    0.9,
                    0.72,
                    AnalyticsDirection.HIGHER_IS_BETTER,
                    0.2,
                )
            ],
            calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
            calibration_bins=5,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["observations"][0]["evidence"], "reproduced")
        self.assertEqual(arguments["pairs"][0]["direction"], "higher_is_better")
        self.assertEqual(arguments["calibration"][0]["predicted"], 0.9)
        self.assertEqual(arguments["calibration_bins"], 5)

    def test_models_fail_closed_on_probability_and_bin_bounds(self) -> None:
        with self.assertRaises(ArgumentError):
            CalibrationObservation("bad", "domain", 1.1, 0.5)
        with self.assertRaises(ArgumentError):
            analytics_request([], calibration_bins=1)

    def test_workbench_request_preserves_nested_contracts(self) -> None:
        request = WorkbenchRequest(
            {"session_id": "studio-1", "artifacts": [], "cells": [], "changes": []},
            dashboard={"include_holes": True},
            ci={"offline": True},
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["dashboard"]["include_holes"], True)
        self.assertEqual(arguments["ci"]["offline"], True)
        with self.assertRaises(ArgumentError):
            WorkbenchRequest({})

    def test_mission_request_builds_dependency_bound_wire_contract(self) -> None:
        request = MissionRequest(
            "mission-1",
            "compose evidence",
            [
                MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities"),
                MissionStep(
                    "metrics",
                    "metrics",
                    "analytics",
                    "prepare measurements",
                    "metrics_analytics_audit",
                    {"observations": [], "inputs": [None]},
                    ("catalog",),
                    True,
                    (MissionBinding("catalog", "/value", "/inputs/0"),),
                ),
            ],
            MissionPolicy(execute=True, allowed_tools=("workspace_capabilities", "metrics_analytics_audit")),
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["steps"][1]["depends_on"], ["catalog"])
        self.assertEqual(arguments["steps"][1]["bindings"][0]["target_pointer"], "/inputs/0")
        self.assertEqual(arguments["policy"]["allowed_tools"], ["workspace_capabilities", "metrics_analytics_audit"])
        with self.assertRaises(ArgumentError):
            MissionRequest("", "goal", [MissionStep("s", "d", "c", "o", "tool")])

    def test_capability_query_builds_bounded_cross_domain_wire_contract(self) -> None:
        query = CapabilityQuery(query="oncology evidence", max_items=3, include_tools=True)
        self.assertEqual(
            query.to_mcp_arguments(),
            {"query": "oncology evidence", "max_items": 3, "include_tools": True},
        )
        with self.assertRaises(ArgumentError):
            CapabilityQuery(max_items=0)

    def test_capability_route_request_batches_named_needs_without_execution(self) -> None:
        request = CapabilityRouteRequest(
            "compose evidence",
            [
                CapabilityRouteNeed("oncology", CapabilityQuery(query="oncology")),
                {"id": "release", "tool": "bundle_verify"},
            ],
            max_candidates_per_need=2,
            max_tools=4,
            include_tools=True,
        )
        arguments = request.to_mcp_arguments()
        self.assertEqual(arguments["needs"][1]["tool"], "bundle_verify")
        self.assertEqual(arguments["max_tools"], 4)
        with self.assertRaises(ArgumentError):
            CapabilityRouteRequest("goal", [{"id": "same"}, {"id": "same"}])


class AnalyticsWorkspaceTests(unittest.TestCase):
    def test_sync_workspace_sends_typed_analytics_request(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).metrics_analytics_audit(
                [observation()],
                calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
            )
        self.assertEqual(result["echo"]["observations"][0]["dimension"], "verification")
        self.assertEqual(result["echo"]["calibration"][0]["observed"], 1.0)

    def test_sync_workspace_exposes_workbench_composition(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).developer_workbench(
                {"session_id": "studio-1", "artifacts": [], "cells": [], "changes": []},
                dashboard={"include_holes": True},
                ci={"offline": True},
            )
        self.assertEqual(result["echo"]["session"]["session_id"], "studio-1")
        self.assertEqual(result["echo"]["ci"]["offline"], True)

    def test_sync_workspace_exposes_agent_mission(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).agent_mission(
                "mission-sync",
                "discover capabilities",
                [MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities")],
            )
        self.assertEqual(result["echo"]["mission_id"], "mission-sync")
        self.assertEqual(result["echo"]["steps"][0]["tool"], "workspace_capabilities")

    def test_sync_workspace_exposes_capability_discovery(self) -> None:
        with Client(command(), timeout=2) as client:
            with self.assertRaises(ArgumentError):
                Workspace(client).capability_discover(query=object())  # type: ignore[arg-type]
            result = Workspace(client).capability_discover(query="oncology")
        self.assertEqual(result["echo"]["query"], "oncology")
        self.assertEqual(result["echo"]["include_tools"], False)

    def test_sync_workspace_exposes_capability_audit(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).capability_audit(include_groups=False)
        self.assertEqual(result["echo"], {"include_groups": False})

    def test_sync_workspace_exposes_capability_route(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).capability_route(
                "compose evidence",
                [{"id": "oncology", "query": "oncology"}],
                max_tools=4,
            )
        self.assertEqual(result["echo"]["goal"], "compose evidence")
        self.assertEqual(result["echo"]["needs"][0]["id"], "oncology")

    def test_sync_workspace_exposes_adapter_planning(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).adapter_plan(
                "scan-1",
                "bytes",
                declared_format="application/dicom",
                available_dependencies=["pydicom"],
            )
        self.assertEqual(result["echo"]["source_id"], "scan-1")
        self.assertEqual(result["echo"]["available_dependencies"], ["pydicom"])


class AsyncAnalyticsWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_matches_sync_surface(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).metrics_analytics_audit(
                [observation()], calibration_bins=7
            )
        self.assertEqual(result["echo"]["calibration_bins"], 7)

    async def test_async_workspace_exposes_workbench(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).developer_workbench(
                {"session_id": "studio-async", "artifacts": [], "cells": [], "changes": []}
            )
        self.assertEqual(result["echo"]["session"]["session_id"], "studio-async")

    async def test_async_workspace_exposes_agent_mission(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).agent_mission(
                "mission-async",
                "discover capabilities",
                [MissionStep("catalog", "workspace", "discovery", "discover routes", "workspace_capabilities")],
            )
        self.assertEqual(result["echo"]["mission_id"], "mission-async")

    async def test_async_workspace_exposes_capability_discovery(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_discover(domain="release", max_items=2)
        self.assertEqual(result["echo"]["domain"], "release")
        self.assertEqual(result["echo"]["max_items"], 2)

    async def test_async_workspace_exposes_capability_audit(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_audit()
        self.assertEqual(result["echo"], {"include_groups": True})

    async def test_async_workspace_exposes_capability_route(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).capability_route(
                "compose evidence",
                [CapabilityRouteNeed("release", CapabilityQuery(tool="bundle_verify"))],
            )
        self.assertEqual(result["echo"]["needs"][0]["tool"], "bundle_verify")

    async def test_async_workspace_exposes_adapter_planning(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).adapter_plan(
                "variants-1",
                "bytes",
                declared_format="text/vcf",
                available_dependencies=["pysam"],
            )
        self.assertEqual(result["echo"]["declared_format"], "text/vcf")


if __name__ == "__main__":
    unittest.main()
