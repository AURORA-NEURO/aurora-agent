from __future__ import annotations

import asyncio
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest

from prism_sdk import ApiClient, ApiError, ArgumentError, AsyncApiClient, BioCapabilityEvidenceAuditRequest, BioQlCompileRequest, ClaimRequest, DeliveryPage, EventPage, EventPersistenceStatus, EvidenceItem, LabPlanRequest, MissionInventoryPage, MissionPersistenceStatus, MissionRequest, MissionStep, MissionWaitTimeout, RouteReviewEvidence, RoutingDecisionRequest, SseSnapshot, WorldClaimCheckRequest


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
        elif self.path.startswith("/v1/route-reviews/"):
            review_id = "a" * 64
            self._send(200, {"ok": True, "workflow": "capability_route_review_evidence", "review_id": review_id, "found": True, "page": {"events": [{"id": 1, "event_type": "tool.completed", "subject": "capability_route_review", "request_id": "req-1", "payload": {}}], "after": 0, "next_after": 1, "oldest": 1, "newest": 1, "gap": False, "dropped_events": 0}})
        elif self.path.startswith("/v1/events/stream"):
            body = b'id: 1\nevent: mission.trace\ndata: {"mission_id":"async-1"}\n\n'
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream; charset=utf-8")
            self.send_header("X-Next-After", "1")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == "/v1/events/persistence":
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "retained_events": 2, "next_event_id": 3, "dropped_events": 0, "subscriptions_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "events restore with cursor continuity; subscriptions and deliveries must be re-established", "flush": "/v1/events/persistence/flush"})
        elif self.path.startswith("/v1/events"):
            self._send(200, {"ok": True, "page": {"events": [], "after": 0, "next_after": 0, "oldest": None, "newest": None, "gap": False, "dropped_events": 0}})
        elif self.path.startswith("/v1/missions?"):
            self._send(200, {"ok": True, "missions": [{"mission_id": "async-1", "status": "succeeded", "cancel_requested": False, "progress": {"phase": "succeeded", "current_wave": 0, "total_steps": 1, "completed_steps": 1, "active_steps": 0, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "trace_sequence": 4, "last_event": "mission.completed"}, "summary": {"total_steps": 1, "completed_steps": 1, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "result_available": True}, "poll": "/v1/missions/async-1", "cancel": "/v1/missions/async-1/cancel", "trace": "/v1/missions/async-1/trace"}], "returned": 1, "total_matching": 1, "limit": 5, "truncated": False, "status_filter": "succeeded"})
        elif self.path == "/v1/missions/persistence":
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "max_result_bytes": 256 * 1024, "registry_size": 1, "event_log_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "terminal snapshots restore; queued and running jobs fail explicitly after restart", "flush": "/v1/missions/persistence/flush"})
        elif self.path.startswith("/v1/missions/async-1/trace"):
            self._send(200, {"ok": True, "mission_id": "async-1", "trace_schema_version": "bioprism-devplat-mission-trace/0.1", "events": [{"sequence": 0, "event": "mission.started", "wave": None, "step_id": None, "tool": None, "status": "running", "arguments_digest": None, "bytes": 0, "detail": None}, {"sequence": 1, "event": "mission.completed", "wave": None, "step_id": None, "tool": None, "status": "succeeded", "arguments_digest": None, "bytes": 14, "detail": None}], "after": 0, "next_after": 2, "oldest": 0, "newest": 1, "gap": False, "dropped_events": 0, "terminal": True, "limit": 100, "truncated": False})
        elif self.path == "/v1/missions/async-1":
            self._send(200, {"ok": True, "mission_id": "async-1", "status": "succeeded", "cancel_requested": False, "progress": {"phase": "succeeded", "current_wave": 0, "total_steps": 1, "completed_steps": 1, "active_steps": 0, "succeeded": 1, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 14, "trace_sequence": 4, "last_event": "mission.completed"}, "result": {"mission_status": "succeeded"}})
        elif self.path == "/v1/missions/slow":
            self._send(200, {"ok": True, "mission_id": "slow", "status": "running", "cancel_requested": False, "progress": {"phase": "running", "current_wave": 0, "total_steps": 1, "completed_steps": 0, "active_steps": 1, "succeeded": 0, "refused": 0, "blocked": 0, "cancelled": 0, "required_failures": 0, "returned_bytes": 0, "trace_sequence": 1, "last_event": "step.started"}})
        else:
            self._send(404, {"ok": False, "error": {"code": "not_found"}})

    def do_POST(self) -> None:  # noqa: N802
        size = int(self.headers.get("Content-Length", "0"))
        body = json.loads(self.rfile.read(size) or b"{}")
        if self.path == "/v1/missions/preflight":
            self._send(200, {"ok": True, "workflow": "agent_mission", "execution": "planned", "mission_status": "planned", "preflight": True, "dispatch": "not_started", "results": []})
        elif self.path == "/v1/missions":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "queued", "cancel_requested": False})
        elif self.path == "/v1/missions/async-1/cancel":
            self._send(202, {"ok": True, "mission_id": "async-1", "status": "running", "cancel_requested": True, "cancel_reason": body.get("reason")})
        elif self.path in {"/v1/missions/persistence/flush", "/v1/events/persistence/flush"}:
            self._send(200, {"ok": True, "enabled": True, "file_present": True, "file_bytes": 128, "schema_version": 1, "max_file_bytes": 64 * 1024 * 1024, "max_result_bytes": 256 * 1024, "registry_size": 1, "retained_events": 2, "next_event_id": 3, "dropped_events": 0, "event_log_durable": False, "subscriptions_durable": False, "webhook_deliveries_durable": False, "recovery_policy": "events restore with cursor continuity; subscriptions and deliveries must be re-established", "flush": self.path})
        elif self.path == "/v1/tools/echo":
            self._send(200, {"ok": True, "tool": "echo", "mcp": {"result": body}})
        elif self.path.startswith("/v1/tools/capability_") or self.path in {"/v1/tools/biocapability_evidence_audit", "/v1/tools/bioql_compile", "/v1/tools/world_claim_check", "/v1/tools/lab_plan", "/v1/tools/routing_decide", "/v1/tools/fiber_compile", "/v1/tools/fiber_refine", "/v1/tools/fiber_explain", "/v1/tools/fiber_verify", "/v1/tools/projection_bundle", "/v1/tools/repository_catalog", "/v1/tools/repository_bundle", "/v1/tools/repository_impact", "/v1/tools/telemetry_project"}:
            self._send(200, {"ok": True, "tool": self.path.rsplit("/", 1)[-1], "mcp": {"result": body}})
        elif self.path == "/v1/tools/adapter_plan":
            self._send(200, {"ok": True, "tool": "adapter_plan", "mcp": {"result": body}})
        elif self.path.endswith("/replay"):
            self._send(200, {"ok": True, "replayed": [{"delivery_id": 1, "subscription_id": "sub", "attempt": 1, "state": "pending", "last_error": None, "last_error_retryable": None, "event_id": 2, "event_type": "tool.completed", "signature": "sha256=x", "envelope": {}}]})
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

    def test_delivery_failure_state_is_typed_and_replay_is_explicit(self) -> None:
        page = DeliveryPage.from_wire({
            "ok": True,
            "page": {
                "deliveries": [{
                    "delivery_id": 1,
                    "subscription_id": "sub",
                    "attempt": 1,
                    "state": "failed",
                    "last_error": "blocked",
                    "last_error_retryable": False,
                    "event_id": 2,
                    "event_type": "tool.completed",
                    "signature": "sha256=x",
                    "envelope": {},
                }],
                "after": 0,
                "next_after": 1,
                "pending_count": 1,
                "dropped_deliveries": 0,
            },
        })
        self.assertEqual(page.deliveries[0].state, "failed")
        self.assertEqual(page.deliveries[0].last_error, "blocked")
        self.assertIs(page.deliveries[0].last_error_retryable, False)
        replayed = ApiClient(self.base_url).replay("sub", [1])
        self.assertEqual(replayed["replayed"][0]["state"], "pending")

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
        remote_preflight = client.preflight_mission(
            MissionRequest(
                "mission-http-remote-preflight",
                "check",
                [MissionStep("one", "data", "read", "check", "echo", {"value": 3})],
            )
        )
        self.assertTrue(remote_preflight["preflight"])
        self.assertEqual(remote_preflight["dispatch"], "not_started")
        self.assertEqual(client.call_tool("echo", {"value": 3})["mcp"]["result"]["value"], 3)
        submitted = client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))
        self.assertEqual(submitted.status, "queued")
        status = client.mission_status("async-1")
        self.assertEqual(status.result["mission_status"], "succeeded")
        self.assertIsNotNone(status.progress)
        self.assertEqual(status.progress.phase, "succeeded")
        self.assertEqual(status.progress.completed_steps, 1)
        self.assertEqual(status.progress.last_event, "mission.completed")
        waited = client.wait_mission("async-1", timeout=1.0, poll_interval=0.01)
        self.assertEqual(waited.status, "succeeded")
        trace = client.mission_trace("async-1")
        self.assertEqual(trace.events[0].event, "mission.started")
        self.assertEqual(trace.events[-1].event, "mission.completed")
        self.assertEqual(trace.next_after, 2)
        with self.assertRaises(ArgumentError):
            client.mission_trace("async-1", after=-1)
        inventory = client.missions(status="succeeded", limit=5)
        self.assertEqual(inventory["missions"][0]["mission_id"], "async-1")
        typed_inventory = client.mission_inventory(status="succeeded", limit=5)
        self.assertIsInstance(typed_inventory, MissionInventoryPage)
        self.assertTrue(typed_inventory.missions[0].terminal)
        self.assertEqual(typed_inventory.missions[0].progress.completed_steps, 1)
        self.assertIsInstance(client.mission_persistence(), MissionPersistenceStatus)
        self.assertIsInstance(client.flush_mission_persistence(), MissionPersistenceStatus)
        with self.assertRaises(ArgumentError):
            client.wait_mission("async-1", timeout=0)
        with self.assertRaises(MissionWaitTimeout) as wait_error:
            client.wait_mission("slow", timeout=0.01, poll_interval=0.01)
        self.assertEqual(wait_error.exception.last_job.status, "running")
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
        event_page = client.event_page()
        self.assertIsInstance(event_page, EventPage)
        self.assertFalse(event_page.gap)
        stream = client.event_stream()
        self.assertIsInstance(stream, SseSnapshot)
        self.assertEqual(stream.next_after, 1)
        self.assertEqual(stream.events[0].event, "mission.trace")
        evidence = client.route_review_evidence("a" * 64)
        self.assertIsInstance(evidence, RouteReviewEvidence)
        self.assertTrue(evidence.found)
        self.assertEqual(evidence.page.events[0].subject, "capability_route_review")
        with self.assertRaises(ArgumentError):
            client.route_review_evidence("invalid")
        self.assertIsInstance(client.event_persistence(), EventPersistenceStatus)
        self.assertIsInstance(client.flush_event_persistence(), EventPersistenceStatus)
        with self.assertRaises(ArgumentError):
            client.event_page(after=True)
        with self.assertRaises(ApiError) as error:
            client.request("POST", "/v1/tools/refuse", {})
        self.assertEqual(error.exception.status, 422)

    def test_http_arguments_and_async_facade(self) -> None:
        with self.assertRaises(ArgumentError):
            ApiClient(self.base_url, bearer_token="short")
        client = AsyncApiClient(ApiClient(self.base_url))

        async def run() -> None:
            self.assertTrue((await client.health())["ok"])
            self.assertEqual((await client.replay("sub", [1]))["replayed"][0]["state"], "pending")
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
            remote_preflight = await client.preflight_mission(
                MissionRequest(
                    "mission-http-async-remote-preflight",
                    "check",
                    [MissionStep("one", "data", "read", "check", "echo", {"value": 5})],
                )
            )
            self.assertTrue(remote_preflight["preflight"])
            self.assertEqual(remote_preflight["dispatch"], "not_started")
            self.assertEqual((await client.call_tool("echo", {"async": True}))["tool"], "echo")
            self.assertEqual((await client.submit_mission(MissionRequest("async-1", "run", [MissionStep("one", "data", "read", "run", "echo", {"value": 1})]))).status, "queued")
            status = await client.mission_status("async-1")
            self.assertEqual(status.status, "succeeded")
            self.assertIsNotNone(status.progress)
            self.assertEqual(status.progress.phase, "succeeded")
            waited = await client.wait_mission("async-1", timeout=1.0, poll_interval=0.01)
            self.assertEqual(waited.status, "succeeded")
            trace = await client.mission_trace("async-1")
            self.assertEqual(trace.events[-1].event, "mission.completed")
            inventory = await client.missions(status="succeeded", limit=5)
            self.assertEqual(inventory["missions"][0]["mission_id"], "async-1")
            typed_inventory = await client.mission_inventory(status="succeeded", limit=5)
            self.assertIsInstance(typed_inventory, MissionInventoryPage)
            self.assertTrue(typed_inventory.missions[0].terminal)
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
            self.assertFalse((await client.event_page(review_id="a" * 64)).gap)
            self.assertEqual((await client.event_stream()).events[0].data, '{"mission_id":"async-1"}')
            self.assertTrue((await client.route_review_evidence("a" * 64)).found)

        asyncio.run(run())


if __name__ == "__main__":
    unittest.main()
