from __future__ import annotations

from pathlib import Path
import sys
import unittest

from prism_sdk import (
    ArgumentError,
    AsyncClient,
    AsyncWorkspace,
    Client,
    LifecycleError,
    RemoteError,
    ToolRefusal,
    Workspace,
)


ROOT = Path(__file__).parent
FAKE = ROOT / "fake_mcp_server.py"


def command() -> list[str]:
    return [sys.executable, "-u", str(FAKE)]


class SyncClientTests(unittest.TestCase):
    def test_lifecycle_tools_resources_and_domain_facade(self) -> None:
        client = Client(command(), timeout=2)
        with self.assertRaises(LifecycleError):
            client.list_tools()
        with client:
            self.assertEqual(client.session.protocol_version, "2025-06-18")
            tools = client.list_tools()
            self.assertEqual(tools[0]["name"], "echo")
            result = client.call_tool("echo", {"value": 3})
            self.assertFalse(result.is_error)
            self.assertEqual(result.require_ok()["echo"]["value"], 3)
            self.assertEqual(client.read_resource("test://resource")["contents"][0]["text"], "{}")
            report = Workspace(client).developer_delivery_audit(
                request_id="sdk-test",
                targets=["developer_platform"],
            )
            self.assertEqual(report["echo"]["release_request"]["id"], "sdk-test")
            otel = Workspace(client).trace_otel_ingest(
                "otel-test",
                otlp_json='{"resourceSpans":[]}',
                include_events=True,
            )
            self.assertEqual(otel["echo"]["trace_id"], "otel-test")

        self.assertFalse(client.running)

    def test_remote_error_and_structured_refusal_are_distinct(self) -> None:
        with Client(command(), timeout=2) as client:
            with self.assertRaises(RemoteError) as remote:
                client.call_tool("remote_error")
            self.assertEqual(remote.exception.code, -32001)

            with self.assertRaises(ToolRefusal) as refusal:
                client.call_tool("refuse").require_ok()
            self.assertEqual(refusal.exception.payload["fail_closed"], True)

    def test_arguments_and_frame_bounds_fail_before_transport(self) -> None:
        with self.assertRaises(ArgumentError):
            Client([], timeout=2)
        with Client(command(), timeout=2, max_frame_bytes=256) as client:
            with self.assertRaises(ArgumentError):
                client.call_tool("echo", {"large": "x" * 200})


class AsyncClientTests(unittest.IsolatedAsyncioTestCase):
    async def test_async_lifecycle_and_facade(self) -> None:
        async with AsyncClient(command(), timeout=2) as client:
            self.assertEqual((await client.list_tools())[0]["name"], "echo")
            result = await client.call_tool("echo", {"async": True})
            self.assertEqual(result.require_ok()["echo"]["async"], True)
            report = await AsyncWorkspace(client).developer_delivery_audit(
                request_id="async-test",
                targets=["developer_platform"],
            )
            self.assertEqual(report["echo"]["release_request"]["id"], "async-test")
            context = await AsyncWorkspace(client).compile_context(
                {"world": "fixture"},
                {"query": "fixture"},
                include_views=True,
            )
            self.assertTrue(context["echo"]["include_views"])
            otel = await AsyncWorkspace(client).trace_otel_ingest(
                "async-otel-test",
                document="fixtures/trace.json",
            )
            self.assertEqual(otel["echo"]["document"], "fixtures/trace.json")


if __name__ == "__main__":
    unittest.main()
