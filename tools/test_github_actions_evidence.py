from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).parent))

from github_actions_evidence import (  # noqa: E402
    ExportError,
    build_collection_envelope,
    build_payload,
    build_provider_evidence_request,
    canonical_bytes,
    discover_github_actions_payload,
    payload_digest,
    write_payload,
)


class _JsonResponse:
    def __init__(self, value: object) -> None:
        self._body = json.dumps(value).encode("utf-8")
        self.headers = {"Content-Length": str(len(self._body))}

    def __enter__(self) -> "_JsonResponse":
        return self

    def __exit__(self, *args: object) -> None:
        return None

    def read(self, limit: int = -1) -> bytes:
        return self._body[:limit] if limit >= 0 else self._body


class GithubActionsEvidenceTests(unittest.TestCase):
    def test_composite_action_contract_is_present(self) -> None:
        action = Path(__file__).parents[1] / ".github" / "actions" / "github-actions-evidence" / "action.yml"
        metadata = action.read_text(encoding="utf-8")
        self.assertIn("using: composite", metadata)
        self.assertIn("discover:", metadata)
        self.assertIn("github-token:", metadata)
        self.assertIn("collect-evidence:", metadata)
        self.assertIn("evidence-output:", metadata)
        self.assertIn("$GITHUB_ACTION_PATH/../../../tools/github_actions_evidence.py", metadata)
        self.assertIn("value: ${{ steps.export.outputs['payload-digest'] }}", metadata)
        self.assertIn("value: ${{ steps.export.outputs['discovery-mode'] }}", metadata)
        fixture = Path(__file__).parent / "fixtures" / "github-actions-checks.json"
        fixture_payload = json.loads(fixture.read_text(encoding="utf-8"))
        self.assertEqual([row["name"] for row in fixture_payload["jobs"]], ["unit", "lint"])

    def test_payload_is_bounded_deterministic_and_event_aware(self) -> None:
        event = {
            "workflow_run": {
                "id": 42,
                "conclusion": "success",
                "html_url": "https://github.com/example/repo/actions/runs/42",
            },
            "token": "must-not-be-copied",
        }
        checks = {
            "jobs": [
                {"name": "unit", "conclusion": "success", "duration_ms": 1200},
                {"name": "lint", "status": "skipped", "detail": "advisory"},
            ]
        }
        payload = build_payload(checks, event)
        self.assertEqual(payload["provider"], "github_actions")
        self.assertEqual(payload["run"]["id"], "42")
        self.assertEqual(payload["run"]["conclusion"], "success")
        self.assertEqual(payload["jobs"][1]["conclusion"], "skipped")
        self.assertNotIn("token", json.dumps(payload))
        self.assertEqual(payload_digest(payload), hashlib.sha256(canonical_bytes(payload)).hexdigest())

        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "nested" / "payload.json"
            github_output = Path(directory) / "github-output.txt"
            digest = write_payload(payload, output, github_output=github_output)
            self.assertEqual(output.read_bytes(), canonical_bytes(payload) + b"\n")
            self.assertIn(f"payload-digest={digest}", github_output.read_text(encoding="utf-8"))
            self.assertIn("check-count=2", github_output.read_text(encoding="utf-8"))
            self.assertIn("discovery-mode=manual", github_output.read_text(encoding="utf-8"))
            with self.assertRaisesRegex(ExportError, "output path.*control characters"):
                write_payload(payload, Path(directory) / "bad\noutput.json")

    def test_api_discovery_is_bounded_event_aware_and_token_free(self) -> None:
        responses = [
            {
                "id": 42,
                "conclusion": "success",
                "html_url": "https://github.com/example/repo/actions/runs/42",
            },
            {
                "jobs": [
                    {
                        "id": 101,
                        "name": "unit",
                        "status": "completed",
                        "conclusion": "success",
                        "started_at": "2026-08-18T00:00:00Z",
                        "completed_at": "2026-08-18T00:00:01.500Z",
                        "html_url": "https://github.com/example/repo/actions/runs/42/job/101",
                    },
                    {"id": 102, "name": "lint", "status": "queued", "conclusion": None},
                ]
            },
        ]

        def fake_urlopen(request: object, timeout: int) -> _JsonResponse:
            self.assertEqual(timeout, 30)
            assert hasattr(request, "headers")
            self.assertEqual(request.headers["Authorization"], "Bearer secret-token")
            self.assertEqual(request.get_header("X-github-api-version"), "2022-11-28")
            return _JsonResponse(responses.pop(0))

        with patch("github_actions_evidence.urlopen", side_effect=fake_urlopen):
            discovered = discover_github_actions_payload(
                token="secret-token",
                api_url="https://api.github.test",
                repository="example/repo",
                run_id="42",
            )

        self.assertEqual(len(responses), 0)
        self.assertEqual(discovered["run"]["id"], 42)
        self.assertEqual(discovered["jobs"][0]["duration_ms"], 1500)
        self.assertEqual(discovered["jobs"][1]["status"], "queued")
        self.assertNotIn("secret-token", json.dumps(discovered))

    def test_api_collection_binds_artifact_metadata_and_log_locators(self) -> None:
        responses = [
            {"id": 42, "conclusion": "success"},
            {
                "jobs": [
                    {
                        "id": 101,
                        "name": "unit",
                        "status": "completed",
                        "conclusion": "success",
                        "logs_url": "https://api.github.test/repos/example/repo/actions/jobs/101/logs",
                    }
                ]
            },
            {
                "artifacts": [
                    {
                        "id": 501,
                        "name": "junit",
                        "archive_download_url": "https://api.github.test/repos/example/repo/actions/artifacts/501",
                        "size_in_bytes": 2048,
                        "expired": False,
                    }
                ]
            },
        ]
        urls: list[str] = []

        def fake_urlopen(request: object, timeout: int) -> _JsonResponse:
            urls.append(request.full_url)  # type: ignore[attr-defined]
            return _JsonResponse(responses.pop(0))

        with patch("github_actions_evidence.urlopen", side_effect=fake_urlopen):
            discovered = discover_github_actions_payload(
                token="secret-token",
                api_url="https://api.github.test",
                repository="example/repo",
                run_id="42",
                collect_evidence=True,
            )

        self.assertEqual(len(responses), 0)
        self.assertEqual(len(discovered["artifacts"]), 1)
        self.assertEqual(discovered["artifacts"][0]["kind"], "junit")
        self.assertEqual(discovered["artifacts"][0]["run_id"], "42")
        self.assertEqual(discovered["logs"][0]["check"], "unit")
        self.assertEqual(discovered["logs"][0]["truncated"], False)
        self.assertIn("/artifacts?per_page=129", urls[-1])
        self.assertNotIn("secret-token", json.dumps(discovered))

    def test_manual_collection_emits_exact_registry_request_and_explicit_limits(self) -> None:
        from github_actions_evidence import main

        root = Path(__file__).parent / "fixtures"
        with tempfile.TemporaryDirectory() as directory:
            output_root = Path(directory)
            output = output_root / "payload.json"
            collection_output = output_root / "collection.json"
            evidence_output = output_root / "request.json"
            self.assertEqual(
                main(
                    [
                        "--checks",
                        str(root / "github-actions-checks.json"),
                        "--artifacts",
                        str(root / "github-actions-artifacts.json"),
                        "--logs",
                        str(root / "github-actions-logs.json"),
                        "--attestations",
                        str(root / "github-actions-attestations.json"),
                        "--ci",
                        str(root / "github-actions-ci.json"),
                        "--run-id",
                        "ci-fixture-run",
                        "--conclusion",
                        "success",
                        "--output",
                        str(output),
                        "--collection-output",
                        str(collection_output),
                        "--evidence-output",
                        str(evidence_output),
                    ]
                ),
                0,
            )
            collection = json.loads(collection_output.read_text(encoding="utf-8"))
            evidence = json.loads(evidence_output.read_text(encoding="utf-8"))
            self.assertEqual(collection["collection"]["verification"], "metadata_only")
            self.assertEqual(collection["collection"]["attestations"], "caller_supplied")
            self.assertEqual(evidence["source"], "caller_attested")
            self.assertEqual(evidence["ci"]["checks"][0]["name"], "unit")
            self.assertEqual(evidence["artifacts"][0]["run_id"], "ci-fixture-run")
            self.assertEqual(evidence["logs"][0]["check"], "unit")
            self.assertEqual(evidence["attestations"][0]["subject"], "artifact-unit")
            self.assertNotIn("token", json.dumps(collection).lower())

    def test_evidence_builders_do_not_infer_a_ci_plan(self) -> None:
        payload = build_payload([{"name": "unit", "conclusion": "success"}], {"run_id": "run-1"})
        with self.assertRaisesRegex(ExportError, "ci must be an object"):
            build_provider_evidence_request([], payload, source="provider_observed")
        envelope = build_collection_envelope(payload, discovery_mode="manual")
        self.assertEqual(envelope["collection"]["execution"], "not_started")
        self.assertEqual(envelope["collection"]["verification"], "metadata_only")

    def test_api_discovery_refuses_partial_job_lists_and_invalid_endpoints(self) -> None:
        responses = [{"id": "run-1", "conclusion": "success"}, {"jobs": [{"id": i} for i in range(65)]}]

        with patch(
            "github_actions_evidence.urlopen",
            side_effect=lambda request, timeout: _JsonResponse(responses.pop(0)),
        ):
            with self.assertRaisesRegex(ExportError, "refusing a partial payload"):
                discover_github_actions_payload(
                    token="secret-token",
                    api_url="https://api.github.test",
                    repository="example/repo",
                    run_id="run-1",
                )

        with self.assertRaisesRegex(ExportError, "absolute HTTPS"):
            discover_github_actions_payload(
                token="secret-token",
                api_url="http://api.github.test",
                repository="example/repo",
                run_id="run-1",
            )

    def test_api_discovery_refuses_provider_reported_rows_beyond_first_page(self) -> None:
        responses = [
            {"id": "run-1", "conclusion": "success"},
            {"total_count": 65, "jobs": [{"id": i, "name": f"job-{i}"} for i in range(64)]},
        ]
        with patch(
            "github_actions_evidence.urlopen",
            side_effect=lambda request, timeout: _JsonResponse(responses.pop(0)),
        ):
            with self.assertRaisesRegex(ExportError, "refusing a partial payload"):
                discover_github_actions_payload(
                    token="secret-token",
                    api_url="https://api.github.test",
                    repository="example/repo",
                    run_id="run-1",
                )

    def test_discovery_cli_prefers_workflow_run_target_over_downstream_runner_id(self) -> None:
        from github_actions_evidence import main

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / "event.json"
            event.write_text(
                json.dumps({"workflow_run": {"id": "upstream-run", "conclusion": "success"}}),
                encoding="utf-8",
            )
            output = root / "payload.json"
            with patch.dict(
                os.environ,
                {"GITHUB_RUN_ID": "downstream-run", "GITHUB_TOKEN": "secret-token"},
                clear=False,
            ), patch(
                "github_actions_evidence.discover_github_actions_payload",
                return_value={
                    "run": {"id": "upstream-run", "conclusion": "success"},
                    "jobs": [{"name": "unit", "conclusion": "success"}],
                },
            ) as discover:
                self.assertEqual(
                    main(
                        [
                            "--discover",
                            "--event",
                            str(event),
                            "--api-url",
                            "https://api.github.test",
                            "--repository",
                            "example/repo",
                            "--output",
                            str(output),
                        ]
                    ),
                    0,
                )
            self.assertEqual(discover.call_args.kwargs["run_id"], "upstream-run")
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["run"]["id"], "upstream-run")

    def test_missing_run_duplicate_and_oversized_inputs_fail_closed(self) -> None:
        with self.assertRaisesRegex(ExportError, "run.id"):
            build_payload([{"name": "unit", "conclusion": "success"}])
        with self.assertRaisesRegex(ExportError, "duplicate check name"):
            build_payload(
                [
                    {"name": "unit", "conclusion": "success"},
                    {"name": "unit", "conclusion": "success"},
                ],
                {"run_id": "run-1"},
            )
        with self.assertRaisesRegex(ExportError, "more than 64"):
            build_payload(
                [{"name": f"job-{index}", "conclusion": "success"} for index in range(65)],
                {"run_id": "run-1"},
            )

    def test_malformed_digest_and_control_text_are_not_rewritten(self) -> None:
        with self.assertRaisesRegex(ExportError, "lowercase SHA-256"):
            build_payload(
                [{"name": "unit", "conclusion": "success", "result_digest": "A" * 64}],
                {"run_id": "run-1"},
            )
        with self.assertRaisesRegex(ExportError, "control characters"):
            build_payload(
                [{"name": "unit\nforged", "conclusion": "success"}],
                {"run_id": "run-1"},
            )

    def test_cli_accepts_empty_optional_event_input(self) -> None:
        from github_actions_evidence import main

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            checks = root / "checks.json"
            checks.write_text(json.dumps([{"name": "unit", "conclusion": "success"}]), encoding="utf-8")
            output = root / "payload.json"
            previous_run_id = os.environ.get("GITHUB_RUN_ID")
            os.environ["GITHUB_RUN_ID"] = "run-from-environment"
            try:
                exit_code = main(
                    [
                        "--checks",
                        str(checks),
                        "--event",
                        "",
                        "--run-id",
                        "",
                        "--output",
                        str(output),
                    ]
                )
            finally:
                if previous_run_id is None:
                    os.environ.pop("GITHUB_RUN_ID", None)
                else:
                    os.environ["GITHUB_RUN_ID"] = previous_run_id
            self.assertEqual(exit_code, 0)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8"))["run"]["id"], "run-from-environment")


if __name__ == "__main__":
    unittest.main()
