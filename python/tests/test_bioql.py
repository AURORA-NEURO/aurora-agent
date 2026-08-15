from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import AsyncClient, AsyncWorkspace, BioQlCompileRequest, Client, Workspace
from prism_sdk.errors import ArgumentError


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


def request() -> BioQlCompileRequest:
    return BioQlCompileRequest(
        "SELECT sample.id, sample.expression FROM samples AS sample WHERE sample.expression > 0",
        {
            "schema_version": "biolang-schema/0.1",
            "collections": {
                "samples": {
                    "fields": {
                        "id": {"type": "identifier"},
                        "expression": {"type": "quantity", "unit": "fraction", "frame": "sample"},
                    }
                }
            },
        },
    )


class BioQlModelTests(unittest.TestCase):
    def test_request_preserves_query_and_explicit_schema(self) -> None:
        arguments = request().to_mcp_arguments()

        self.assertIn("SELECT sample.id", arguments["query"])
        self.assertEqual(arguments["schema"]["collections"]["samples"]["fields"]["id"]["type"], "identifier")

    def test_request_rejects_empty_oversized_or_non_json_schema(self) -> None:
        with self.assertRaises(ArgumentError):
            BioQlCompileRequest(" ", {})
        with self.assertRaises(ArgumentError):
            BioQlCompileRequest("x" * 1_000_001, {})
        with self.assertRaises(ArgumentError):
            BioQlCompileRequest("x", {"bad": float("nan")})
        with self.assertRaises(ArgumentError):
            BioQlCompileRequest("x", [])  # type: ignore[arg-type]

    def test_sync_workspace_compiles_against_an_explicit_schema(self) -> None:
        with Client(command(), timeout=2) as client:
            result = Workspace(client).bioql_compile(request())

        self.assertEqual(result["echo"]["schema"]["schema_version"], "biolang-schema/0.1")


class AsyncBioQlWorkspaceTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_workspace_compiles_against_an_explicit_schema(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            result = await AsyncWorkspace(client).bioql_compile(request().query, request().schema)

        self.assertIn("SELECT sample.id", result["echo"]["query"])


if __name__ == "__main__":
    unittest.main()
