from __future__ import annotations

import asyncio
import sys
from pathlib import Path
import unittest

from prism_sdk import (
    ArgumentError,
    AsyncClient,
    AsyncWorkspace,
    Client,
    RepositoryBundleRequest,
    RepositoryCatalogRequest,
    RepositoryImpactRequest,
    RepositoryTraversalPolicy,
    TelemetryProjectRequest,
    Workspace,
)


FAKE = Path(__file__).parent / "fake_mcp_server.py"


EVENT = {"kind": "job.completed", "fields": {"status": {"value": "ok", "class": "public"}}}
POLICY = {"treatments": {"public": {"action": "emit"}}}


class RepositoryRequestTests(unittest.TestCase):
    def test_repository_and_telemetry_requests_bound_shape_and_disclosure(self) -> None:
        self.assertEqual(
            RepositoryCatalogRequest("docs/", 10, True, True).to_mcp_arguments(),
            {"prefix": "docs/", "limit": 10, "include_briefs": True, "include_findings": True},
        )
        bundle = RepositoryBundleRequest(
            {"id": "route-1", "must_read": ["docs/README"]},
            RepositoryTraversalPolicy.EXHAUSTIVE,
            4,
            ("protected",),
            ("requires", "references"),
            True,
            100_000,
        )
        self.assertEqual(bundle.to_mcp_arguments()["policy"], "exhaustive")
        self.assertEqual(bundle.to_mcp_arguments()["max_markdown_chars"], 100_000)
        self.assertEqual(
            RepositoryImpactRequest("docs/README", route={"id": "route-1"}).to_mcp_arguments()["route"]["id"],
            "route-1",
        )
        self.assertEqual(
            TelemetryProjectRequest(EVENT, POLICY, "trace-1", metric={"kind": "ratio"}, observations={"n": 1}).to_mcp_arguments()["trace"],
            "trace-1",
        )
        with self.assertRaises(ArgumentError):
            RepositoryCatalogRequest("../private", 10)
        with self.assertRaises(ArgumentError):
            RepositoryImpactRequest("docs/README", route={"id": "one"}, routes=[{"id": "two"}])
        with self.assertRaises(ArgumentError):
            TelemetryProjectRequest(EVENT, POLICY, "trace-1", metric={"kind": "ratio"})
        with self.assertRaises(ArgumentError):
            RepositoryBundleRequest({"id": "route"}, max_markdown_chars=0)

    def test_sync_and_async_workspace_expose_repository_and_telemetry_domains(self) -> None:
        with Client([sys.executable, "-u", str(FAKE)], timeout=2) as client:
            workspace = Workspace(client)
            catalog = workspace.repository_catalog(prefix="docs/", limit=5)
            bundle = workspace.repository_bundle({"id": "route-1"}, policy="exhaustive", max_depth=2)
            impact = workspace.repository_impact("docs/README", routes=[{"id": "route-1"}])
            telemetry = workspace.telemetry_project(EVENT, POLICY, "trace-1")
        self.assertEqual(catalog["echo"]["prefix"], "docs/")
        self.assertEqual(bundle["echo"]["policy"], "exhaustive")
        self.assertEqual(impact["echo"]["changed"], "docs/README")
        self.assertEqual(telemetry["echo"]["trace"], "trace-1")

    async def _async_workflow(self) -> None:
        async with AsyncClient([sys.executable, "-u", str(FAKE)], timeout=2) as client:
            workspace = AsyncWorkspace(client)
            catalog = await workspace.repository_catalog(RepositoryCatalogRequest(limit=3))
            bundle = await workspace.repository_bundle(RepositoryBundleRequest({"id": "route-2"}))
            impact = await workspace.repository_impact(RepositoryImpactRequest("docs/README"))
            telemetry = await workspace.telemetry_project(TelemetryProjectRequest(EVENT, POLICY, "trace-2"))
        self.assertEqual(catalog["echo"]["limit"], 3)
        self.assertEqual(bundle["echo"]["route"]["id"], "route-2")
        self.assertEqual(impact["echo"]["changed"], "docs/README")
        self.assertEqual(telemetry["echo"]["trace"], "trace-2")

    def test_async_workspace_mirrors_the_domain_helpers(self) -> None:
        asyncio.run(self._async_workflow())


if __name__ == "__main__":
    unittest.main()
