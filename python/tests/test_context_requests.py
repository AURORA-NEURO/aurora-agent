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
    ContextLayer,
    FiberCompileRequest,
    FiberExplainRequest,
    FiberRefineRequest,
    FiberVerifyRequest,
    ProjectionBundleRequest,
    Workspace,
)


FAKE = Path(__file__).parent / "fake_mcp_server.py"


class ContextRequestTests(unittest.TestCase):
    def test_requests_encode_progressive_disclosure_and_mutual_exclusion(self) -> None:
        compile_request = FiberCompileRequest("world.json", "query.json", ContextLayer.L2)
        self.assertEqual(
            compile_request.to_mcp_arguments(),
            {"world": "world.json", "query": "query.json", "layer": "l2"},
        )
        self.assertEqual(
            FiberRefineRequest("l3", handle={"digest": "abc"}).to_mcp_arguments(),
            {"layer": "l3", "handle": {"digest": "abc"}},
        )
        self.assertEqual(
            FiberRefineRequest("l1", world="world.json", query="query.json").to_mcp_arguments(),
            {"layer": "l1", "world": "world.json", "query": "query.json"},
        )
        self.assertEqual(
            ProjectionBundleRequest(world="world.json", query="query.json", include_views=True).to_mcp_arguments(),
            {"world": "world.json", "query": "query.json", "include_views": True},
        )
        self.assertEqual(FiberExplainRequest("world.json", "query.json").to_mcp_arguments()["query"], "query.json")
        self.assertEqual(FiberVerifyRequest("certificate.json").to_mcp_arguments()["certificate"], "certificate.json")
        with self.assertRaises(ArgumentError):
            FiberRefineRequest("l2")
        with self.assertRaises(ArgumentError):
            FiberRefineRequest("l2", handle={"digest": "abc"}, world="world.json", query="query.json")
        with self.assertRaises(ArgumentError):
            ProjectionBundleRequest(world="world.json", query="query.json", include_views=1)  # type: ignore[arg-type]
        with self.assertRaises(ArgumentError):
            FiberCompileRequest("..\\outside.json", "query.json")

    def test_sync_workspace_exposes_compile_refine_explain_verify_and_projection(self) -> None:
        with Client([sys.executable, "-u", str(FAKE)], timeout=2) as client:
            workspace = Workspace(client)
            compiled = workspace.fiber_compile("world.json", "query.json", layer="l1")
            refined = workspace.fiber_refine("l2", handle={"digest": "compiled"})
            explained = workspace.fiber_explain("world.json", "query.json")
            verified = workspace.fiber_verify("certificate.json")
            projected = workspace.projection_bundle("world.json", "query.json", include_views=True)

        self.assertEqual(compiled["echo"]["layer"], "l1")
        self.assertEqual(refined["echo"]["handle"]["digest"], "compiled")
        self.assertEqual(explained["echo"]["world"], "world.json")
        self.assertEqual(verified["echo"]["certificate"], "certificate.json")
        self.assertTrue(projected["echo"]["include_views"])

    async def _async_workflow(self) -> None:
        async with AsyncClient([sys.executable, "-u", str(FAKE)], timeout=2) as client:
            workspace = AsyncWorkspace(client)
            compiled = await workspace.context_compile(FiberCompileRequest("world.json", "query.json"))
            refined = await workspace.context_refine(FiberRefineRequest("l2", handle={"digest": "async"}))
            explained = await workspace.context_explain(FiberExplainRequest("world.json", "query.json"))
            verified = await workspace.context_verify(FiberVerifyRequest("certificate.json"))
            projected = await workspace.projection_bundle(
                ProjectionBundleRequest(handle={"digest": "async"}, include_views=False)
            )
        self.assertEqual(compiled["echo"]["layer"], "l0")
        self.assertEqual(refined["echo"]["handle"]["digest"], "async")
        self.assertEqual(explained["echo"]["query"], "query.json")
        self.assertEqual(verified["echo"]["certificate"], "certificate.json")
        self.assertFalse(projected["echo"]["include_views"])

    def test_async_workspace_mirrors_the_full_lifecycle(self) -> None:
        asyncio.run(self._async_workflow())


if __name__ == "__main__":
    unittest.main()
