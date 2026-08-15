from __future__ import annotations

import asyncio
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest

from prism_sdk import ApiClient, ApiError, ArgumentError, AsyncApiClient, BioCapabilityEvidenceAuditRequest, ClaimRequest, EvidenceItem


class FakeApiHandler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, _format: str, *_args: object) -> None:
        return

    def _send(self, status: int, value: dict) -> None:
        body = json.dumps(value, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # noqa: N802
        if self.path == "/healthz":
            self._send(200, {"ok": True, "ready": True})
        elif self.path == "/v1/tools":
            self._send(200, {"tools": [{"name": "echo"}]})
        elif self.path.startswith("/v1/events"):
            self._send(200, {"ok": True, "page": {"events": [], "next_after": 0}})
        else:
            self._send(404, {"ok": False, "error": {"code": "not_found"}})

    def do_POST(self) -> None:  # noqa: N802
        size = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(size) or b"{}")
        if self.path == "/v1/tools/echo":
            self._send(200, {"ok": True, "tool": "echo", "mcp": {"result": body}})
        elif self.path.startswith("/v1/tools/capability_") or self.path == "/v1/tools/biocapability_evidence_audit":
            self._send(200, {"ok": True, "tool": self.path.rsplit("/", 1)[-1], "mcp": {"result": body}})
        elif self.path == "/v1/tools/adapter_plan":
            self._send(200, {"ok": True, "tool": "adapter_plan", "mcp": {"result": body}})
        else:
            self._send(422, {"ok": False, "error": {"code": "refused"}})


class HttpApiClientTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), FakeApiHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        host, port = cls.server.server_address
        cls.base_url = f"http://{host}:{port}"

    @classmethod
    def tearDownClass(cls) -> None:
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join(timeout=2)

    def test_http_health_tools_events_and_structured_errors(self) -> None:
        client = ApiClient(self.base_url, bearer_token="0123456789abcdef")
        self.assertTrue(client.health()["ready"])
        self.assertEqual(client.tools()[0]["name"], "echo")
        self.assertEqual(client.call_tool("echo", {"value": 3})["mcp"]["result"]["value"], 3)
        self.assertEqual(
            client.capability_discover(query="oncology")["mcp"]["result"]["query"],
            "oncology",
        )
        self.assertEqual(
            client.capability_audit(include_groups=False)["mcp"]["result"]["include_groups"],
            False,
        )
        self.assertEqual(
            client.capability_route("compose evidence", [{"id": "oncology", "query": "oncology"}])["mcp"]["result"]["goal"],
            "compose evidence",
        )
        self.assertEqual(
            client.adapter_plan("scan-1", "bytes", declared_format="application/dicom")["mcp"]["result"]["source_id"],
            "scan-1",
        )
        evidence_request = BioCapabilityEvidenceAuditRequest(
            [EvidenceItem("grounding", "evidence_grounding", "observed", support={"source": "ledger", "scope": "pack/1"})],
            [ClaimRequest("claim", "grounded profile", ("evidence_grounding",))],
            vectors=({"system": "a"}, {"system": "b"}),
        )
        self.assertEqual(
            client.biocapability_evidence_audit(evidence_request)["mcp"]["result"]["claim_requests"][0]["id"],
            "claim",
        )
        self.assertEqual(client.events()["page"]["events"], [])
        with self.assertRaises(ApiError) as error:
            client.request("POST", "/v1/tools/refuse", {})
        self.assertEqual(error.exception.status, 422)

    def test_http_arguments_and_async_facade(self) -> None:
        with self.assertRaises(ArgumentError):
            ApiClient(self.base_url, bearer_token="short")
        client = AsyncApiClient(ApiClient(self.base_url))

        async def run() -> None:
            self.assertTrue((await client.health())["ok"])
            self.assertEqual((await client.call_tool("echo", {"async": True}))["tool"], "echo")
            self.assertEqual(
                (await client.capability_route("async route", [{"id": "release", "tool": "bundle_verify"}]))["mcp"]["result"]["goal"],
                "async route",
            )
            self.assertEqual(
                (await client.adapter_plan("variants", "bytes", declared_format="text/vcf"))["mcp"]["result"]["declared_format"],
                "text/vcf",
            )
            evidence_request = BioCapabilityEvidenceAuditRequest(
                [EvidenceItem("grounding", "evidence_grounding", "observed", support={"source": "ledger", "scope": "pack/1"})],
                [ClaimRequest("claim", "grounded profile", ("evidence_grounding",))],
                vectors=({"system": "a"}, {"system": "b"}),
            )
            self.assertEqual(
                (await client.biocapability_evidence_audit(evidence_request))["mcp"]["result"]["max_items"],
                100,
            )

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
