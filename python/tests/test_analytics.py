from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    AnalyticsDirection,
    AnalyticsEvidence,
    AsyncClient,
    AsyncWorkspace,
    CalibrationObservation,
    Client,
    MetricObservation,
    PairedObservation,
    Workspace,
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


class AnalyticsWorkspaceTests(unittest.TestCase):
    def test_sync_workspace_sends_typed_analytics_request(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).metrics_analytics_audit(
                [observation()],
                calibration=[CalibrationObservation("forecast-1", "oncology", 0.9, 1.0)],
            )
        self.assertEqual(result["echo"]["observations"][0]["dimension"], "verification")
        self.assertEqual(result["echo"]["calibration"][0]["observed"], 1.0)


class AsyncAnalyticsWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_matches_sync_surface(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).metrics_analytics_audit(
                [observation()], calibration_bins=7
            )
        self.assertEqual(result["echo"]["calibration_bins"], 7)


if __name__ == "__main__":
    unittest.main()
