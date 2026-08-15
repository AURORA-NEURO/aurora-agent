from __future__ import annotations

import asyncio
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest

from prism_sdk import ApiClient, ApiError, ArgumentError, AsyncApiClient, BioCapabilityEvidenceAuditRequest, BioQlCompileRequest, ClaimRequest, EvidenceItem, LabPlanRequest, MissionRequest, MissionStep, RoutingDecisionRequest, WorldClaimCheckRequest


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
            self._send(
                200,
                {
                    "tools": [
                        {
                            "name": "echo",
                            "inputSchema": {
                                "type": "object",
                                "properties": {"value": {"type": "integer"}},
                            },
                        }
                    ]
                },
            )
        elif self.path.startswith("/v1/events"):
            self._send(200, {"ok": True, "page": {"events": [], "next_after": 0}})
        elif self.path == "/v1/missions/async-1":
            self._send(200, {"ok": True, "mission_id": "async-1", "status": "succeeded", "cancel_requested": False, "result": {"mission_status": "succeeded"}})
        else:
            self._send(404, {"ok": False, "error": {"code": "not_found"}})

    def do_POST(self) -> None:  # noqa: N802
        size = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(size) or b"{}")
        if self.path == "/v1/missions":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "queued", "cancel_requested": False})
        elif self.path == "/v1/missions/async-1/cancel":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "running", "cancel_requested": True, "cancel_reason": body.get("reason")})
        elif self.path == "/v1/tools/echo":
            self._send(200, {"ok": True, "tool": "echo", "mcp": {"result": body}})
        elif self.path.startswith("/v1/tools/capability_") or self.path in {"/v1/tools/biocapability_evidence_audit", "/v1/tools/bioql_compile", "/v1/tools/world_claim_check", "/v1/tools/lab_plan", "/v1/tools/routing_decide", "/v1/tools/fiber_compile", "/v1/tools/fiber_refine", "/v1/tools/fiber_explain", "/v1/tools/fiber_verify", "/v1/tools/projection_bundle", "/v1/tools/repository_catalog", "/v1/tools/repository_bundle", "/v1/tools/repository_impact", "/v1/tools/telemetry_project"}:
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
        catalogue = client.tool_catalogue()
        self.assertEqual(client.plan_tool("echo", {"value": 3}, catalogue=catalogue).tool, "echo")
        self.assertEqual(client.tool_checked("echo", {"value": 3}, catalogue=catalogue)["mcp"]["result"]["value"], 3)
        with self.assertRaises(ArgumentError):
            client.plan_tool("echo", {"value": "not-an-integer"}, catalogue=catalogue)
        mission = client.mission_preflight(
            MissionRequest(
                "mission-http",
                "check",
                [MissionStep("one", "data", "read", "check", "echo", {"value": 3})],
            ),
            catalogue=catalogue,
        )
        self.assertTrue(mission.ok)
        self.assertEqual(client.call_tool("echo", {"value": 3})["mcp"]["result"]["value"], 3)
        submitted = client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))
        self.assertEqual(submitted.status, "queued")
        self.assertEqual(client.mission_status("async-1").result["mission_status"], "succeeded")
        self.assertTrue(client.cancel_mission("async-1", "operator stop").cancel_requested)
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
        self.assertEqual(
            client.bioql_compile(BioQlCompileRequest("SELECT sample.id", {"schema_version": "v1"}))["mcp"]["result"]["query"],
            "SELECT sample.id",
        )
        self.assertEqual(
            client.world_claim_check(WorldClaimCheckRequest({"top": "observed"}, {"kind": "biology"}))["mcp"]["result"]["provenance"]["top"],
            "observed",
        )
        self.assertEqual(
            client.lab_plan(LabPlanRequest({"obligations": []}, [{"id": "assay"}], {"tokens": 1}))["mcp"]["result"]["actions"][0]["id"],
            "assay",
        )
        self.assertEqual(
            client.routing_decide(RoutingDecisionRequest({"features": {}}, [{"task_id": "other"}], {"safe_default": "abstain"}))["mcp"]["result"]["policy"]["safe_default"],
            "abstain",
        )
        self.assertEqual(
            client.fiber_compile("world.json", "query.json", layer="l1")["mcp"]["result"]["layer"],
            "l1",
        )
        self.assertEqual(
            client.fiber_refine("l2", handle={"digest": "compiled"})["mcp"]["result"]["handle"]["digest"],
            "compiled",
        )
        self.assertEqual(
            client.fiber_explain("world.json", "query.json")["mcp"]["result"]["query"],
            "query.json",
        )
        self.assertEqual(
            client.fiber_verify("certificate.json")["mcp"]["result"]["certificate"],
            "certificate.json",
        )
        self.assertTrue(
            client.projection_bundle("world.json", "query.json", include_views=True)["mcp"]["result"]["include_views"]
        )
        self.assertEqual(
            client.repository_catalog(prefix="docs/", limit=3)["mcp"]["result"]["prefix"],
            "docs/",
        )
        self.assertEqual(
            client.repository_bundle({"id": "route-1"}, policy="exhaustive")["mcp"]["result"]["policy"],
            "exhaustive",
        )
        self.assertEqual(
            client.repository_impact("docs/README")["mcp"]["result"]["changed"],
            "docs/README",
        )
        self.assertEqual(
            client.telemetry_project({"kind": "event"}, {"treatments": {}}, "trace-http")["mcp"]["result"]["trace"],
            "trace-http",
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
            catalogue = await client.tool_catalogue()
            self.assertEqual((await client.plan_tool("echo", {"value": 5}, catalogue=catalogue)).tool, "echo")
            self.assertEqual((await client.tool_checked("echo", {"value": 5}, catalogue=catalogue))["mcp"]["result"]["value"], 5)
            mission = await client.mission_preflight(
                MissionRequest(
                    "mission-http-async",
                    "check",
                    [MissionStep("one", "data", "read", "check", "echo", {"value": 5})],
                ),
                catalogue=catalogue,
            )
            self.assertTrue(mission.fully_checked)
            self.assertEqual((await client.call_tool("echo", {"async": True}))["tool"], "echo")
            self.assertEqual((await client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))).status, "queued")
            self.assertEqual((await client.mission_status("async-1")).status, "succeeded")
            self.assertTrue((await client.cancel_mission("async-1", "operator stop")).cancel_requested)
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
            self.assertEqual(
                (await client.bioql_compile("SELECT sample.id", {"schema_version": "v1"}))["mcp"]["result"]["schema"]["schema_version"],
                "v1",
            )
            self.assertEqual(
                (await client.routing_decide({"features": {}}, [{"task_id": "other"}], {"safe_default": "abstain"}, task_id="new"))["mcp"]["result"]["task_id"],
                "new",
            )
            self.assertEqual(
                (await client.fiber_compile("world.json", "query.json"))["mcp"]["result"]["layer"],
                "l0",
            )
            self.assertEqual(
                (await client.fiber_refine("l1", handle={"digest": "async"}))["mcp"]["result"]["handle"]["digest"],
                "async",
            )
            self.assertEqual(
                (await client.fiber_explain("world.json", "query.json"))["mcp"]["result"]["world"],
                "world.json",
            )
            self.assertEqual(
                (await client.fiber_verify("certificate.json"))["mcp"]["result"]["certificate"],
                "certificate.json",
            )
            self.assertFalse(
                (await client.projection_bundle("world.json", "query.json"))["mcp"]["result"]["include_views"]
            )
            self.assertEqual(
                (await client.repository_catalog(limit=2))["mcp"]["result"]["limit"],
                2,
            )
            self.assertEqual(
                (await client.repository_bundle({"id": "route-async"}))["mcp"]["result"]["route"]["id"],
                "route-async",
            )
            self.assertEqual(
                (await client.repository_impact("docs/README"))["mcp"]["result"]["changed"],
                "docs/README",
            )
            self.assertEqual(
                (await client.telemetry_project({"kind": "event"}, {"treatments": {}}, "trace-async"))["mcp"]["result"]["trace"],
                "trace-async",
            )

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
